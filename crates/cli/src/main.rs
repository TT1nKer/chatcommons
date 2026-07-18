use chatcommons_cli::NodeState;
use chatcommons_crypto::UserId;
use chatcommons_node_core::{CoreNode, NodeError};
use chatcommons_profile_chat::{ChatError, create_chat_genesis};
use chatcommons_protocol::{CommunityId, ProtocolError, community_id};
use chatcommons_storage::{EventStore, StorageError};
use chatcommons_sync::{
    SyncError, SyncPeer,
    auth::{RevocationSet, create_device_certificate},
    network::{NetworkError, NetworkEvent, NetworkNode},
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

const USAGE: &str = r#"ChatCommons M2c diagnostic node

Usage:
  chatcommons-node init --state <directory>
  chatcommons-node info --state <directory>
  chatcommons-node create-community --state <directory> --name <name>
  chatcommons-node run --state <directory> --community <hex>
    --listen <multiaddr> --allow-user <user-id-hex> [--allow-user <hex> ...]
    [--dial-peer <peer-id> --dial-address <multiaddr>] [--exit-after-events <count>]

This is a developer tool. It has no discovery, NAT traversal, relay, or secure
invitation bootstrap. Exchange UserId, PeerId, and address out of band.
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

async fn command_run(options: &Options) -> Result<(), CliError> {
    options.allow_only(&[
        "--state",
        "--community",
        "--listen",
        "--allow-user",
        "--dial-peer",
        "--dial-address",
        "--exit-after-events",
    ])?;
    let state = NodeState::load(options.require_one("--state")?)?;
    let community = parse_community(options.require_one("--community")?)?;
    let listen = parse_multiaddr(options.require_one("--listen")?)?;
    let allowed_users = parse_allowed_users(options.many("--allow-user"))?;
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
    let certificate =
        create_device_certificate(state.user(), state.device(), state.created_at_ms());
    let mut network = NetworkNode::new(
        state.device(),
        certificate,
        SyncPeer::new(core, community)?,
        allowed_users,
        RevocationSet::default(),
    )?;
    let peer_id = network.peer_id();
    println!("USER_ID={}", hex::encode(state.user().user_id().as_bytes()));
    println!("PEER_ID={peer_id}");
    println!("COMMUNITY_ID={}", hex::encode(community.as_bytes()));
    network.listen(listen)?;
    if let Some((peer, address)) = dial {
        network.dial(peer, address)?;
    }
    io::stdout().flush()?;

    loop {
        let event = network.next_event().await?;
        match &event {
            NetworkEvent::Listening(address) => println!("LISTEN_ADDRESS={address}"),
            NetworkEvent::Connected(peer) => println!("CONNECTED={peer}"),
            NetworkEvent::Authenticated(peer) => println!("AUTHENTICATED={peer}"),
            NetworkEvent::SyncProgress(peer) => println!("SYNC_PROGRESS={peer}"),
            NetworkEvent::Disconnected(peer) => println!("DISCONNECTED={peer}"),
        }
        io::stdout().flush()?;
        if matches!(
            event,
            NetworkEvent::Authenticated(_) | NetworkEvent::SyncProgress(_)
        ) && exit_after_events
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
    if values.is_empty() {
        return Err(CliError::Arguments(
            "at least one --allow-user is required".into(),
        ));
    }
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
