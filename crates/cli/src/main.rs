use chatcommons_cli::NodeState;
use chatcommons_crypto::UserId;
use chatcommons_node_core::{CoreNode, MAX_PENDING_EVENTS, NodeError};
use chatcommons_profile_chat::{
    ChatError, ChatPayload, HomeServerBinding, HomeServerId, InviteCapability, InviteError,
    create_chat_event, create_chat_genesis, decode, parse_invite_package, resolve,
};
use chatcommons_protocol::{CommunityId, ProtocolError, community_id};
use chatcommons_storage::{
    EventStore, StorageError,
    archive::{self, ArchiveError},
};
use chatcommons_sync::{
    SyncError, SyncPeer,
    auth::{DeviceId, RevocationSet, create_device_certificate},
    bootstrap::{BootstrapError, create_code, parse_code},
    network::{
        BootstrapGrant, MAX_BOOTSTRAP_ANCESTRY_EVENTS, NetworkError, NetworkEvent, NetworkNode,
    },
};
use libp2p::{Multiaddr, PeerId, identity};
use rand_core::{OsRng, RngCore};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    process::ExitCode,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

const USAGE: &str = r#"ChatCommons M2c-M3d diagnostic node

Usage:
  chatcommons-node init --state <directory>
  chatcommons-node info --state <directory>
  chatcommons-node create-community --state <directory> --name <name>
  chatcommons-node create-channel --state <directory> --community <hex> --name <name>
  chatcommons-node list-channels --state <directory> --community <hex>
  chatcommons-node send-message --state <directory> --community <hex>
    --channel <hex> --text <message>
  chatcommons-node list-messages --state <directory> --community <hex>
    [--channel <hex>]
  chatcommons-node set-home-server --state <directory> --community <hex>
    --server-public-key <hex> --endpoint <multiaddr-or-url> [--endpoint <...> ...]
  chatcommons-node export-community --state <directory> --community <hex>
    --output <archive-file>
  chatcommons-node import-community --state <directory> --input <archive-file>
  chatcommons-node create-invite --state <directory> --community <hex>
    [--address <public-multiaddr>]
  chatcommons-node join --state <directory> --invite-code <code>
  chatcommons-node run --state <directory> --community <hex>
    --listen <multiaddr> [--allow-user <user-id-hex> ...]
    [--relay-address <relay-base-multiaddr>]
    [--dial-peer <peer-id> --dial-address <multiaddr>] [--exit-after-events <count>]
  chatcommons-node serve-community --state <directory> --community <hex>
    --listen <multiaddr> [--relay-address <relay-base-multiaddr>]
    [--max-store-bytes <bytes>]
  chatcommons-node sync-home-server --state <directory> --community <hex>
    --listen <multiaddr> [--relay-address <relay-base-multiaddr>]
    [--exit-after-events <count>] [--idle-timeout-ms <milliseconds>]

