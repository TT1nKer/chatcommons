use crate::{
    SyncError, SyncMessage, SyncPeer, auth,
    bootstrap::{BOOTSTRAP_NONCE_BYTES, possession_proof_bytes},
};
use chatcommons_crypto::{PUBLIC_KEY_LEN, SIGNATURE_LEN, UserId, verify};
use chatcommons_protocol::{EventId, SignedEvent, author_id, validate_event};
use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder, dcutr, identify, noise, ping, relay,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const NETWORK_PROTOCOL: &str = "/chatcommons/sync/1";
pub const MAX_NETWORK_FRAME_BYTES: u64 = 1024 * 1024;
pub const MAX_MESSAGES_PER_FRAME: usize = 64;
pub const MAX_BOOTSTRAP_ANCESTRY_EVENTS: usize = 256;
pub const IDENTIFY_PROTOCOL: &str = "/chatcommons/node/1";

type RequestResponse = request_response::json::Behaviour<NetworkRequest, NetworkResponse>;

#[derive(NetworkBehaviour)]
struct Behaviour {
    request_response: RequestResponse,
    relay_client: relay::client::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
    dcutr: dcutr::Behaviour,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum NetworkRequest {
    Authenticate {
        certificate: auth::DeviceCertificate,
    },
    Sync {
        message: SyncMessage,
    },
    BootstrapBegin {
        certificate: auth::DeviceCertificate,
        invitation: EventId,
    },
    BootstrapProve {
        invitation: EventId,
        signature: Vec<u8>,
    },
    BootstrapAccept {
        invitation: EventId,
        acceptance: SignedEvent,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum NetworkResponse {
    Authenticated,
    Sync {
        messages: Vec<SyncMessage>,
    },
    BootstrapChallenge {
        invitation: EventId,
        nonce: [u8; BOOTSTRAP_NONCE_BYTES],
    },
    BootstrapAncestry {
        invitation: EventId,
        events: Vec<SignedEvent>,
    },
    BootstrapAccepted,
    Rejected {
        reason: RejectionCode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionCode {
    InvalidCertificate,
    PeerIdMismatch,
    UserNotAllowed,
    DeviceRevoked,
    NotAuthenticated,
    InvalidSync,
    InvalidBootstrap,
    InvitationUnavailable,
    BootstrapNotApproved,
    FrameLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEvent {
    Listening(Multiaddr),
    Connected {
        peer: PeerId,
        relayed: bool,
    },
    RelayConnected(PeerId),
    RelayDisconnected(PeerId),
    RelayReservationAccepted(PeerId),
    RelayCircuitEstablished {
        relay: Option<PeerId>,
        remote: Option<PeerId>,
    },
    RelayConnectionFailed {
        relay: Option<PeerId>,
        reason: String,
    },
    ObservedAddress {
        peer: PeerId,
        address: Multiaddr,
    },
    HolePunchSucceeded(PeerId),
    HolePunchFailed {
        peer: PeerId,
        reason: String,
    },
    Authenticated(PeerId),
    SyncProgress(PeerId),
    BootstrapChallenge {
        peer: PeerId,
        invitation: EventId,
        proof_bytes: Vec<u8>,
    },
    BootstrapAncestry {
        peer: PeerId,
        invitation: EventId,
        events: Vec<SignedEvent>,
    },
    BootstrapAcceptance {
        peer: PeerId,
        user_id: UserId,
        invitation: EventId,
        acceptance: Box<SignedEvent>,
    },
    BootstrapAccepted(PeerId),
    RequestFailed {
        peer: PeerId,
        reason: String,
    },
    Disconnected(PeerId),
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("device authentication failed: {0}")]
    Auth(#[from] auth::AuthError),
    #[error("event synchronization failed: {0}")]
    Sync(#[from] SyncError),
    #[error("network listen failed: {0}")]
    Listen(String),
    #[error("network dial failed: {0}")]
    Dial(String),
    #[error("network transport setup failed: {0}")]
    Transport(String),
    #[error("request-response operation failed: {0}")]
    Request(String),
    #[error("remote peer rejected the request: {0:?}")]
    Rejected(RejectionCode),
    #[error("bootstrap operation is invalid: {0}")]
    Bootstrap(String),
}

#[derive(Clone)]
pub struct BootstrapGrant {
    pub invitation: EventId,
    pub capability_public_key: [u8; PUBLIC_KEY_LEN],
    pub ancestry: Vec<SignedEvent>,
}

struct PendingChallenge {
    invitation: EventId,
    authenticated: auth::AuthenticatedDevice,
    proof_bytes: Vec<u8>,
}

struct ProvedBootstrap {
    invitation: EventId,
    authenticated: auth::AuthenticatedDevice,
}

struct PendingAcceptance {
    invitation: EventId,
    authenticated: auth::AuthenticatedDevice,
    channel: request_response::ResponseChannel<NetworkResponse>,
}

#[derive(Clone, Copy)]
struct BootstrapTarget {
    peer: PeerId,
    invitation: EventId,
    endpoint_approved: bool,
}

enum AuthenticationState {
    Full,
    Provisional,
}

pub struct NetworkNode {
    swarm: Swarm<Behaviour>,
    sync: SyncPeer,
    certificate: auth::DeviceCertificate,
    allowed_users: BTreeSet<UserId>,
    trusted_infrastructure_devices: BTreeSet<auth::DeviceId>,
    revocations: auth::RevocationSet,
    authenticated: BTreeSet<PeerId>,
    authenticated_devices: BTreeMap<PeerId, auth::AuthenticatedDevice>,
    authentication_sent: BTreeSet<PeerId>,
    accepted_by_remote: BTreeSet<PeerId>,
    sync_started: BTreeSet<PeerId>,
    provisional: BTreeMap<PeerId, auth::AuthenticatedDevice>,
    bootstrap_grants: BTreeMap<EventId, BootstrapGrant>,
    pending_challenges: BTreeMap<PeerId, PendingChallenge>,
    proved_bootstraps: BTreeMap<PeerId, ProvedBootstrap>,
    pending_acceptances: BTreeMap<PeerId, PendingAcceptance>,
    bootstrap_target: Option<BootstrapTarget>,
    relay_peers: BTreeSet<PeerId>,
    relayed_application_peers: BTreeSet<PeerId>,
    pending_relay_dials: BTreeMap<PeerId, (PeerId, Multiaddr)>,
    pending_relay_reservations: BTreeMap<PeerId, Multiaddr>,
}

impl NetworkNode {
    pub fn new(
        device: &auth::DeviceIdentity,
        certificate: auth::DeviceCertificate,
        sync: SyncPeer,
        allowed_users: BTreeSet<UserId>,
        revocations: auth::RevocationSet,
    ) -> Result<Self, NetworkError> {
        if auth::peer_id_from_certificate(&certificate)? != device.peer_id() {
            return Err(NetworkError::Auth(auth::AuthError::InvalidDeviceKey));
        }
        let codec = request_response::json::codec::Codec::default()
            .set_request_size_maximum(MAX_NETWORK_FRAME_BYTES)
            .set_response_size_maximum(MAX_NETWORK_FRAME_BYTES);
        let request_response = request_response::Behaviour::with_codec(
            codec,
            [(StreamProtocol::new(NETWORK_PROTOCOL), ProtocolSupport::Full)],
            request_response::Config::default(),
        );
        let builder = SwarmBuilder::with_existing_identity(device.keypair())
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|error| NetworkError::Transport(error.to_string()))?
            .with_quic()
            .with_dns()
            .map_err(|error| NetworkError::Transport(error.to_string()))?
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .map_err(|error| NetworkError::Transport(error.to_string()))?
            .with_behaviour(|keypair, relay_client| Behaviour {
                request_response,
                relay_client,
                identify: identify::Behaviour::new(identify::Config::new(
                    IDENTIFY_PROTOCOL.to_owned(),
                    keypair.public(),
                )),
                ping: ping::Behaviour::new(ping::Config::new()),
                dcutr: dcutr::Behaviour::new(keypair.public().to_peer_id()),
            })
            .map_err(|error| NetworkError::Transport(error.to_string()))?;
        Ok(Self {
            swarm: builder.build(),
            sync,
            certificate,
            allowed_users,
            trusted_infrastructure_devices: BTreeSet::new(),
            revocations,
            authenticated: BTreeSet::new(),
            authenticated_devices: BTreeMap::new(),
            authentication_sent: BTreeSet::new(),
            accepted_by_remote: BTreeSet::new(),
            sync_started: BTreeSet::new(),
            provisional: BTreeMap::new(),
            bootstrap_grants: BTreeMap::new(),
            pending_challenges: BTreeMap::new(),
            proved_bootstraps: BTreeMap::new(),
            pending_acceptances: BTreeMap::new(),
            bootstrap_target: None,
            relay_peers: BTreeSet::new(),
            relayed_application_peers: BTreeSet::new(),
            pending_relay_dials: BTreeMap::new(),
            pending_relay_reservations: BTreeMap::new(),
        })
    }

    pub fn peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn listen(&mut self, address: Multiaddr) -> Result<(), NetworkError> {
        self.swarm
            .listen_on(address)
            .map(|_| ())
            .map_err(|error| NetworkError::Listen(format!("{error:?}")))
    }

    pub fn reserve_relay(&mut self, relay_address: Multiaddr) -> Result<Multiaddr, NetworkError> {
        let relay = relay_peer_from_base(&relay_address)
            .ok_or_else(|| NetworkError::Dial("relay address must end in its Peer ID".into()))?;
        if relay_address
            .iter()
            .any(|protocol| matches!(protocol, libp2p::multiaddr::Protocol::P2pCircuit))
        {
            return Err(NetworkError::Dial(
                "relay base address must not contain p2p-circuit".into(),
            ));
        }
        self.relay_peers.insert(relay);
        let circuit = relay_address
            .clone()
            .with(libp2p::multiaddr::Protocol::P2pCircuit);
        if self.pending_relay_reservations.contains_key(&relay) {
            return Err(NetworkError::Dial(
                "a reservation with this relay is already pending".into(),
            ));
        }
        self.pending_relay_reservations
            .insert(relay, circuit.clone());
        if let Err(error) = self.swarm.dial(relay_address) {
            self.pending_relay_reservations.remove(&relay);
            return Err(NetworkError::Dial(format!("{error:?}")));
        }
        Ok(circuit)
    }

    pub fn dial(&mut self, peer: PeerId, mut address: Multiaddr) -> Result<(), NetworkError> {
        if address
            .iter()
            .any(|protocol| matches!(protocol, libp2p::multiaddr::Protocol::P2pCircuit))
        {
            let relay = relay_peer_from_route(&address).ok_or_else(|| {
                NetworkError::Dial("relay route does not identify its relay peer".into())
            })?;
            self.relay_peers.insert(relay);
            let relay_base = relay_base_from_route(&address).ok_or_else(|| {
                NetworkError::Dial("relay route has an invalid base address".into())
            })?;
            if self.pending_relay_dials.contains_key(&relay) {
                return Err(NetworkError::Dial(
                    "a dial through this relay is already pending".into(),
                ));
            }
            self.pending_relay_dials.insert(relay, (peer, address));
            if let Err(error) = self.swarm.dial(relay_base) {
                self.pending_relay_dials.remove(&relay);
                return Err(NetworkError::Dial(format!("{error:?}")));
            }
            return Ok(());
        }
        address.push(libp2p::multiaddr::Protocol::P2p(peer));
        self.swarm
            .dial(address)
            .map_err(|error| NetworkError::Dial(format!("{error:?}")))
    }

    pub fn is_authenticated(&self, peer: PeerId) -> bool {
        self.authenticated.contains(&peer)
    }

    /// Replace the live authorization projection after signed community state
    /// changes. Peers removed by the new projection immediately lose sync
    /// authorization even if their transport connection remains open.
    pub fn replace_authorization(
        &mut self,
        allowed_users: BTreeSet<UserId>,
        trusted_infrastructure_devices: BTreeSet<auth::DeviceId>,
    ) {
        self.allowed_users = allowed_users;
        self.trusted_infrastructure_devices = trusted_infrastructure_devices;
        let rejected: Vec<PeerId> = self
            .authenticated_devices
            .iter()
            .filter(|(_, device)| {
                !self.allowed_users.contains(&device.user_id)
                    && !self
                        .trusted_infrastructure_devices
                        .contains(&device.device_id)
            })
            .map(|(peer, _)| *peer)
            .collect();
        for peer in rejected {
            self.authenticated.remove(&peer);
            self.authenticated_devices.remove(&peer);
            self.authentication_sent.remove(&peer);
            self.accepted_by_remote.remove(&peer);
            self.sync_started.remove(&peer);
            let _ = self.swarm.disconnect_peer_id(peer);
        }
    }

    pub fn sync_peer(&self) -> &SyncPeer {
        &self.sync
    }

    pub fn sync_peer_mut(&mut self) -> &mut SyncPeer {
        &mut self.sync
    }

    pub fn register_bootstrap_grant(&mut self, grant: BootstrapGrant) -> Result<(), NetworkError> {
        self.validate_bootstrap_grant(&grant)?;
        self.bootstrap_grants.insert(grant.invitation, grant);
        Ok(())
    }

    pub fn replace_bootstrap_grants(
        &mut self,
        grants: Vec<BootstrapGrant>,
    ) -> Result<(), NetworkError> {
        let mut replacement = BTreeMap::new();
        for grant in grants {
            self.validate_bootstrap_grant(&grant)?;
            replacement.insert(grant.invitation, grant);
        }
        self.bootstrap_grants = replacement;
        Ok(())
    }

    fn validate_bootstrap_grant(&self, grant: &BootstrapGrant) -> Result<(), NetworkError> {
        if grant.ancestry.is_empty()
            || grant.ancestry.len() > MAX_BOOTSTRAP_ANCESTRY_EVENTS
            || !grant
                .ancestry
                .iter()
                .any(|event| event.event_id == grant.invitation)
        {
            return Err(NetworkError::Bootstrap(
                "invitation ancestry is empty, oversized, or incomplete".into(),
            ));
        }
        for event in &grant.ancestry {
            validate_event(event).map_err(|error| NetworkError::Bootstrap(error.to_string()))?;
            let belongs = event.content.community_id == Some(self.sync.community())
                || (event.content.community_id.is_none()
                    && chatcommons_protocol::CommunityId::from(event.event_id)
                        == self.sync.community());
            if !belongs {
                return Err(NetworkError::Bootstrap(
                    "invitation ancestry belongs to another community".into(),
                ));
            }
        }
        let response = NetworkResponse::BootstrapAncestry {
            invitation: grant.invitation,
            events: grant.ancestry.clone(),
        };
        if serde_json::to_vec(&response)
            .map_err(|error| NetworkError::Bootstrap(error.to_string()))?
            .len()
            > MAX_NETWORK_FRAME_BYTES as usize
        {
            return Err(NetworkError::Bootstrap(
                "invitation ancestry exceeds the network frame limit".into(),
            ));
        }
        Ok(())
    }

    pub fn configure_bootstrap_target(
        &mut self,
        peer: PeerId,
        invitation: EventId,
    ) -> Result<(), NetworkError> {
        if self.bootstrap_target.is_some() {
            return Err(NetworkError::Bootstrap(
                "a bootstrap target is already configured".into(),
            ));
        }
        self.bootstrap_target = Some(BootstrapTarget {
            peer,
            invitation,
            endpoint_approved: false,
        });
        Ok(())
    }

    pub fn provisional_user(&self, peer: PeerId) -> Option<UserId> {
        self.provisional.get(&peer).map(|device| device.user_id)
    }

    pub fn provisional_device(&self, peer: PeerId) -> Option<auth::DeviceId> {
        self.provisional.get(&peer).map(|device| device.device_id)
    }

    pub fn approve_bootstrap_endpoint(&mut self, peer: PeerId) -> Result<(), NetworkError> {
        let target = self
            .bootstrap_target
            .as_mut()
            .filter(|target| target.peer == peer)
            .ok_or_else(|| NetworkError::Bootstrap("peer is not the bootstrap target".into()))?;
        if !self.provisional.contains_key(&peer) {
            return Err(NetworkError::Bootstrap(
                "bootstrap endpoint has not presented a valid device certificate".into(),
            ));
        }
        target.endpoint_approved = true;
        Ok(())
    }

    pub fn submit_bootstrap_proof(
        &mut self,
        peer: PeerId,
        invitation: EventId,
        signature: Vec<u8>,
    ) -> Result<(), NetworkError> {
        let target = self
            .bootstrap_target
            .filter(|target| target.peer == peer && target.invitation == invitation)
            .ok_or_else(|| NetworkError::Bootstrap("unexpected bootstrap challenge".into()))?;
        if signature.len() != SIGNATURE_LEN {
            return Err(NetworkError::Bootstrap(
                "bootstrap proof signature has the wrong length".into(),
            ));
        }
        let _ = target;
        self.send(
            peer,
            NetworkRequest::BootstrapProve {
                invitation,
                signature,
            },
        )
    }

    pub fn submit_bootstrap_acceptance(
        &mut self,
        peer: PeerId,
        invitation: EventId,
        acceptance: SignedEvent,
    ) -> Result<(), NetworkError> {
        let target = self
            .bootstrap_target
            .filter(|target| target.peer == peer && target.invitation == invitation)
            .ok_or_else(|| NetworkError::Bootstrap("unexpected bootstrap ancestry".into()))?;
        if !target.endpoint_approved {
            return Err(NetworkError::Bootstrap(
                "bootstrap endpoint membership has not been approved".into(),
            ));
        }
        validate_event(&acceptance).map_err(|error| NetworkError::Bootstrap(error.to_string()))?;
        self.send(
            peer,
            NetworkRequest::BootstrapAccept {
                invitation,
                acceptance,
            },
        )
    }

    pub fn resolve_bootstrap_acceptance(
        &mut self,
        peer: PeerId,
        approved: bool,
    ) -> Result<(), NetworkError> {
        let pending = self
            .pending_acceptances
            .remove(&peer)
            .ok_or_else(|| NetworkError::Bootstrap("no pending bootstrap acceptance".into()))?;
        if approved {
            self.bootstrap_grants.remove(&pending.invitation);
            self.allowed_users.insert(pending.authenticated.user_id);
            self.authenticated.insert(peer);
            self.authenticated_devices
                .insert(peer, pending.authenticated);
            self.swarm
                .behaviour_mut()
                .request_response
                .send_response(pending.channel, NetworkResponse::BootstrapAccepted)
                .map_err(|_| NetworkError::Request("response channel closed".into()))?;
            self.maybe_start_sync(peer)?;
        } else {
            self.swarm
                .behaviour_mut()
                .request_response
                .send_response(
                    pending.channel,
                    NetworkResponse::Rejected {
                        reason: RejectionCode::BootstrapNotApproved,
                    },
                )
                .map_err(|_| NetworkError::Request("response channel closed".into()))?;
        }
        Ok(())
    }

    pub async fn next_event(&mut self) -> Result<NetworkEvent, NetworkError> {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Ok(NetworkEvent::Listening(address));
                }
                SwarmEvent::ConnectionEstablished {
                    peer_id, endpoint, ..
                } => {
                    if self.relay_peers.contains(&peer_id) {
                        return Ok(NetworkEvent::RelayConnected(peer_id));
                    }
                    if endpoint.is_relayed() {
                        self.relayed_application_peers.insert(peer_id);
                    }
                    if !self.authenticated.contains(&peer_id)
                        && self.authentication_sent.insert(peer_id)
                    {
                        self.send(
                            peer_id,
                            NetworkRequest::Authenticate {
                                certificate: self.certificate.clone(),
                            },
                        )?;
                    }
                    return Ok(NetworkEvent::Connected {
                        peer: peer_id,
                        relayed: endpoint.is_relayed(),
                    });
                }
                SwarmEvent::ConnectionClosed {
                    peer_id,
                    num_established,
                    ..
                } => {
                    if num_established > 0 {
                        continue;
                    }
                    if self.relay_peers.contains(&peer_id) {
                        return Ok(NetworkEvent::RelayDisconnected(peer_id));
                    }
                    self.authenticated.remove(&peer_id);
                    self.authenticated_devices.remove(&peer_id);
                    self.authentication_sent.remove(&peer_id);
                    self.accepted_by_remote.remove(&peer_id);
                    self.sync_started.remove(&peer_id);
                    self.provisional.remove(&peer_id);
                    self.pending_challenges.remove(&peer_id);
                    self.proved_bootstraps.remove(&peer_id);
                    self.pending_acceptances.remove(&peer_id);
                    return Ok(NetworkEvent::Disconnected(peer_id));
                }
                SwarmEvent::Behaviour(event) => {
                    if let Some(event) = self.handle_behaviour(event)? {
                        return Ok(event);
                    }
                }
                SwarmEvent::OutgoingConnectionError { peer_id, error, .. }
                    if peer_id.is_some_and(|peer| self.relay_peers.contains(&peer)) =>
                {
                    return Ok(NetworkEvent::RelayConnectionFailed {
                        relay: peer_id,
                        reason: format!("{error:?}"),
                    });
                }
                SwarmEvent::OutgoingConnectionError {
                    peer_id: Some(peer),
                    error,
                    ..
                } if self.relayed_application_peers.contains(&peer) => {
                    // DCUtR's direct candidates are supplemental to an already
                    // established relay path. Their errors can arrive after the
                    // application peer has cleanly closed that path, so connection
                    // state alone cannot distinguish them from a primary dial.
                    return Ok(NetworkEvent::HolePunchFailed {
                        peer,
                        reason: format!("{error:?}"),
                    });
                }
                SwarmEvent::OutgoingConnectionError {
                    peer_id: Some(peer),
                    ..
                } if self.swarm.is_connected(&peer) => {
                    // DCUtR may fail individual direct candidates while the authenticated
                    // relayed connection remains usable. Its own behaviour emits the final
                    // success or failure after the bounded retry sequence.
                }
                SwarmEvent::OutgoingConnectionError { error, .. } => {
                    return Err(NetworkError::Dial(format!("{error:?}")));
                }
                SwarmEvent::ListenerError { error, .. } => {
                    return Err(NetworkError::Listen(format!("{error:?}")));
                }
                _ => {}
            }
        }
    }

    fn handle_behaviour(
        &mut self,
        event: BehaviourEvent,
    ) -> Result<Option<NetworkEvent>, NetworkError> {
        match event {
            BehaviourEvent::RequestResponse(event) => self.handle_request_response(event),
            BehaviourEvent::RelayClient(relay::client::Event::ReservationReqAccepted {
                relay_peer_id,
                ..
            }) => Ok(Some(NetworkEvent::RelayReservationAccepted(relay_peer_id))),
            BehaviourEvent::RelayClient(relay::client::Event::OutboundCircuitEstablished {
                relay_peer_id,
                ..
            }) => Ok(Some(NetworkEvent::RelayCircuitEstablished {
                relay: Some(relay_peer_id),
                remote: None,
            })),
            BehaviourEvent::RelayClient(relay::client::Event::InboundCircuitEstablished {
                src_peer_id,
                ..
            }) => Ok(Some(NetworkEvent::RelayCircuitEstablished {
                relay: None,
                remote: Some(src_peer_id),
            })),
            BehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. }) => {
                if let Some(route) = self.pending_relay_reservations.remove(&peer_id) {
                    self.swarm
                        .listen_on(route)
                        .map_err(|error| NetworkError::Listen(format!("{error:?}")))?;
                }
                if let Some((target, mut route)) = self.pending_relay_dials.remove(&peer_id) {
                    route.push(libp2p::multiaddr::Protocol::P2p(target));
                    self.swarm
                        .dial(route)
                        .map_err(|error| NetworkError::Dial(format!("{error:?}")))?;
                }
                Ok(Some(NetworkEvent::ObservedAddress {
                    peer: peer_id,
                    address: info.observed_addr,
                }))
            }
            BehaviourEvent::Dcutr(event) => match event.result {
                Ok(_) => Ok(Some(NetworkEvent::HolePunchSucceeded(event.remote_peer_id))),
                Err(error) => Ok(Some(NetworkEvent::HolePunchFailed {
                    peer: event.remote_peer_id,
                    reason: error.to_string(),
                })),
            },
            BehaviourEvent::Ping(_) | BehaviourEvent::Identify(_) => Ok(None),
        }
    }

    fn handle_request_response(
        &mut self,
        event: request_response::Event<NetworkRequest, NetworkResponse>,
    ) -> Result<Option<NetworkEvent>, NetworkError> {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    let request = match request {
                        NetworkRequest::BootstrapAccept {
                            invitation,
                            acceptance,
                        } => {
                            return self
                                .handle_bootstrap_acceptance(peer, invitation, acceptance, channel)
                                .map(Some);
                        }
                        request => request,
                    };
                    let (response, authentication) = self.handle_request(peer, request);
                    self.respond(channel, response)?;
                    if matches!(authentication, Some(AuthenticationState::Full)) {
                        self.maybe_start_sync(peer)?;
                    } else if matches!(authentication, Some(AuthenticationState::Provisional)) {
                        self.start_bootstrap(peer)?;
                    }
                    Ok(Some(
                        if matches!(authentication, Some(AuthenticationState::Full)) {
                            NetworkEvent::Authenticated(peer)
                        } else {
                            NetworkEvent::SyncProgress(peer)
                        },
                    ))
                }
                request_response::Message::Response { response, .. } => {
                    self.handle_response(peer, response).map(Some)
                }
            },
            request_response::Event::OutboundFailure { peer, error, .. } => {
                Ok(Some(NetworkEvent::RequestFailed {
                    peer,
                    reason: error.to_string(),
                }))
            }
            request_response::Event::InboundFailure { peer, error, .. } => {
                Ok(Some(NetworkEvent::RequestFailed {
                    peer,
                    reason: error.to_string(),
                }))
            }
            request_response::Event::ResponseSent { .. } => Ok(None),
        }
    }

    fn handle_request(
        &mut self,
        peer: PeerId,
        request: NetworkRequest,
    ) -> (NetworkResponse, Option<AuthenticationState>) {
        match request {
            NetworkRequest::Authenticate { certificate } => {
                match self.authenticate(peer, &certificate) {
                    Ok(state) => (NetworkResponse::Authenticated, Some(state)),
                    Err(reason) => (NetworkResponse::Rejected { reason }, None),
                }
            }
            NetworkRequest::Sync { message } => {
                if !self.authenticated.contains(&peer) {
                    return (
                        NetworkResponse::Rejected {
                            reason: RejectionCode::NotAuthenticated,
                        },
                        None,
                    );
                }
                if matches!(&message, SyncMessage::Want { event_ids, .. } if event_ids.len() != 1) {
                    return (
                        NetworkResponse::Rejected {
                            reason: RejectionCode::FrameLimit,
                        },
                        None,
                    );
                }
                match self.sync.receive(message) {
                    Ok(messages) => match normalize_messages(messages) {
                        Ok(messages) => (NetworkResponse::Sync { messages }, None),
                        Err(reason) => (NetworkResponse::Rejected { reason }, None),
                    },
                    Err(_) => (
                        NetworkResponse::Rejected {
                            reason: RejectionCode::InvalidSync,
                        },
                        None,
                    ),
                }
            }
            NetworkRequest::BootstrapBegin {
                certificate,
                invitation,
            } => (self.begin_bootstrap(peer, &certificate, invitation), None),
            NetworkRequest::BootstrapProve {
                invitation,
                signature,
            } => (self.prove_bootstrap(peer, invitation, &signature), None),
            NetworkRequest::BootstrapAccept { .. } => (
                NetworkResponse::Rejected {
                    reason: RejectionCode::InvalidBootstrap,
                },
                None,
            ),
        }
    }

    fn begin_bootstrap(
        &mut self,
        peer: PeerId,
        certificate: &auth::DeviceCertificate,
        invitation: EventId,
    ) -> NetworkResponse {
        let authenticated = match self.validate_peer_certificate(peer, certificate) {
            Ok(authenticated) => authenticated,
            Err(reason) => return NetworkResponse::Rejected { reason },
        };
        if !self.bootstrap_grants.contains_key(&invitation) {
            return NetworkResponse::Rejected {
                reason: RejectionCode::InvitationUnavailable,
            };
        }
        let mut nonce = [0_u8; BOOTSTRAP_NONCE_BYTES];
        OsRng.fill_bytes(&mut nonce);
        let proof_bytes = possession_proof_bytes(
            self.sync.community(),
            invitation,
            authenticated.user_id,
            authenticated.device_id.as_bytes(),
            peer,
            self.peer_id(),
            &nonce,
        );
        self.pending_challenges.insert(
            peer,
            PendingChallenge {
                invitation,
                authenticated,
                proof_bytes,
            },
        );
        NetworkResponse::BootstrapChallenge { invitation, nonce }
    }

    fn prove_bootstrap(
        &mut self,
        peer: PeerId,
        invitation: EventId,
        signature: &[u8],
    ) -> NetworkResponse {
        let Some(pending) = self.pending_challenges.remove(&peer) else {
            return NetworkResponse::Rejected {
                reason: RejectionCode::InvalidBootstrap,
            };
        };
        let Some(grant) = self.bootstrap_grants.get(&invitation) else {
            return NetworkResponse::Rejected {
                reason: RejectionCode::InvitationUnavailable,
            };
        };
        if pending.invitation != invitation
            || signature.len() != SIGNATURE_LEN
            || verify(
                &grant.capability_public_key,
                &pending.proof_bytes,
                signature,
            )
            .is_err()
        {
            return NetworkResponse::Rejected {
                reason: RejectionCode::InvalidBootstrap,
            };
        }
        self.proved_bootstraps.insert(
            peer,
            ProvedBootstrap {
                invitation,
                authenticated: pending.authenticated,
            },
        );
        NetworkResponse::BootstrapAncestry {
            invitation,
            events: grant.ancestry.clone(),
        }
    }

    fn handle_bootstrap_acceptance(
        &mut self,
        peer: PeerId,
        invitation: EventId,
        acceptance: SignedEvent,
        channel: request_response::ResponseChannel<NetworkResponse>,
    ) -> Result<NetworkEvent, NetworkError> {
        let proved = self.proved_bootstraps.remove(&peer);
        let valid = proved.as_ref().is_some_and(|proved| {
            self.bootstrap_grants.contains_key(&invitation)
                && proved.invitation == invitation
                && acceptance.content.community_id == Some(self.sync.community())
                && acceptance.content.parents.contains(&invitation)
                && validate_event(&acceptance).is_ok()
                && author_id(&acceptance).ok() == Some(proved.authenticated.user_id)
        });
        let Some(proved) = proved.filter(|_| valid) else {
            self.respond(
                channel,
                NetworkResponse::Rejected {
                    reason: RejectionCode::InvalidBootstrap,
                },
            )?;
            return Ok(NetworkEvent::SyncProgress(peer));
        };
        if self.pending_acceptances.contains_key(&peer) {
            self.respond(
                channel,
                NetworkResponse::Rejected {
                    reason: RejectionCode::InvalidBootstrap,
                },
            )?;
            return Ok(NetworkEvent::SyncProgress(peer));
        }
        self.pending_acceptances.insert(
            peer,
            PendingAcceptance {
                invitation,
                authenticated: proved.authenticated,
                channel,
            },
        );
        Ok(NetworkEvent::BootstrapAcceptance {
            peer,
            user_id: proved.authenticated.user_id,
            invitation,
            acceptance: Box::new(acceptance),
        })
    }

    fn handle_response(
        &mut self,
        peer: PeerId,
        response: NetworkResponse,
    ) -> Result<NetworkEvent, NetworkError> {
        match response {
            NetworkResponse::Authenticated => {
                self.accepted_by_remote.insert(peer);
                self.maybe_start_sync(peer)?;
                Ok(NetworkEvent::Authenticated(peer))
            }
            NetworkResponse::Sync { messages } => {
                if !self.authenticated.contains(&peer) {
                    return Err(NetworkError::Rejected(RejectionCode::NotAuthenticated));
                }
                if messages.len() > MAX_MESSAGES_PER_FRAME {
                    return Err(NetworkError::Rejected(RejectionCode::FrameLimit));
                }
                for message in messages {
                    let outgoing = normalize_messages(self.sync.receive(message)?)
                        .map_err(NetworkError::Rejected)?;
                    for message in outgoing {
                        self.send(peer, NetworkRequest::Sync { message })?;
                    }
                }
                Ok(NetworkEvent::SyncProgress(peer))
            }
            NetworkResponse::BootstrapChallenge { invitation, nonce } => {
                let target = self
                    .bootstrap_target
                    .filter(|target| target.peer == peer && target.invitation == invitation)
                    .ok_or_else(|| {
                        NetworkError::Bootstrap("unexpected bootstrap challenge".into())
                    })?;
                let local = auth::validate_device_certificate(&self.certificate)?;
                let proof_bytes = possession_proof_bytes(
                    self.sync.community(),
                    invitation,
                    local.user_id,
                    local.device_id.as_bytes(),
                    self.peer_id(),
                    peer,
                    &nonce,
                );
                let _ = target;
                Ok(NetworkEvent::BootstrapChallenge {
                    peer,
                    invitation,
                    proof_bytes,
                })
            }
            NetworkResponse::BootstrapAncestry { invitation, events } => {
                let target = self
                    .bootstrap_target
                    .filter(|target| target.peer == peer && target.invitation == invitation)
                    .ok_or_else(|| {
                        NetworkError::Bootstrap("unexpected bootstrap ancestry".into())
                    })?;
                if events.is_empty() || events.len() > MAX_BOOTSTRAP_ANCESTRY_EVENTS {
                    return Err(NetworkError::Rejected(RejectionCode::FrameLimit));
                }
                for event in &events {
                    validate_event(event)
                        .map_err(|_| NetworkError::Rejected(RejectionCode::InvalidBootstrap))?;
                    let belongs = event.content.community_id == Some(self.sync.community())
                        || (event.content.community_id.is_none()
                            && chatcommons_protocol::CommunityId::from(event.event_id)
                                == self.sync.community());
                    if !belongs {
                        return Err(NetworkError::Rejected(RejectionCode::InvalidBootstrap));
                    }
                }
                if !events.iter().any(|event| event.event_id == invitation) {
                    return Err(NetworkError::Rejected(RejectionCode::InvalidBootstrap));
                }
                let _ = target;
                Ok(NetworkEvent::BootstrapAncestry {
                    peer,
                    invitation,
                    events,
                })
            }
            NetworkResponse::BootstrapAccepted => {
                let target = self
                    .bootstrap_target
                    .filter(|target| target.peer == peer && target.endpoint_approved)
                    .ok_or_else(|| {
                        NetworkError::Bootstrap("bootstrap endpoint was not approved".into())
                    })?;
                let provisional = self.provisional.remove(&peer).ok_or_else(|| {
                    NetworkError::Bootstrap(
                        "bootstrap endpoint did not present a valid certificate".into(),
                    )
                })?;
                self.allowed_users.insert(provisional.user_id);
                self.authenticated.insert(peer);
                self.authenticated_devices.insert(peer, provisional);
                self.accepted_by_remote.insert(peer);
                self.maybe_start_sync(peer)?;
                let _ = target;
                Ok(NetworkEvent::BootstrapAccepted(peer))
            }
            NetworkResponse::Rejected {
                reason: RejectionCode::UserNotAllowed,
            } if self
                .bootstrap_target
                .is_some_and(|target| target.peer == peer) =>
            {
                Ok(NetworkEvent::SyncProgress(peer))
            }
            NetworkResponse::Rejected { reason } => Err(NetworkError::Rejected(reason)),
        }
    }

    fn authenticate(
        &mut self,
        peer: PeerId,
        certificate: &auth::DeviceCertificate,
    ) -> Result<AuthenticationState, RejectionCode> {
        let authenticated = self.validate_peer_certificate(peer, certificate)?;
        if self.allowed_users.contains(&authenticated.user_id)
            || self
                .trusted_infrastructure_devices
                .contains(&authenticated.device_id)
        {
            self.authenticated.insert(peer);
            self.authenticated_devices.insert(peer, authenticated);
            return Ok(AuthenticationState::Full);
        }
        if self
            .bootstrap_target
            .is_some_and(|target| target.peer == peer)
        {
            self.provisional.insert(peer, authenticated);
            return Ok(AuthenticationState::Provisional);
        }
        Err(RejectionCode::UserNotAllowed)
    }

    fn validate_peer_certificate(
        &self,
        peer: PeerId,
        certificate: &auth::DeviceCertificate,
    ) -> Result<auth::AuthenticatedDevice, RejectionCode> {
        let authenticated = auth::validate_device_certificate(certificate)
            .map_err(|_| RejectionCode::InvalidCertificate)?;
        let certificate_peer = auth::peer_id_from_certificate(certificate)
            .map_err(|_| RejectionCode::InvalidCertificate)?;
        if certificate_peer != peer {
            return Err(RejectionCode::PeerIdMismatch);
        }
        if self
            .revocations
            .contains(authenticated.user_id, authenticated.device_id)
        {
            return Err(RejectionCode::DeviceRevoked);
        }
        Ok(authenticated)
    }

    fn respond(
        &mut self,
        channel: request_response::ResponseChannel<NetworkResponse>,
        response: NetworkResponse,
    ) -> Result<(), NetworkError> {
        if serde_json::to_vec(&response)
            .map_err(|error| NetworkError::Request(error.to_string()))?
            .len()
            > MAX_NETWORK_FRAME_BYTES as usize
        {
            return Err(NetworkError::Rejected(RejectionCode::FrameLimit));
        }
        self.swarm
            .behaviour_mut()
            .request_response
            .send_response(channel, response)
            .map_err(|_| NetworkError::Request("response channel closed".into()))
    }

    fn send(&mut self, peer: PeerId, request: NetworkRequest) -> Result<(), NetworkError> {
        if serde_json::to_vec(&request)
            .map_err(|error| NetworkError::Request(error.to_string()))?
            .len()
            > MAX_NETWORK_FRAME_BYTES as usize
        {
            return Err(NetworkError::Rejected(RejectionCode::FrameLimit));
        }
        self.swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer, request);
        Ok(())
    }

    fn start_bootstrap(&mut self, peer: PeerId) -> Result<(), NetworkError> {
        let target = self
            .bootstrap_target
            .filter(|target| target.peer == peer)
            .ok_or_else(|| NetworkError::Bootstrap("peer is not the bootstrap target".into()))?;
        self.send(
            peer,
            NetworkRequest::BootstrapBegin {
                certificate: self.certificate.clone(),
                invitation: target.invitation,
            },
        )
    }

    fn maybe_start_sync(&mut self, peer: PeerId) -> Result<(), NetworkError> {
        if self.authenticated.contains(&peer)
            && self.accepted_by_remote.contains(&peer)
            && self.sync_started.insert(peer)
        {
            self.send(
                peer,
                NetworkRequest::Sync {
                    message: self.sync.hello(),
                },
            )?;
        }
        Ok(())
    }
}

