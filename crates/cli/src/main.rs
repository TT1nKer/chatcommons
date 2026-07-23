use chatcommons_cli::NodeState;
use chatcommons_crypto::UserId;
use chatcommons_node_core::{CoreNode, NodeError};
use chatcommons_profile_chat::{
    ChatError, InviteCapability, InviteError, create_chat_event, create_chat_genesis,
    parse_invite_package, resolve,
};
use chatcommons_protocol::{CommunityId, ProtocolError, community_id};
use chatcommons_storage::{EventStore, StorageError};
use chatcommons_sync::{
    SyncError, SyncPeer,
    auth::{RevocationSet, create_device_certificate},
    bootstrap::{BootstrapError, create_code, parse_code},
    network::{
        BootstrapGrant, MAX_BOOTSTRAP_ANCESTRY_EVENTS, NetworkError, NetworkEvent, NetworkNode,
    },
};
use libp2p::{Multiaddr, PeerId};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    io::{self, Write},
    process::ExitCode,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const USAGE: &str = r#"ChatCommons M2c-M2e diagnostic node

Usage:
  chatcommons-node init --state <directory>
  chatcommons-node info --state <directory>
  chatcommons-node create-community --state <directory> --name <name>
  chatcommons-node create-invite --state <directory> --community <hex>
    --address <public-multiaddr>
  chatcommons-node join --state <directory> --invite-code <code>
  chatcommons-node run --state <directory> --community <hex>
    --listen <multiaddr> [--allow-user <user-id-hex> ...]
    [--relay-address <relay-base-multiaddr>]
    [--dial-peer <peer-id> --dial-address <multiaddr>] [--exit-after-events <count>]

This is a developer tool. Relay-assisted hole punching requires an explicit relay.
It has no discovery, production relay configuration, or GUI.
"#;
const MAX_ALLOWED_USERS: usize = 256;

