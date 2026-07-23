#![cfg(unix)]

use chatcommons_cli::NodeState;
use chatcommons_node_core::CoreNode;
use chatcommons_profile_chat::{
    ChatPayload, InviteCapability, create_chat_event, parse_invite_package, resolve,
};
use chatcommons_protocol::{CommunityId, EventId};
use chatcommons_storage::EventStore;
use libp2p::identity;
use std::{
    io::{BufRead, BufReader, Read},
    net::UdpSocket,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn run_command(arguments: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args(arguments)
        .output()?)
}

fn require_success(output: &Output) -> Result<(), Box<dyn std::error::Error>> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

fn field(output: &Output, name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prefix = format!("{name}=");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .ok_or_else(|| format!("missing {name} in command output").into())
}

fn parse_id(value: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    Ok(hex::decode(value)?
        .try_into()
        .map_err(|_| "identifier has the wrong length")?)
}

fn available_address() -> Result<String, Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let port = socket.local_addr()?.port();
    drop(socket);
    Ok(format!("/ip4/127.0.0.1/udp/{port}/quic-v1"))
}

fn invalid_device_public_key() -> Result<[u8; 32], Box<dyn std::error::Error>> {
    for value in 0_u32..=u32::MAX {
        let mut candidate = [0_u8; 32];
        candidate[..4].copy_from_slice(&value.to_be_bytes());
        if identity::ed25519::PublicKey::try_from_bytes(&candidate).is_err() {
            return Ok(candidate);
        }
    }
    Err("could not construct an invalid Ed25519 public key".into())
}

struct RunningNode {
    child: Child,
    reader: Option<thread::JoinHandle<()>>,
}

impl RunningNode {
    fn spawn(arguments: &[&str]) -> Result<(Self, String), Box<dyn std::error::Error>> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
            .args(arguments)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().ok_or("node stdout was not captured")?;
        let (sender, receiver) = mpsc::channel();
        let drain = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if sender.send(line).is_err() {
                    continue;
                }
            }
        });
        let listen_address = loop {
            match receiver.recv_timeout(Duration::from_secs(10)) {
                Ok(Ok(line)) => {
                    if let Some(address) = line.strip_prefix("LISTEN_ADDRESS=") {
                        break address.to_owned();
                    }
                }
                Ok(Err(error)) => return Err(error.into()),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("node did not start listening: {error}").into());
                }
            }
        };
        Ok((
            Self {
                child,
                reader: Some(drain),
            },
            listen_address,
        ))
    }

    fn has_exited(&mut self) -> Result<bool, std::io::Error> {
        Ok(self.child.try_wait()?.is_some())
    }
}