This is a developer tool. Relay-assisted hole punching requires an explicit relay.
It has no discovery, production relay configuration, or GUI.
"#;
const MAX_ALLOWED_USERS: usize = 256;
const DEFAULT_HOME_SERVER_STORE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_CHANNEL_NAME_BYTES: usize = 80;
const MAX_MESSAGE_BYTES: usize = 4_000;

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
    #[error("JSON encoding failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("community archive failed: {0}")]
    Archive(#[from] ArchiveError),
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
    #[error("community has no active Home Server declaration")]
    MissingHomeServer,
    #[error("local device key does not match the active Home Server declaration")]
    WrongHomeServerIdentity,
    #[error("Home Server public key is not a valid Ed25519 device key")]
    InvalidHomeServerPublicKey,
    #[error("Home Server declaration has no supported network endpoint")]
    UnsupportedHomeServerEndpoint,
    #[error("community archive could not be fully ingested")]
    IncompleteArchive,
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
        "create-channel" => command_create_channel(&options),
        "list-channels" => command_list_channels(&options),
        "send-message" => command_send_message(&options),
        "list-messages" => command_list_messages(&options),
        "set-home-server" => command_set_home_server(&options),
        "export-community" => command_export_community(&options),
        "import-community" => command_import_community(&options),
        "create-invite" => command_create_invite(&options),
        "join" => command_join(&options).await,
        "run" => command_network(&options, NetworkRole::Peer, DialMode::Explicit).await,
        "serve-community" => {
            command_network(&options, NetworkRole::HomeServer, DialMode::Explicit).await
        }
        "sync-home-server" => {
            command_network(&options, NetworkRole::Peer, DialMode::HomeServer).await
        }
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
    let _lock = state.acquire_lock()?;
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

fn command_create_channel(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community", "--name"])?;
    let name = options.require_one("--name")?.trim();
    if name.is_empty() || name.len() > MAX_CHANNEL_NAME_BYTES {
        return Err(CliError::Arguments(format!(
            "channel name must contain 1 to {MAX_CHANNEL_NAME_BYTES} UTF-8 bytes"
        )));
    }
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let community = parse_community(options.require_one("--community")?)?;
    let mut core = open_community(&state, community)?;
    let mut channel_id = [0_u8; 32];
    OsRng.fill_bytes(&mut channel_id);
    let event = create_chat_event(
        state.user(),
        community,
        core.heads()?,
        now_ms()?,
        ChatPayload::ChannelCreate {
            channel_id,
            name: name.into(),
        },
    )?;
    require_candidate_accepted(&core, &event)?;
    core.ingest(vec![event.clone()])?;
    println!("CHANNEL_ID={}", hex::encode(channel_id));
    println!(
        "CHANNEL_EVENT_ID={}",
        hex::encode(event.event_id.as_bytes())
    );
    io::stdout().flush()?;
    Ok(())
}

fn command_send_message(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community", "--channel", "--text"])?;
    let text = options.require_one("--text")?.trim();
    if text.is_empty() || text.len() > MAX_MESSAGE_BYTES {
        return Err(CliError::Arguments(format!(
            "message must contain 1 to {MAX_MESSAGE_BYTES} UTF-8 bytes"
        )));
    }
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let community = parse_community(options.require_one("--community")?)?;
    let channel_id = parse_hex_32(options.require_one("--channel")?)?;
    let mut core = open_community(&state, community)?;
    let event = create_chat_event(
        state.user(),
        community,
        core.heads()?,
        now_ms()?,
        ChatPayload::MessageCreate {
            channel_id,
            text: text.into(),
        },
    )?;
    require_candidate_accepted(&core, &event)?;
    core.ingest(vec![event.clone()])?;
    println!(
        "MESSAGE_EVENT_ID={}",
        hex::encode(event.event_id.as_bytes())
    );
    io::stdout().flush()?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelView {
    channel_id: String,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageView {
    event_id: String,
    channel_id: String,
    author_id: String,
    timestamp_ms: i64,
    text: String,
}

fn command_list_channels(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let community = parse_community(options.require_one("--community")?)?;
    let core = open_community(&state, community)?;
    let events = core.all_events()?;
    let resolution = resolve(&events)?;
    let accepted = resolution.snapshot.event_ids;
    let channels: Vec<ChannelView> = events
        .iter()
        .filter(|event| accepted.contains(&event.event_id))
        .filter_map(|event| match decode(event) {
            Ok(ChatPayload::ChannelCreate { channel_id, name }) => Some(ChannelView {
                channel_id: hex::encode(channel_id),
                name,
            }),
            _ => None,
        })
        .collect();
    println!("{}", serde_json::to_string(&channels)?);
    io::stdout().flush()?;
    Ok(())
}

fn command_list_messages(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community", "--channel"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let community = parse_community(options.require_one("--community")?)?;
    let selected_channel = options
        .optional_one("--channel")?
        .map(parse_hex_32)
        .transpose()?;
    let core = open_community(&state, community)?;
    let events = core.all_events()?;
    let resolution = resolve(&events)?;
    let by_id: BTreeMap<_, _> = events.iter().map(|event| (event.event_id, event)).collect();
    let mut messages = Vec::new();
    for event_id in resolution.accepted_in_order {
        let event = by_id.get(&event_id).ok_or(CliError::ProfileRejected)?;
        if let ChatPayload::MessageCreate { channel_id, text } = decode(event)? {
            if selected_channel.is_some_and(|selected| selected != channel_id) {
                continue;
            }
            messages.push(MessageView {
                event_id: hex::encode(event.event_id.as_bytes()),
                channel_id: hex::encode(channel_id),
                author_id: hex::encode(chatcommons_protocol::author_id(event)?.as_bytes()),
                timestamp_ms: event.content.timestamp_ms,
                text,
            });
        }
    }
    println!("{}", serde_json::to_string(&messages)?);
    io::stdout().flush()?;
    Ok(())
}

fn open_community(state: &NodeState, community: CommunityId) -> Result<CoreNode, CliError> {
    let store = EventStore::open(state.database_path())?;
    if store.events(community)?.is_empty() {
        return Err(CliError::WrongDatabaseCommunity);
    }
    Ok(CoreNode::open(store, Some(community))?)
}

fn require_candidate_accepted(
    core: &CoreNode,
    candidate: &chatcommons_protocol::SignedEvent,
) -> Result<(), CliError> {
    let mut candidates = core.all_events()?;
    candidates.push(candidate.clone());
    if resolve(&candidates)?
        .snapshot
        .event_ids
        .contains(&candidate.event_id)
    {
        Ok(())
    } else {
        Err(CliError::ProfileRejected)
    }
}

fn command_set_home_server(options: &Options) -> Result<(), CliError> {
    options.allow_only(&[
        "--state",
        "--community",
        "--server-public-key",
        "--endpoint",
    ])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let community = parse_community(options.require_one("--community")?)?;
    let server_public_key = parse_hex_32(options.require_one("--server-public-key")?)?;
    identity::ed25519::PublicKey::try_from_bytes(&server_public_key)
        .map_err(|_| CliError::InvalidHomeServerPublicKey)?;
    let endpoints = options.many("--endpoint").to_vec();
    let store = EventStore::open(state.database_path())?;
    if store.events(community)?.is_empty() {
        return Err(CliError::WrongDatabaseCommunity);
    }
    let mut core = CoreNode::open(store, Some(community))?;
    let declaration = create_chat_event(
        state.user(),
        community,
        core.heads()?,
        now_ms()?,
        ChatPayload::HomeServerSet {
            server_public_key: server_public_key.to_vec(),
            endpoints,
        },
    )?;
    let mut candidates = core.all_events()?;
    candidates.push(declaration.clone());
    let resolution = resolve(&candidates)?;
    let accepted = resolution
        .snapshot
        .home_server
        .as_ref()
        .is_some_and(|binding| {
            binding.declaration == declaration.event_id
                && binding.server_public_key == server_public_key
        });
    if !accepted {
        return Err(CliError::ProfileRejected);
    }
    core.ingest(vec![declaration.clone()])?;
    let server_id = HomeServerId::from_public_key(&server_public_key);
    println!(
        "HOME_SERVER_DECLARATION_ID={}",
        hex::encode(declaration.event_id.as_bytes())
    );
    println!("HOME_SERVER_ID={}", hex::encode(server_id.as_bytes()));
    io::stdout().flush()?;
    Ok(())
}

fn command_export_community(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community", "--output"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let community = parse_community(options.require_one("--community")?)?;
    let store = EventStore::open(state.database_path())?;
    if store.events(community)?.is_empty() {
        return Err(CliError::WrongDatabaseCommunity);
    }
    let core = CoreNode::open(store, Some(community))?;
    let bytes = archive::encode(community, core.all_events()?)?;
    let output = Path::new(options.require_one("--output")?);
    let mut file = create_private_output(output)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!("EXPORTED_COMMUNITY={}", hex::encode(community.as_bytes()));
    println!("EXPORTED_EVENTS={}", core.event_ids().len());
    println!("EXPORTED_BYTES={}", bytes.len());
    io::stdout().flush()?;
    Ok(())
}

fn command_import_community(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--input"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let bytes = read_archive_file(Path::new(options.require_one("--input")?))?;
    let validated = archive::parse(&bytes)?.validate()?;
    let community = validated.community();
    let projected = resolve(validated.events())?;
    if projected.snapshot.community != Some(community) {
        return Err(CliError::ProfileRejected);
    }
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
    let (_, events) = validated.into_parts();
    let (inserted, already_present, core) = ingest_archive(core, events)?;
    let final_projection = resolve(&core.all_events()?)?;
    if final_projection.snapshot.community != Some(community) {
        return Err(CliError::ProfileRejected);
    }
    println!("IMPORTED_COMMUNITY={}", hex::encode(community.as_bytes()));
    println!("IMPORTED_EVENTS={inserted}");
    println!("ALREADY_PRESENT={already_present}");
    io::stdout().flush()?;
    Ok(())
}

fn ingest_archive(
    mut core: CoreNode,
    events: Vec<chatcommons_protocol::SignedEvent>,
) -> Result<(usize, usize, CoreNode), CliError> {
    let already_present = events
        .iter()
        .filter(|event| core.event_ids().contains(&event.event_id))
        .count();
    let mut pending: BTreeMap<_, _> = events
        .into_iter()
        .filter(|event| !core.event_ids().contains(&event.event_id))
        .map(|event| (event.event_id, event))
        .collect();
    let mut remaining_parents = BTreeMap::new();
    let mut children = BTreeMap::<_, Vec<_>>::new();
    for (id, event) in &pending {
        let mut remaining = 0_usize;
        for parent in &event.content.parents {
            if pending.contains_key(parent) {
                remaining += 1;
                children.entry(*parent).or_default().push(*id);
            } else if !core.event_ids().contains(parent) {
                return Err(CliError::IncompleteArchive);
            }
        }
        remaining_parents.insert(*id, remaining);
    }
    let mut ready: BTreeSet<_> = remaining_parents
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut topological = Vec::with_capacity(pending.len());
    while let Some(id) = ready.pop_first() {
        topological.push(id);
        for child in children.remove(&id).unwrap_or_default() {
            let count = remaining_parents
                .get_mut(&child)
                .ok_or(CliError::IncompleteArchive)?;
            *count = (*count).checked_sub(1).ok_or(CliError::IncompleteArchive)?;
            if *count == 0 {
                ready.insert(child);
            }
        }
    }
    if topological.len() != pending.len() {
        return Err(CliError::IncompleteArchive);
    }
    let mut inserted = 0;
    for ids in topological.chunks(MAX_PENDING_EVENTS) {
        let mut batch = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(event) = pending.remove(id) {
                batch.push(event);
            }
        }
        let report = core.ingest(batch)?;
        if !report.unresolved.is_empty() {
            return Err(CliError::IncompleteArchive);
        }
        inserted += report.inserted;
    }
    Ok((inserted, already_present, core))
}

fn read_archive_file(path: &Path) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((archive::MAX_ARCHIVE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > archive::MAX_ARCHIVE_BYTES {
        return Err(CliError::Archive(ArchiveError::TooLarge));
    }
    Ok(bytes)
}

fn create_private_output(path: &Path) -> Result<File, CliError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    Ok(options.open(path)?)
}

fn command_create_invite(options: &Options) -> Result<(), CliError> {
    options.allow_only(&["--state", "--community", "--address"])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let community = parse_community(options.require_one("--community")?)?;
    let store = EventStore::open(state.database_path())?;
    let mut core = CoreNode::open(store, Some(community))?;
    let existing = core.all_events()?;
    let (bootstrap_peer, address) = match options.optional_one("--address")? {
        Some(address) => (state.device().peer_id(), parse_multiaddr(address)?),
        None => {
            let profile = resolve(&existing)?;
            home_server_target(
                profile
                    .snapshot
                    .home_server
                    .as_ref()
                    .ok_or(CliError::MissingHomeServer)?,
            )?
        }
    };
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
    let code = create_code(package, bootstrap_peer, &address)?;
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
    let _lock = state.acquire_lock()?;
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
                let endpoint_device = network
                    .provisional_device(peer)
                    .ok_or(CliError::UntrustedBootstrapEndpoint)?;
                let member_endpoint = resolution.snapshot.members.contains(&endpoint_user);
                let home_server_endpoint =
                    resolution
                        .snapshot
                        .home_server
                        .as_ref()
                        .is_some_and(|binding| {
                            DeviceId::from_public_key(&binding.server_public_key) == endpoint_device
                        });
                if !member_endpoint && !home_server_endpoint {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum NetworkRole {
    Peer,
    HomeServer,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DialMode {
    Explicit,
    HomeServer,
}

async fn command_network(
    options: &Options,
    role: NetworkRole,
    dial_mode: DialMode,
) -> Result<(), CliError> {
    match (role, dial_mode) {
        (NetworkRole::Peer, DialMode::Explicit) => options.allow_only(&[
            "--state",
            "--community",
            "--listen",
            "--relay-address",
            "--allow-user",
            "--dial-peer",
            "--dial-address",
            "--exit-after-events",
            "--idle-timeout-ms",
        ])?,
        (NetworkRole::Peer, DialMode::HomeServer) => options.allow_only(&[
            "--state",
            "--community",
            "--listen",
            "--relay-address",
            "--exit-after-events",
            "--idle-timeout-ms",
        ])?,
        (NetworkRole::HomeServer, DialMode::Explicit) => options.allow_only(&[
            "--state",
            "--community",
            "--listen",
            "--relay-address",
            "--max-store-bytes",
        ])?,
        (NetworkRole::HomeServer, DialMode::HomeServer) => {
            return Err(CliError::Arguments(
                "Home Server role cannot dial itself".into(),
            ));
        }
    }
    let state = NodeState::load(options.require_one("--state")?)?;
    let _lock = state.acquire_lock()?;
    let community = parse_community(options.require_one("--community")?)?;
    let listen = parse_multiaddr(options.require_one("--listen")?)?;
    let relay = options
        .optional_one("--relay-address")?
        .map(parse_multiaddr)
        .transpose()?;
    let explicit_allowed_users = parse_allowed_users(options.many("--allow-user"))?;
    let explicit_dial = if dial_mode == DialMode::Explicit {
        parse_dial(options)?
    } else {
        None
    };
    let exit_after_events = options
        .optional_one("--exit-after-events")?
        .map(parse_positive_usize)
        .transpose()?;
    let idle_timeout = options
        .optional_one("--idle-timeout-ms")?
        .map(parse_positive_milliseconds)
        .transpose()?
        .map(Duration::from_millis);
    let max_store_bytes = if role == NetworkRole::HomeServer {
        Some(
            options
                .optional_one("--max-store-bytes")?
                .map(parse_positive_u64)
                .transpose()?
                .unwrap_or(DEFAULT_HOME_SERVER_STORE_BYTES),
        )
    } else {
        None
    };

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
    let home_server = profile.snapshot.home_server.clone();
    if role == NetworkRole::HomeServer
        && !home_server
            .as_ref()
            .is_some_and(|binding| binding.server_public_key == state.device().public_key())
    {
        return Err(if home_server.is_some() {
            CliError::WrongHomeServerIdentity
        } else {
            CliError::MissingHomeServer
        });
    }
    let dial = match dial_mode {
        DialMode::Explicit => explicit_dial,
        DialMode::HomeServer => Some(home_server_target(
            home_server.as_ref().ok_or(CliError::MissingHomeServer)?,
        )?),
    };
    let mut allowed_users = explicit_allowed_users.clone();
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
    let sync_peer = SyncPeer::new(core, community)?;
    let sync_peer = match max_store_bytes {
        Some(limit) => sync_peer.with_storage_quota(limit)?,
        None => sync_peer,
    };
    let mut network = NetworkNode::new(
        state.device(),
        certificate,
        sync_peer,
        allowed_users.clone(),
        RevocationSet::default(),
    )?;
    let trusted_infrastructure_devices = home_server
        .into_iter()
        .map(|binding| DeviceId::from_public_key(&binding.server_public_key))
        .collect();
    network.replace_authorization(allowed_users, trusted_infrastructure_devices);
    for grant in bootstrap_grants {
        network.register_bootstrap_grant(grant)?;
    }
    let peer_id = network.peer_id();
    println!("USER_ID={}", hex::encode(state.user().user_id().as_bytes()));
    println!("PEER_ID={peer_id}");
    println!("COMMUNITY_ID={}", hex::encode(community.as_bytes()));
    println!(
        "ROLE={}",
        if role == NetworkRole::HomeServer {
            "home-server"
        } else {
            "peer"
        }
    );
    if let Some(limit) = max_store_bytes {
        println!("MAX_STORE_BYTES={limit}");
    }
    network.listen(listen)?;
    if let Some(relay) = relay {
        let route = network.reserve_relay(relay)?;
        println!("RELAY_ROUTE={route}");
    }
    if let Some((peer, address)) = dial {
        network.dial(peer, address)?;
    }
    io::stdout().flush()?;

    let mut synchronization_started = false;
    loop {
        let event = if synchronization_started && idle_timeout.is_some() {
            match tokio::time::timeout(
                idle_timeout
                    .ok_or_else(|| CliError::Arguments("idle timeout is missing".into()))?,
                network.next_event(),
            )
            .await
            {
                Ok(result) => result?,
                Err(_) => {
                    println!(
                        "SYNC_COMPLETE events={}",
                        network.sync_peer().node().event_ids().len()
                    );
                    io::stdout().flush()?;
                    return Ok(());
                }
            }
        } else {
            network.next_event().await?
        };
        let sync_progress = matches!(&event, NetworkEvent::SyncProgress(_));
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
        refresh_network_authorization(
            &mut network,
            &explicit_allowed_users,
            role,
            state.device().public_key(),
        )?;
        if sync_progress {
            synchronization_started = true;
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

fn refresh_network_authorization(
    network: &mut NetworkNode,
    explicit_allowed_users: &BTreeSet<UserId>,
    role: NetworkRole,
    local_device_public_key: [u8; 32],
) -> Result<(), CliError> {
    let profile = resolve(&network.sync_peer().node().all_events()?)?;
    if role == NetworkRole::HomeServer
        && !profile
            .snapshot
            .home_server
            .as_ref()
            .is_some_and(|binding| binding.server_public_key == local_device_public_key)
    {
        return Err(if profile.snapshot.home_server.is_some() {
            CliError::WrongHomeServerIdentity
        } else {
            CliError::MissingHomeServer
        });
    }
    let mut bootstrap_grants = Vec::new();
    for (invitation, capability_public_key) in &profile.snapshot.active_invitations {
        bootstrap_grants.push(BootstrapGrant {
            invitation: *invitation,
            capability_public_key: *capability_public_key,
            ancestry: network
                .sync_peer()
                .node()
                .ancestry(*invitation, MAX_BOOTSTRAP_ANCESTRY_EVENTS)?,
        });
    }
    let mut allowed_users = explicit_allowed_users.clone();
    allowed_users.extend(profile.snapshot.members);
    let trusted_infrastructure_devices = profile
        .snapshot
        .home_server
        .into_iter()
        .map(|binding| DeviceId::from_public_key(&binding.server_public_key))
        .collect();
    network.replace_authorization(allowed_users, trusted_infrastructure_devices);
    network.replace_bootstrap_grants(bootstrap_grants)?;
    Ok(())
}

fn print_identity(state: &NodeState) -> Result<(), CliError> {
    println!("USER_ID={}", hex::encode(state.user().user_id().as_bytes()));
    println!("PEER_ID={}", state.device().peer_id());
    println!(
        "DEVICE_PUBLIC_KEY={}",
        hex::encode(state.device().public_key())
    );
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

fn home_server_target(binding: &HomeServerBinding) -> Result<(PeerId, Multiaddr), CliError> {
    let public_key = identity::ed25519::PublicKey::try_from_bytes(&binding.server_public_key)
        .map_err(|_| CliError::InvalidHomeServerPublicKey)?;
    let peer = identity::PublicKey::from(public_key).to_peer_id();
    let address = binding
        .endpoints
        .iter()
        .find_map(|endpoint| Multiaddr::from_str(endpoint).ok())
        .ok_or(CliError::UnsupportedHomeServerEndpoint)?;
    Ok((peer, address))
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

fn parse_positive_u64(value: &str) -> Result<u64, CliError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| CliError::Arguments("byte count must be a positive integer".into()))?;
    if parsed == 0 {
        return Err(CliError::Arguments(
            "byte count must be greater than zero".into(),
        ));
    }
    Ok(parsed)
}

fn parse_positive_milliseconds(value: &str) -> Result<u64, CliError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| CliError::Arguments("timeout must be a positive integer".into()))?;
    if parsed == 0 {
        return Err(CliError::Arguments(
            "timeout must be greater than zero".into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use chatcommons_crypto::Identity;
    use chatcommons_protocol::{EventContent, PROTOCOL_VERSION, create_genesis, create_signed};

    #[test]
    fn archive_ingest_batches_a_chain_larger_than_the_node_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let identity = Identity::from_seed([91; 32]);
        let genesis = create_genesis(&identity, "batch.genesis", Vec::new(), 1);
        let community = community_id(&genesis)?;
        let mut events = vec![genesis.clone()];
        let mut parent = genesis.event_id;
        for timestamp_ms in 2..=(MAX_PENDING_EVENTS as i64 + 2) {
            let event = create_signed(
                EventContent {
                    protocol_version: PROTOCOL_VERSION,
                    community_id: Some(community),
                    parents: vec![parent],
                    timestamp_ms,
                    event_type: "batch.event".into(),
                    payload: Vec::new(),
                },
                &identity,
            );
            parent = event.event_id;
            events.push(event);
        }
        events.reverse();
        let temporary = tempfile::tempdir()?;
        let core = CoreNode::open(
            EventStore::open(temporary.path().join("events.sqlite3"))?,
            None,
        )?;
        let expected = events.len();
        let (inserted, already_present, core) = ingest_archive(core, events)?;
        assert_eq!(inserted, expected);
        assert_eq!(already_present, 0);
        assert_eq!(core.event_ids().len(), expected);
        Ok(())
    }
}