#[derive(Debug, Error)]
enum CliError {
    #[error("invalid command line: {0}")]
    Arguments(String),
    #[error("hex value is invalid: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("Peer ID is invalid: {0}")]
    PeerId(String),
    #[error("multiaddress is invalid: {0}")]
    Multiaddr(String),
    #[error("system clock is before the Unix epoch")]
    InvalidSystemTime,
    #[error("state failed: {0}")]
    State(#[from] chatcommons_cli::StateError),
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("node failed: {0}")]
    Node(#[from] NodeError),
    #[error("chat profile failed: {0}")]
    Chat(#[from] ChatError),
    #[error("invitation failed: {0}")]
    Invite(#[from] InviteError),
    #[error("bootstrap failed: {0}")]
    Bootstrap(#[from] BootstrapError),
    #[error("protocol failed: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("sync failed: {0}")]
    Sync(#[from] SyncError),
    #[error("network failed: {0}")]
    Network(#[from] NetworkError),
    #[error("database already contains another or unknown community")]
    WrongDatabaseCommunity,
    #[error("database already contains a community")]
    DatabaseNotEmpty,
    #[error("chat profile did not authorize the requested operation")]
    ProfileRejected,
    #[error("bootstrap response did not contain the expected invitation")]
    MissingInvitation,
    #[error("bootstrap endpoint is not an active community member")]
    UntrustedBootstrapEndpoint,
    #[error("output failed: {0}")]
    Output(#[from] io::Error),
}

#[derive(Default)]
struct Options(BTreeMap<String, Vec<String>>);

impl Options {
    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, CliError> {
        let mut values = BTreeMap::<String, Vec<String>>::new();
        let mut arguments = arguments;
        while let Some(option) = arguments.next() {
            if !option.starts_with("--") {
                return Err(CliError::Arguments(format!(
                    "expected an option, got {option}"
                )));
            }
            let value = arguments
                .next()
                .ok_or_else(|| CliError::Arguments(format!("missing value for {option}")))?;
            values.entry(option).or_default().push(value);
        }
        Ok(Self(values))
    }

    fn require_one(&self, name: &str) -> Result<&str, CliError> {
        match self.0.get(name).map(Vec::as_slice) {
            Some([value]) => Ok(value),
            Some(_) => Err(CliError::Arguments(format!(
                "{name} must be provided exactly once"
            ))),
            None => Err(CliError::Arguments(format!("missing {name}"))),
        }
    }

    fn optional_one(&self, name: &str) -> Result<Option<&str>, CliError> {
        match self.0.get(name).map(Vec::as_slice) {
            Some([value]) => Ok(Some(value)),
            Some(_) => Err(CliError::Arguments(format!(
                "{name} may be provided only once"
            ))),
            None => Ok(None),
        }
    }

    fn many(&self, name: &str) -> &[String] {
        self.0.get(name).map(Vec::as_slice).unwrap_or_default()
    }

    fn allow_only(&self, allowed: &[&str]) -> Result<(), CliError> {
        if let Some(unknown) = self.0.keys().find(|name| !allowed.contains(&name.as_str())) {
            return Err(CliError::Arguments(format!("unknown option {unknown}")));
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), CliError> {
    let mut arguments = env::args().skip(1);
    let Some(command) = arguments.next() else {
        print!("{USAGE}");
        return Ok(());
    };
    if command == "--help" || command == "help" {
        print!("{USAGE}");
        return Ok(());
    }
    let options = Options::parse(arguments)?;
    match command.as_str() {
        "init" => command_init(&options),
        "info" => command_info(&options),
        "create-community" => command_create_community(&options),
        "create-invite" => command_create_invite(&options),
        "join" => command_join(&options).await,
        "run" => command_run(&options).await,
        _ => Err(CliError::Arguments(format!("unknown command {command}"))),
    }
}

fn command_init(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state"])?;
    let state = NodeState::initialize(options.require_one("--state")?)?;
    print_identity(&state)
}

fn command_info(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    print_identity(&state)
}

fn command_create_community(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--name"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let store = EventStore::open(state.database_path())?;
    if !store.is_empty()? {
        return Err(CliError::DatabaseNotEmpty);
    }
    let genesis = create_chat_genesis(state.user(), options.require_one("--name")?, now_ms()?)?;
    let community = community_id(&genesis)?;
    let mut node = CoreNode::open(store, None)?;
    node.ingest(vec![genesis])?;
    println!("COMMUNITY_ID={}", hex::encode(community.as_bytes()));
    Ok(())
}

fn command_create_invite(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community", "--address"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let community = parse_community(options.require_one("--community")?)?;
    let address = parse_multiaddr(options.require_one("--address")?)?;
    let store = EventStore::open(state.database_path())?;
    let mut core = CoreNode::open(store, Some(community))?;
    let existing = core.all_events()?;
    let capability = InviteCapability::generate();
    let invitation = create_chat_event(
        state.user(),
        community,
        core.heads()?,
        now_ms()?,
        capability.invitation_payload(),
    )?;
    let mut candidates = existing;
    candidates.push(invitation.clone());
    let resolution = resolve(&candidates)?;
    if !resolution.snapshot.event_ids.contains(&invitation.event_id)
        || !resolution
            .snapshot
            .active_invitations
            .contains_key(&invitation.event_id)
    {
        return Err(CliError::ProfileRejected);
    }
    let package = capability.encode_package(community, invitation.event_id)?;
    let code = create_code(package, state.device().peer_id(), &address)?;
    core.ingest(vec![invitation.clone()])?;
    println!(
        "INVITATION_ID={}",
        hex::encode(invitation.event_id.as_bytes())
    );
    println!("INVITE_CODE={code}");
    io::stdout().flush()?;
    Ok(())
}

async fn command_join(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--invite-code"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let envelope = parse_code(options.require_one("--invite-code")?)?.validate()?;
    let prepared = parse_invite_package(envelope.invite_package())?.prepare()?;
    let community = prepared.community();
    let invitation = prepared.invitation();
    let store = EventStore::open(state.database_path())?;
    if !store.is_empty()? {
        return Err(CliError::DatabaseNotEmpty);
    }
    let core = CoreNode::open(store, None)?;
    let certificate =
        create_device_certificate(state.user(), state.device(), state.created_at_ms());
    let mut network = NetworkNode::new(
        state.device(),
        certificate,
        SyncPeer::new(core, community)?,
        BTreeSet::new(),
        RevocationSet::default(),
    )?;
    let peer = envelope.peer_id();
    network.configure_bootstrap_target(peer, invitation)?;
    network.listen(parse_multiaddr("/ip4/0.0.0.0/udp/0/quic-v1")?)?;
    network.dial(peer, envelope.address().clone())?;
    println!("COMMUNITY_ID={}", hex::encode(community.as_bytes()));
    println!("BOOTSTRAP_PEER={peer}");
    io::stdout().flush()?;

    let mut prepared = Some(prepared);
    loop {
        match network.next_event().await? {
            NetworkEvent::Connected { peer, relayed } => {
                println!(
                    "CONNECTED={peer} via={}",
                    if relayed { "relay" } else { "direct" }
                )
            }
            NetworkEvent::RelayConnected(peer) => println!("RELAY_CONNECTED={peer}"),
            NetworkEvent::RelayDisconnected(peer) => println!("RELAY_DISCONNECTED={peer}"),
            NetworkEvent::RelayReservationAccepted(peer) => {
                println!("RELAY_RESERVATION_ACCEPTED={peer}")
            }
            NetworkEvent::RelayCircuitEstablished { relay, remote } => {
                println!("RELAY_CIRCUIT_ESTABLISHED relay={relay:?} remote={remote:?}")
            }
            NetworkEvent::RelayConnectionFailed { relay, reason } => {
                return Err(CliError::Network(NetworkError::Request(format!(
                    "relay {relay:?} failed: {reason}"
                ))));
            }
            NetworkEvent::ObservedAddress { peer, address } => {
                println!("OBSERVED_ADDRESS={address} observer={peer}")
            }
            NetworkEvent::HolePunchSucceeded(peer) => {
                println!("HOLE_PUNCH_SUCCEEDED={peer}")
            }
            NetworkEvent::HolePunchFailed { peer, reason } => {
                println!("HOLE_PUNCH_FAILED={peer} reason={reason}; RELAY_FALLBACK={peer}")
            }
            NetworkEvent::Authenticated(peer) => println!("AUTHENTICATED={peer}"),
            NetworkEvent::SyncProgress(peer) => println!("SYNC_PROGRESS={peer}"),
            NetworkEvent::BootstrapChallenge {
                peer,
                invitation,
                proof_bytes,
            } => {
                let signature = prepared
                    .as_ref()
                    .ok_or(CliError::MissingInvitation)?
                    .sign_possession_proof(&proof_bytes)
                    .to_vec();
                network.submit_bootstrap_proof(peer, invitation, signature)?;
                println!("BOOTSTRAP_PROOF_SENT={peer}");
            }
            NetworkEvent::BootstrapAncestry {
                peer,
                invitation,
                events,
            } => {
                if prepared.is_none() {
                    println!("DUPLICATE_BOOTSTRAP_ANCESTRY={peer}");
                    io::stdout().flush()?;
                    continue;
                }
                let invitation_event = events
                    .iter()
                    .find(|event| event.event_id == invitation)
                    .ok_or(CliError::MissingInvitation)?;
                let prepared_invitation = prepared.take().ok_or(CliError::MissingInvitation)?;
                let validated = prepared_invitation.validate(invitation_event)?;
                let resolution = resolve(&events)?;
                if !resolution
                    .snapshot
                    .active_invitations
                    .contains_key(&invitation)
                {
                    return Err(CliError::ProfileRejected);
                }
                let endpoint_user = network
                    .provisional_user(peer)
                    .ok_or(CliError::UntrustedBootstrapEndpoint)?;
                if !resolution.snapshot.members.contains(&endpoint_user) {
                    return Err(CliError::UntrustedBootstrapEndpoint);
                }
                network.approve_bootstrap_endpoint(peer)?;
                network.sync_peer_mut().node_mut().ingest(events.clone())?;
                let acceptance =
                    validated.create_acceptance(state.user(), vec![invitation], now_ms()?)?;
                let mut with_acceptance = events;
                with_acceptance.push(acceptance.clone());
                let accepted = resolve(&with_acceptance)?;
                if !accepted.snapshot.event_ids.contains(&acceptance.event_id)
                    || !accepted.snapshot.members.contains(&state.user().user_id())
                {
                    return Err(CliError::ProfileRejected);
                }
                network
                    .sync_peer_mut()
                    .node_mut()
                    .ingest(vec![acceptance.clone()])?;
                network.submit_bootstrap_acceptance(peer, invitation, acceptance)?;
                println!("BOOTSTRAP_ACCEPTANCE_SENT={peer}");
            }
            NetworkEvent::BootstrapAccepted(peer) => {
                println!("JOIN_COMPLETE={peer}");
                io::stdout().flush()?;
                return Ok(());
            }
            NetworkEvent::Disconnected(peer) => println!("DISCONNECTED={peer}"),
            NetworkEvent::RequestFailed { reason, .. } => {
                return Err(CliError::Network(NetworkError::Request(reason)));
            }
            NetworkEvent::Listening(address) => println!("LISTEN_ADDRESS={address}"),
            NetworkEvent::BootstrapAcceptance { .. } => {
                return Err(CliError::ProfileRejected);
            }
        }
        io::stdout().flush()?;
    }
}

async fn command_run(options: &Options) -> Result<(), CliError> {
    options.allow_only(&[
        "--state",
        "--community",
        "--listen",
        "--relay-address",
        "--allow-user",
        "--dial-peer",
        "--dial-address",
        "--exit-after-events",
    ])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let community = parse_community(options.require_one("--community")?)?;
    let listen = parse_multiaddr(options.require_one("--listen")?)?;
    let relay = options
        .optional_one("--relay-address")?
        .map(parse_multiaddr)
        .transpose()?;
    let mut allowed_users = parse_allowed_users(options.many("--allow-user"))?;
    let dial = parse_dial(options)?;
    let exit_after_events = options
        .optional_one("--exit-after-events")?
        .map(parse_positive_usize)
        .transpose()?;

    let store = EventStore::open(state.database_path())?;
    let database_is_empty = store.is_empty()?;
    if !database_is_empty && store.events(community)?.is_empty() {
        return Err(CliError::WrongDatabaseCommunity);
    }
    let core = CoreNode::open(
        store,
        if database_is_empty {
            None
        } else {
            Some(community)
        },
    )?;
    let profile = resolve(&core.all_events()?)?;
    allowed_users.extend(profile.snapshot.members.iter().copied());
    let mut bootstrap_grants = Vec::new();
    for (invitation, capability_public_key) in profile.snapshot.active_invitations {
        bootstrap_grants.push(BootstrapGrant {
            invitation,
            capability_public_key,
            ancestry: core.ancestry(invitation, MAX_BOOTSTRAP_ANCESTRY_EVENTS)?,
        });
    }
    let certificate =
        create_device_certificate(state.user(), state.device(), state.created_at_ms());
    let mut network = NetworkNode::new(
        state.device(),
        certificate,
        SyncPeer::new(core, community)?,
        allowed_users,
        RevocationSet::default(),
    )?;
    for grant in bootstrap_grants {
        network.register_bootstrap_grant(grant)?;
    }
    let peer_id = network.peer_id();
    println!("USER_ID={}", hex::encode(state.user().user_id().as_bytes()));
    println!("PEER_ID={peer_id}");
    println!("COMMUNITY_ID={}", hex::encode(community.as_bytes()));
    network.listen(listen)?;
    if let Some(relay) = relay {
        let route = network.reserve_relay(relay)?;
        println!("RELAY_ROUTE={route}");
    }
    if let Some((peer, address)) = dial {
        network.dial(peer, address)?;
    }
    io::stdout().flush()?;

    loop {
        let event = network.next_event().await?;
        let sync_progress = matches!(
            &event,
            NetworkEvent::Authenticated(_) | NetworkEvent::SyncProgress(_)
        );
        match event {
            NetworkEvent::Listening(address) => println!("LISTEN_ADDRESS={address}"),
            NetworkEvent::Connected { peer, relayed } => {
                println!(
                    "CONNECTED={peer} via={}",
                    if relayed { "relay" } else { "direct" }
                )
            }
            NetworkEvent::RelayConnected(peer) => println!("RELAY_CONNECTED={peer}"),
            NetworkEvent::RelayDisconnected(peer) => println!("RELAY_DISCONNECTED={peer}"),
            NetworkEvent::RelayReservationAccepted(peer) => {
                println!("RELAY_RESERVATION_ACCEPTED={peer}")
            }
            NetworkEvent::RelayCircuitEstablished { relay, remote } => {
                println!("RELAY_CIRCUIT_ESTABLISHED relay={relay:?} remote={remote:?}")
            }
            NetworkEvent::RelayConnectionFailed { relay, reason } => {
                println!("RELAY_CONNECTION_FAILED={relay:?} reason={reason}")
            }
            NetworkEvent::ObservedAddress { peer, address } => {
                println!("OBSERVED_ADDRESS={address} observer={peer}")
            }
            NetworkEvent::HolePunchSucceeded(peer) => {
                println!("HOLE_PUNCH_SUCCEEDED={peer}")
            }
            NetworkEvent::HolePunchFailed { peer, reason } => {
                println!("HOLE_PUNCH_FAILED={peer} reason={reason}");
                println!("RELAY_FALLBACK={peer}");
            }
            NetworkEvent::Authenticated(peer) => println!("AUTHENTICATED={peer}"),
            NetworkEvent::SyncProgress(peer) => println!("SYNC_PROGRESS={peer}"),
            NetworkEvent::Disconnected(peer) => println!("DISCONNECTED={peer}"),
            NetworkEvent::RequestFailed { peer, reason } => {
                println!("REQUEST_FAILED={peer} reason={reason}")
            }
            NetworkEvent::BootstrapChallenge { peer, .. } => {
                println!("UNEXPECTED_BOOTSTRAP_CHALLENGE={peer}")
            }
            NetworkEvent::BootstrapAncestry { peer, .. } => {
                println!("UNEXPECTED_BOOTSTRAP_ANCESTRY={peer}")
            }
            NetworkEvent::BootstrapAccepted(peer) => {
                println!("UNEXPECTED_BOOTSTRAP_ACCEPTED={peer}")
            }
            NetworkEvent::BootstrapAcceptance {
                peer,
                user_id,
                invitation,
                acceptance,
            } => {
                let mut candidates = network.sync_peer().node().all_events()?;
                candidates.push(acceptance.as_ref().clone());
                let resolution = resolve(&candidates)?;
                let approved = resolution.snapshot.event_ids.contains(&acceptance.event_id)
                    && resolution.snapshot.members.contains(&user_id)
                    && !resolution
                        .snapshot
                        .active_invitations
                        .contains_key(&invitation);
                if approved {
                    network
                        .sync_peer_mut()
                        .node_mut()
                        .ingest(vec![*acceptance])?;
                }
                network.resolve_bootstrap_acceptance(peer, approved)?;
                println!("BOOTSTRAP_DECISION={peer} approved={approved}");
            }
        }
        io::stdout().flush()?;
        if sync_progress
            && exit_after_events
                .is_some_and(|count| network.sync_peer().node().event_ids().len() >= count)
        {
            println!(
                "SYNC_COMPLETE events={}",
                network.sync_peer().node().event_ids().len()
            );
            io::stdout().flush()?;
            return Ok(());
        }
    }
}

fn print_identity(state: &NodeState) -> Result<(), CliError> {
    println!("USER_ID={}", hex::encode(state.user().user_id().as_bytes()));
    println!("PEER_ID={}", state.device().peer_id());
    println!("STATE_CREATED_AT_MS={}", state.created_at_ms());
    io::stdout().flush()?;
    Ok(())
}

fn parse_allowed_users(values: &[String]) -> Result<BTreeSet<UserId>, CliError> {
    if values.len() > MAX_ALLOWED_USERS {
        return Err(CliError::Arguments(format!(
            "at most {MAX_ALLOWED_USERS} --allow-user values are accepted"
        )));
    }
    values.iter().map(|value| parse_user(value)).collect()
}

fn parse_user(value: &str) -> Result<UserId, CliError> {
    Ok(UserId::from_bytes(parse_hex_32(value)?))
}

fn parse_community(value: &str) -> Result<CommunityId, CliError> {
    Ok(CommunityId::from_bytes(parse_hex_32(value)?))
}

fn parse_hex_32(value: &str) -> Result<[u8; 32], CliError> {
    if value.len() != 64 {
        return Err(CliError::Arguments(
            "expected exactly 32 bytes of hex".into(),
        ));
    }
    let bytes = hex::decode(value)?;
    bytes
        .try_into()
        .map_err(|_| CliError::Arguments("expected exactly 32 bytes of hex".into()))
}

fn parse_multiaddr(value: &str) -> Result<Multiaddr, CliError> {
    Multiaddr::from_str(value).map_err(|error| CliError::Multiaddr(error.to_string()))
}

fn parse_dial(options: &Options) -> Result<Option<(PeerId, Multiaddr)>, CliError> {
    let peer = options.optional_one("--dial-peer")?;
    let address = options.optional_one("--dial-address")?;
    match (peer, address) {
        (None, None) => Ok(None),
        (Some(peer), Some(address)) => Ok(Some((
            PeerId::from_str(peer).map_err(|error| CliError::PeerId(error.to_string()))?,
            parse_multiaddr(address)?,
        ))),
        _ => Err(CliError::Arguments(
            "--dial-peer and --dial-address must be supplied together".into(),
        )),
    }
}

fn parse_positive_usize(value: &str) -> Result<usize, CliError> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| CliError::Arguments("event count must be a positive integer".into()))?;
    if parsed == 0 {
        return Err(CliError::Arguments(
            "event count must be greater than zero".into(),
        ));
    }
    Ok(parsed)
}

fn now_ms() -> Result<i64, CliError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CliError::InvalidSystemTime)?;
    i64::try_from(duration.as_millis()).map_err(|_| CliError::InvalidSystemTime)
}
