#![cfg(unix)]

use chatcommons_crypto::UserId;
use chatcommons_node_core::CoreNode;
use chatcommons_profile_chat::{InviteCapability, resolve};
use chatcommons_protocol::{CommunityId, EventId};
use chatcommons_storage::EventStore;
use chatcommons_sync::bootstrap::create_code;
use libp2p::{Multiaddr, PeerId};
use std::{
    io::{self, BufRead, BufReader, Read},
    net::UdpSocket,
    process::{Child, Command, Output, Stdio},
    str::FromStr,
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

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn line_channel(
    stdout: impl Read + Send + 'static,
) -> (mpsc::Receiver<io::Result<String>>, thread::JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    (receiver, reader)
}

fn wait_for_field(
    receiver: &mpsc::Receiver<io::Result<String>>,
    name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let prefix = format!("{name}=");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!("timed out waiting for {name}").into());
        }
        match receiver.recv_timeout(remaining) {
            Ok(Ok(line)) => {
                if let Some(value) = line.strip_prefix(&prefix) {
                    return Ok(value.to_owned());
                }
            }
            Ok(Err(error)) => return Err(error.into()),
            Err(error) => return Err(format!("failed waiting for {name}: {error}").into()),
        }
    }
}

#[test]
fn invite_code_bootstraps_a_new_member_over_quic() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let source_path = temporary.path().join("source");
    let target_path = temporary.path().join("target");
    let source_text = source_path.to_string_lossy().into_owned();
    let target_text = target_path.to_string_lossy().into_owned();

    let source_init = run_command(&["init", "--state", &source_text])?;
    require_success(&source_init)?;
    let source_peer = PeerId::from_str(&field(&source_init, "PEER_ID")?)?;
    let target_init = run_command(&["init", "--state", &target_text])?;
    require_success(&target_init)?;
    let target_user = UserId::from_bytes(parse_id(&field(&target_init, "USER_ID")?)?);
    let created = run_command(&[
        "create-community",
        "--state",
        &source_text,
        "--name",
        "M2d invite test",
    ])?;
    require_success(&created)?;
    let community_text = field(&created, "COMMUNITY_ID")?;
    let community = CommunityId::from_bytes(parse_id(&community_text)?);

    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let port = socket.local_addr()?.port();
    drop(socket);
    let address = format!("/ip4/127.0.0.1/udp/{port}/quic-v1");
    let invitation = run_command(&[
        "create-invite",
        "--state",
        &source_text,
        "--community",
        &community_text,
        "--address",
        &address,
    ])?;
    require_success(&invitation)?;
    let invite_code = field(&invitation, "INVITE_CODE")?;
    let invitation_id = EventId::from_bytes(parse_id(&field(&invitation, "INVITATION_ID")?)?);

    let mut source = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "run",
            "--state",
            &source_text,
            "--community",
            &community_text,
            "--listen",
            &address,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let source_stdout = source
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("source stdout was not captured"))?;
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(source_stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    loop {
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(line)) if line.starts_with("LISTEN_ADDRESS=") => break,
            Ok(Ok(_)) => {}
            Ok(Err(error)) => {
                stop(&mut source);
                return Err(error.into());
            }
            Err(error) => {
                stop(&mut source);
                return Err(format!("source did not start listening: {error}").into());
            }
        }
    }

    let wrong_path = temporary.path().join("wrong-capability");
    let wrong_text = wrong_path.to_string_lossy().into_owned();
    require_success(&run_command(&["init", "--state", &wrong_text])?)?;
    let wrong_capability = InviteCapability::from_seed([201; 32]);
    let wrong_package = wrong_capability.encode_package(community, invitation_id)?;
    let wrong_code = create_code(wrong_package, source_peer, &address.parse::<Multiaddr>()?)?;
    let wrong_join = run_command(&["join", "--state", &wrong_text, "--invite-code", &wrong_code])?;
    assert!(!wrong_join.status.success());
    assert!(String::from_utf8_lossy(&wrong_join.stderr).contains("InvalidBootstrap"));

    let mut target = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "join",
            "--state",
            &target_text,
            "--invite-code",
            &invite_code,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = target.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            stop(&mut target);
            stop(&mut source);
            return Err("invite bootstrap did not finish".into());
        }
        thread::sleep(Duration::from_millis(25));
    };
    let mut target_stdout = String::new();
    let mut target_stderr = String::new();
    target
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("target stdout was not captured"))?
        .read_to_string(&mut target_stdout)?;
    target
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("target stderr was not captured"))?
        .read_to_string(&mut target_stderr)?;
    if !status.success() {
        stop(&mut source);
        return Err(format!("join failed: {target_stderr}\n{target_stdout}").into());
    }
    assert!(target_stdout.contains("JOIN_COMPLETE="));

    let replay_path = temporary.path().join("replay");
    let replay_text = replay_path.to_string_lossy().into_owned();
    require_success(&run_command(&["init", "--state", &replay_text])?)?;
    let replay = run_command(&[
        "join",
        "--state",
        &replay_text,
        "--invite-code",
        &invite_code,
    ])?;
    assert!(!replay.status.success());
    let replay_error = String::from_utf8_lossy(&replay.stderr);
    assert!(
        replay_error.contains("InvitationUnavailable"),
        "unexpected replay error: {replay_error}"
    );

    stop(&mut source);
    drop(receiver);
    let _ = reader.join();

    let source_core = CoreNode::open(
        EventStore::open(source_path.join("events.sqlite3"))?,
        Some(community),
    )?;
    let target_core = CoreNode::open(
        EventStore::open(target_path.join("events.sqlite3"))?,
        Some(community),
    )?;
    assert_eq!(source_core.event_ids(), target_core.event_ids());
    assert_eq!(source_core.event_ids().len(), 3);
    let profile = resolve(&source_core.all_events()?)?;
    assert!(profile.snapshot.members.contains(&target_user));
    assert!(profile.snapshot.active_invitations.is_empty());
    Ok(())
}

