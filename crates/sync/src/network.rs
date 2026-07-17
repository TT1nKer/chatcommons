use crate::{SyncError, SyncMessage, SyncPeer, auth};
use chatcommons_crypto::UserId;
use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
    request_response::{self, ProtocolSupport},
    swarm::SwarmEvent,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const NETWORK_PROTOCOL: &str = "/chatcommons/sync/1";
pub const MAX_NETWORK_FRAME_BYTES: u64 = 1024 * 1024;
pub const MAX_MESSAGES_PER_FRAME: usize = 64;

type Behaviour = request_response::json::Behaviour<NetworkRequest, NetworkResponse>;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum NetworkRequest {
    Authenticate {
        certificate: auth::DeviceCertificate,
    },
    Sync {
        message: SyncMessage,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum NetworkResponse {
    Authenticated,
    Sync { messages: Vec<SyncMessage> },
    Rejected { reason: RejectionCode },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RejectionCode {
    InvalidCertificate,
    PeerIdMismatch,
    UserNotAllowed,
    DeviceRevoked,
    NotAuthenticated,
    InvalidSync,
    FrameLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkEvent {
    Listening(Multiaddr),
    Connected(PeerId),
    Authenticated(PeerId),
    SyncProgress(PeerId),
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
    #[error("request-response operation failed: {0}")]
    Request(String),
    #[error("remote peer rejected the request: {0:?}")]
    Rejected(RejectionCode),
}

pub struct NetworkNode {
    swarm: Swarm<Behaviour>,
    sync: SyncPeer,
    certificate: auth::DeviceCertificate,
    allowed_users: BTreeSet<UserId>,
    revocations: auth::RevocationSet,
    authenticated: BTreeSet<PeerId>,
    accepted_by_remote: BTreeSet<PeerId>,
    sync_started: BTreeSet<PeerId>,
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
        let behaviour = request_response::Behaviour::with_codec(
            codec,
            [(StreamProtocol::new(NETWORK_PROTOCOL), ProtocolSupport::Full)],
            request_response::Config::default(),
        );
        let builder = SwarmBuilder::with_existing_identity(device.keypair())
            .with_tokio()
            .with_quic()
            .with_behaviour(|_| behaviour);
        let builder = match builder {
            Ok(builder) => builder,
            Err(error) => match error {},
        };
        Ok(Self {
            swarm: builder.build(),
            sync,
            certificate,
            allowed_users,
            revocations,
            authenticated: BTreeSet::new(),
            accepted_by_remote: BTreeSet::new(),
            sync_started: BTreeSet::new(),
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

    pub fn dial(&mut self, peer: PeerId, mut address: Multiaddr) -> Result<(), NetworkError> {
        address.push(libp2p::multiaddr::Protocol::P2p(peer));
        self.swarm
            .dial(address)
            .map_err(|error| NetworkError::Dial(format!("{error:?}")))
    }

    pub fn is_authenticated(&self, peer: PeerId) -> bool {
        self.authenticated.contains(&peer)
    }

    pub fn sync_peer(&self) -> &SyncPeer {
        &self.sync
    }

    pub async fn next_event(&mut self) -> Result<NetworkEvent, NetworkError> {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Ok(NetworkEvent::Listening(address));
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    self.send(
                        peer_id,
                        NetworkRequest::Authenticate {
                            certificate: self.certificate.clone(),
                        },
                    )?;
                    return Ok(NetworkEvent::Connected(peer_id));
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    self.authenticated.remove(&peer_id);
                    self.accepted_by_remote.remove(&peer_id);
                    self.sync_started.remove(&peer_id);
                    return Ok(NetworkEvent::Disconnected(peer_id));
                }
                SwarmEvent::Behaviour(event) => {
                    return self.handle_behaviour(event);
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
        event: request_response::Event<NetworkRequest, NetworkResponse>,
    ) -> Result<NetworkEvent, NetworkError> {
        match event {
            request_response::Event::Message { peer, message, .. } => match message {
                request_response::Message::Request {
                    request, channel, ..
                } => {
                    let (response, authenticated_now) = self.handle_request(peer, request);
                    self.swarm
                        .behaviour_mut()
                        .send_response(channel, response)
                        .map_err(|_| NetworkError::Request("response channel closed".into()))?;
                    if authenticated_now {
                        self.maybe_start_sync(peer)?;
                    }
                    Ok(if authenticated_now {
                        NetworkEvent::Authenticated(peer)
                    } else {
                        NetworkEvent::SyncProgress(peer)
                    })
                }
                request_response::Message::Response { response, .. } => {
                    self.handle_response(peer, response)
                }
            },
            request_response::Event::OutboundFailure { error, .. } => {
                Err(NetworkError::Request(error.to_string()))
            }
            request_response::Event::InboundFailure { error, .. } => {
                Err(NetworkError::Request(error.to_string()))
            }
            request_response::Event::ResponseSent { peer, .. } => {
                Ok(NetworkEvent::SyncProgress(peer))
            }
        }
    }

    fn handle_request(&mut self, peer: PeerId, request: NetworkRequest) -> (NetworkResponse, bool) {
        match request {
            NetworkRequest::Authenticate { certificate } => {
                match self.authenticate(peer, &certificate) {
                    Ok(()) => (NetworkResponse::Authenticated, true),
                    Err(reason) => (NetworkResponse::Rejected { reason }, false),
                }
            }
            NetworkRequest::Sync { message } => {
                if !self.authenticated.contains(&peer) {
                    return (
                        NetworkResponse::Rejected {
                            reason: RejectionCode::NotAuthenticated,
                        },
                        false,
                    );
                }
                if matches!(&message, SyncMessage::Want { event_ids, .. } if event_ids.len() != 1) {
                    return (
                        NetworkResponse::Rejected {
                            reason: RejectionCode::FrameLimit,
                        },
                        false,
                    );
                }
                match self.sync.receive(message) {
                    Ok(messages) => match normalize_messages(messages) {
                        Ok(messages) => (NetworkResponse::Sync { messages }, false),
                        Err(reason) => (NetworkResponse::Rejected { reason }, false),
                    },
                    Err(_) => (
                        NetworkResponse::Rejected {
                            reason: RejectionCode::InvalidSync,
                        },
                        false,
                    ),
                }
            }
        }
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
            NetworkResponse::Rejected { reason } => Err(NetworkError::Rejected(reason)),
        }
    }

    fn authenticate(
        &mut self,
        peer: PeerId,
        certificate: &auth::DeviceCertificate,
    ) -> Result<(), RejectionCode> {
        let authenticated = auth::validate_device_certificate(certificate)
            .map_err(|_| RejectionCode::InvalidCertificate)?;
        let certificate_peer = auth::peer_id_from_certificate(certificate)
            .map_err(|_| RejectionCode::InvalidCertificate)?;
        if certificate_peer != peer {
            return Err(RejectionCode::PeerIdMismatch);
        }
        if !self.allowed_users.contains(&authenticated.user_id) {
            return Err(RejectionCode::UserNotAllowed);
        }
        if self
            .revocations
            .contains(authenticated.user_id, authenticated.device_id)
        {
            return Err(RejectionCode::DeviceRevoked);
        }
        self.authenticated.insert(peer);
        Ok(())
    }

    fn send(&mut self, peer: PeerId, request: NetworkRequest) -> Result<(), NetworkError> {
        if serde_json::to_vec(&request)
            .map_err(|error| NetworkError::Request(error.to_string()))?
            .len()
            > MAX_NETWORK_FRAME_BYTES as usize
        {
            return Err(NetworkError::Rejected(RejectionCode::FrameLimit));
        }
        self.swarm.behaviour_mut().send_request(&peer, request);
        Ok(())
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