impl Drop for RunningNode {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn wait_for_exit(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait()?;
            return Err(format!("process timed out and exited with {status}").into());
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn seed_node(
    path: &Path,
    community: CommunityId,
    events: Vec<chatcommons_protocol::SignedEvent>,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = EventStore::open(path.join("events.sqlite3"))?;
    let mut node = CoreNode::open(store, None)?;
    let report = node.ingest(events)?;
    if !report.unresolved.is_empty() || node.community() != Some(community) {
        return Err("failed to seed community history".into());
    }
    Ok(())
}

#[test]
fn declared_home_server_relays_events_between_offline_members()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let owner_path = temporary.path().join("owner");
    let member_path = temporary.path().join("member");
    let server_path = temporary.path().join("server");
    let imposter_path = temporary.path().join("imposter");
    let owner_text = owner_path.to_string_lossy().into_owned();
    let member_text = member_path.to_string_lossy().into_owned();
    let server_text = server_path.to_string_lossy().into_owned();
    let imposter_text = imposter_path.to_string_lossy().into_owned();

    require_success(&run_command(&["init", "--state", &owner_text])?)?;
    require_success(&run_command(&["init", "--state", &member_text])?)?;
    let server_init = run_command(&["init", "--state", &server_text])?;
    require_success(&server_init)?;
    require_success(&run_command(&["init", "--state", &imposter_text])?)?;
    let server_public_key = field(&server_init, "DEVICE_PUBLIC_KEY")?;

    let created = run_command(&[
        "create-community",
        "--state",
        &owner_text,
        "--name",
        "Home Server integration",
    ])?;
    require_success(&created)?;
    let community_text = field(&created, "COMMUNITY_ID")?;
    let community = CommunityId::from_bytes(parse_id(&community_text)?);
    let declared_endpoint = available_address()?;
    let malformed_key = hex::encode(invalid_device_public_key()?);
    let malformed_declaration = run_command(&[
        "set-home-server",
        "--state",
        &owner_text,
        "--community",
        &community_text,
        "--server-public-key",
        &malformed_key,
        "--endpoint",
        &declared_endpoint,
    ])?;
    assert!(!malformed_declaration.status.success());
    assert!(
        String::from_utf8_lossy(&malformed_declaration.stderr)
            .contains("not a valid Ed25519 device key")
    );
    let declaration = run_command(&[
        "set-home-server",
        "--state",
        &owner_text,
        "--community",
        &community_text,
        "--server-public-key",
        &server_public_key,
        "--endpoint",
        &declared_endpoint,
    ])?;
    require_success(&declaration)?;
    assert!(!field(&declaration, "HOME_SERVER_ID")?.is_empty());

    let owner = NodeState::load(&owner_path)?;
    let member = NodeState::load(&member_path)?;
    let owner_store = EventStore::open(owner.database_path())?;
    let mut owner_node = CoreNode::open(owner_store, Some(community))?;
    let channel_id = [42; 32];
    let channel = create_chat_event(
        owner.user(),
        community,
        owner_node.heads()?,
        10,
        ChatPayload::ChannelCreate {
            channel_id,
            name: "general".into(),
        },
    )?;
    owner_node.ingest(vec![channel.clone()])?;
    let capability = InviteCapability::from_seed([211; 32]);
    let invitation = create_chat_event(
        owner.user(),
        community,
        vec![channel.event_id],
        11,
        capability.invitation_payload(),
    )?;
    let package = capability.encode_package(community, invitation.event_id)?;
    let acceptance = parse_invite_package(&package)?
        .validate(&invitation)?
        .create_acceptance(member.user(), vec![invitation.event_id], 12)?;
    owner_node.ingest(vec![invitation, acceptance])?;
    let shared_history = owner_node.all_events()?;
    let shared_count = shared_history.len();
    let profile = resolve(&shared_history)?;
    assert!(profile.snapshot.members.contains(&member.user().user_id()));
    seed_node(&member_path, community, shared_history.clone())?;
    let archive_path = temporary.path().join("community.ccarchive");
    let archive_text = archive_path.to_string_lossy().into_owned();
    let exported = run_command(&[
        "export-community",
        "--state",
        &owner_text,
        "--community",
        &community_text,
        "--output",
        &archive_text,
    ])?;
    require_success(&exported)?;
    assert_eq!(
        std::fs::metadata(&archive_path)?.permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        field(&exported, "EXPORTED_EVENTS")?,
        shared_count.to_string()
    );
    let overwrite = run_command(&[
        "export-community",
        "--state",
        &owner_text,
        "--community",
        &community_text,
        "--output",
        &archive_text,
    ])?;
    assert!(!overwrite.status.success());
    for state in [&server_text, &imposter_text] {
        let imported = run_command(&[
            "import-community",
            "--state",
            state,
            "--input",
            &archive_text,
        ])?;
        require_success(&imported)?;
        assert_eq!(
            field(&imported, "IMPORTED_EVENTS")?,
            shared_count.to_string()
        );
    }
    let repeated_import = run_command(&[
        "import-community",
        "--state",
        &server_text,
        "--input",
        &archive_text,
    ])?;
    require_success(&repeated_import)?;
    assert_eq!(field(&repeated_import, "IMPORTED_EVENTS")?, "0");
    assert_eq!(
        field(&repeated_import, "ALREADY_PRESENT")?,
        shared_count.to_string()
    );

    let wrong_server = run_command(&[
        "serve-community",
        "--state",
        &imposter_text,
        "--community",
        &community_text,
        "--listen",
        &available_address()?,
    ])?;
    assert!(!wrong_server.status.success());
    assert!(
        String::from_utf8_lossy(&wrong_server.stderr)
            .contains("does not match the active Home Server")
    );

    let (mut imposter, imposter_address) = RunningNode::spawn(&[
        "run",
        "--state",
        &imposter_text,
        "--community",
        &community_text,
        "--listen",
        &available_address()?,
    ])?;
    let (mut rejected_client, _) = RunningNode::spawn(&[
        "run",
        "--state",
        &owner_text,
        "--community",
        &community_text,
        "--listen",
        "/ip4/127.0.0.1/udp/0/quic-v1",
        "--dial-peer",
        &NodeState::load(&imposter_path)?
            .device()
            .peer_id()
            .to_string(),
        "--dial-address",
        &imposter_address,
    ])?;
    let rejection_deadline = Instant::now() + Duration::from_secs(10);
    while !imposter.has_exited()? {
        if rejected_client.has_exited()? {
            return Err("client exited while evaluating the untrusted server".into());
        }
        if Instant::now() >= rejection_deadline {
            return Err("client did not reject the undeclared server device".into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    drop(imposter);
    drop(rejected_client);

    let member_store = EventStore::open(member.database_path())?;
    let mut member_node = CoreNode::open(member_store, Some(community))?;
    let message = create_chat_event(
        member.user(),
        community,
        member_node.heads()?,
        13,
        ChatPayload::MessageCreate {
            channel_id,
            text: "stored while the owner is offline".into(),
        },
    )?;
    member_node.ingest(vec![message.clone()])?;
    let expected_count = shared_count + 1;

    let (mut server, _) = RunningNode::spawn(&[
        "serve-community",
        "--state",
        &server_text,
        "--community",
        &community_text,
        "--listen",
        &declared_endpoint,
        "--max-store-bytes",
        "1048576",
    ])?;
    let mut uploader = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "sync-home-server",
            "--state",
            &member_text,
            "--community",
            &community_text,
            "--listen",
            "/ip4/127.0.0.1/udp/0/quic-v1",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let server_observer = EventStore::open(server_path.join("events.sqlite3"))?;
    let upload_deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if server_observer.get(message.event_id)?.is_some() {
            break;
        }
        if uploader.try_wait()?.is_some() || server.has_exited()? {
            return Err("uploader or Home Server exited before persistence".into());
        }
        if Instant::now() >= upload_deadline {
            return Err("Home Server did not persist the member event".into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    let _ = uploader.kill();
    let _ = uploader.wait();

    let mut downloader = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "sync-home-server",
            "--state",
            &owner_text,
            "--community",
            &community_text,
            "--listen",
            "/ip4/127.0.0.1/udp/0/quic-v1",
            "--exit-after-events",
            &expected_count.to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let status = wait_for_exit(&mut downloader, Duration::from_secs(15))?;
    let mut stdout = String::new();
    let mut stderr = String::new();
    downloader
        .stdout
        .take()
        .ok_or("downloader stdout was not captured")?
        .read_to_string(&mut stdout)?;
    downloader
        .stderr
        .take()
        .ok_or("downloader stderr was not captured")?
        .read_to_string(&mut stderr)?;
    if !status.success() {
        return Err(format!("download failed: {stderr}\n{stdout}").into());
    }
    assert!(stdout.contains(&format!("SYNC_COMPLETE events={expected_count}")));
    assert!(
        EventStore::open(owner_path.join("events.sqlite3"))?
            .get(EventId::from_bytes(*message.event_id.as_bytes()))?
            .is_some()
    );

    let newcomer_path = temporary.path().join("newcomer");
    let newcomer_text = newcomer_path.to_string_lossy().into_owned();
    require_success(&run_command(&["init", "--state", &newcomer_text])?)?;
    let invite = run_command(&[
        "create-invite",
        "--state",
        &owner_text,
        "--community",
        &community_text,
    ])?;
    require_success(&invite)?;
    let invitation_id = EventId::from_bytes(parse_id(&field(&invite, "INVITATION_ID")?)?);
    let invite_code = field(&invite, "INVITE_CODE")?;
    let uploaded_invite = run_command(&[
        "sync-home-server",
        "--state",
        &owner_text,
        "--community",
        &community_text,
        "--listen",
        "/ip4/127.0.0.1/udp/0/quic-v1",
        "--idle-timeout-ms",
        "500",
    ])?;
    require_success(&uploaded_invite)?;
    assert!(String::from_utf8_lossy(&uploaded_invite.stdout).contains("SYNC_COMPLETE"));
    let invite_deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if server_observer.get(invitation_id)?.is_some() {
            break;
        }
        if server.has_exited()? {
            return Err("Home Server exited before persisting the invitation".into());
        }
        if Instant::now() >= invite_deadline {
            return Err("Home Server did not persist the invitation".into());
        }
        thread::sleep(Duration::from_millis(25));
    }

    let joined = run_command(&[
        "join",
        "--state",
        &newcomer_text,
        "--invite-code",
        &invite_code,
    ])?;
    require_success(&joined)?;
    assert!(String::from_utf8_lossy(&joined.stdout).contains("JOIN_COMPLETE"));
    let newcomer = NodeState::load(&newcomer_path)?;
    let newcomer_events = EventStore::open(newcomer.database_path())?.events(community)?;
    assert!(
        resolve(&newcomer_events)?
            .snapshot
            .members
            .contains(&newcomer.user().user_id())
    );
    assert!(!server.has_exited()?);
    Ok(())
}