fn relay_peer_from_base(address: &Multiaddr) -> Option<PeerId> {
    match address.iter().last() {
        Some(libp2p::multiaddr::Protocol::P2p(peer)) => Some(peer),
        _ => None,
    }
}

fn relay_peer_from_route(address: &Multiaddr) -> Option<PeerId> {
    let mut previous_peer = None;
    for protocol in address.iter() {
        match protocol {
            libp2p::multiaddr::Protocol::P2p(peer) => previous_peer = Some(peer),
            libp2p::multiaddr::Protocol::P2pCircuit => return previous_peer,
            _ => {}
        }
    }
    None
}

fn relay_base_from_route(address: &Multiaddr) -> Option<Multiaddr> {
    let mut base = Multiaddr::empty();
    for protocol in address.iter() {
        if matches!(protocol, libp2p::multiaddr::Protocol::P2pCircuit) {
            return relay_peer_from_base(&base).map(|_| base);
        }
        base.push(protocol);
    }
    None
}

fn normalize_messages(messages: Vec<SyncMessage>) -> Result<Vec<SyncMessage>, RejectionCode> {
    let mut normalized = Vec::new();
    for message in messages {
        match message {
            SyncMessage::Want {
                community_id,
                event_ids,
            } => {
                normalized.extend(event_ids.into_iter().map(|event_id| SyncMessage::Want {
                    community_id,
                    event_ids: vec![event_id],
                }));
            }
            other => normalized.push(other),
        }
    }
    if normalized.len() > MAX_MESSAGES_PER_FRAME {
        return Err(RejectionCode::FrameLimit);
    }
    let response = NetworkResponse::Sync {
        messages: normalized.clone(),
    };
    let encoded = serde_json::to_vec(&response).map_err(|_| RejectionCode::FrameLimit)?;
    if encoded.len() > MAX_NETWORK_FRAME_BYTES as usize {
        return Err(RejectionCode::FrameLimit);
    }
    Ok(normalized)
}