#[test]
fn invite_bootstraps_through_relay() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let source_path = temporary.path().join("relay-source");
    let target_path = temporary.path().join("relay-target");
    let source_text = source_path.to_string_lossy().into_owned();
    let target_text = target_path.to_string_lossy().into_owned();

    require_success(&run_command(&["init", "--state", &source_text])?)?;
    require_success(&run_command(&["init", "--state", &target_text])?)?;
    let created = run_command(&[
        "create-community",
        "--state",
        &source_text,
        "--name",
        "M2e relay invite test",
    ])?;
    require_success(&created)?;
    let community_text = field(&created, "COMMUNITY_ID")?;

    let socket = UdpSocket::bind("127.0.0.1:0")?;
    let relay_port = socket.local_addr()?.port();
    drop(socket);
    let relay_listen = format!("/ip4/127.0.0.1/udp/{relay_port}/quic-v1");
    let mut relay = Command::new(env!("CARGO_BIN_EXE_chatcommons-relay"))
        .args(["--listen", &relay_listen])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let relay_stdout = relay
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("relay stdout was not captured"))?;
    let (relay_receiver, relay_reader) = line_channel(relay_stdout);
    let relay_peer = wait_for_field(&relay_receiver, "RELAY_PEER_ID")?;
    let relay_address = wait_for_field(&relay_receiver, "RELAY_LISTEN_ADDRESS")?;
    let relay_base = format!("{relay_address}/p2p/{relay_peer}");
    let relay_route = format!("{relay_base}/p2p-circuit");

    let invitation = run_command(&[
        "create-invite",
        "--state",
        &source_text,
        "--community",
        &community_text,
        "--address",
        &relay_route,
    ])?;
    require_success(&invitation)?;
    let invite_code = field(&invitation, "INVITE_CODE")?;

    let mut source = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "run",
            "--state",
            &source_text,
            "--community",
            &community_text,
            "--listen",
            "/ip4/0.0.0.0/udp/0/quic-v1",
            "--relay-address",
            &relay_base,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let source_stdout = source
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("source stdout was not captured"))?;
    let (source_receiver, source_reader) = line_channel(source_stdout);
    let _ = wait_for_field(&source_receiver, "RELAY_RESERVATION_ACCEPTED")?;

    let mut target = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "join",
            "--state",
            &target_text,
            "--invite-code",
            &invite_code,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = target.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            stop(&mut target);
            stop(&mut source);
            stop(&mut relay);
            return Err("relay invitation bootstrap did not finish".into());
        }
        thread::sleep(Duration::from_millis(25));
    };
    let mut target_stdout = String::new();
    let mut target_stderr = String::new();
    target
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("target stdout was not captured"))?
        .read_to_string(&mut target_stdout)?;
    target
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("target stderr was not captured"))?
        .read_to_string(&mut target_stderr)?;

    stop(&mut source);
    stop(&mut relay);
    drop(source_receiver);
    drop(relay_receiver);
    let _ = source_reader.join();
    let _ = relay_reader.join();

    if !status.success() {
        return Err(format!("relay join failed: {target_stderr}\n{target_stdout}").into());
    }
    assert!(target_stdout.contains("CONNECTED=") && target_stdout.contains("via=relay"));
    assert!(target_stdout.contains("JOIN_COMPLETE="));
    Ok(())
}
