#![cfg(unix)]

use chatcommons_node_core::CoreNode;
use chatcommons_protocol::CommunityId;
use chatcommons_storage::EventStore;
use std::{
    io::{self, BufRead, BufReader, Read},
    process::{Child, Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn run_command(arguments: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args(arguments)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn field(output: &Output, name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let prefix = format!("{name}=");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
        .ok_or_else(|| format!("missing {name} in command output").into())
}

fn stop(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn two_cli_processes_sync_a_community_over_quic() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let source_path = temporary.path().join("source");
    let target_path = temporary.path().join("target");
    let source_text = source_path.to_string_lossy().into_owned();
    let target_text = target_path.to_string_lossy().into_owned();

    let source_init = run_command(&["init", "--state", &source_text])?;
    let target_init = run_command(&["init", "--state", &target_text])?;
    let source_user = field(&source_init, "USER_ID")?;
    let source_peer = field(&source_init, "PEER_ID")?;
    let target_user = field(&target_init, "USER_ID")?;
    let created = run_command(&[
        "create-community",
        "--state",
        &source_text,
        "--name",
        "M2c process test",
    ])?;
    let community = field(&created, "COMMUNITY_ID")?;

    let mut source = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "run",
            "--state",
            &source_text,
            "--community",
            &community,
            "--listen",
            "/ip4/127.0.0.1/udp/0/quic-v1",
            "--allow-user",
            &target_user,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let source_stdout = source
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("source process stdout was not captured"))?;
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(source_stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let listen_address = loop {
        match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(line)) => {
                if let Some(value) = line.strip_prefix("LISTEN_ADDRESS=") {
                    break value.to_owned();
                }
            }
            Ok(Err(error)) => {
                stop(&mut source);
                return Err(error.into());
            }
            Err(error) => {
                stop(&mut source);
                return Err(format!("source did not start listening: {error}").into());
            }
        }
    };

    let mut target = Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args([
            "run",
            "--state",
            &target_text,
            "--community",
            &community,
            "--listen",
            "/ip4/127.0.0.1/udp/0/quic-v1",
            "--allow-user",
            &source_user,
            "--dial-peer",
            &source_peer,
            "--dial-address",
            &listen_address,
            "--exit-after-events",
            "1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let target_status = loop {
        if let Some(status) = target.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            stop(&mut target);
            stop(&mut source);
            return Err("target process did not finish synchronization".into());
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
    drop(receiver);
    let _ = reader.join();

    if !target_status.success() {
        return Err(format!("target failed: {target_stderr}").into());
    }
    assert!(target_stdout.contains("SYNC_COMPLETE events=1"));

    let bytes: [u8; 32] = hex::decode(&community)?
        .try_into()
        .map_err(|_| "community ID has the wrong length")?;
    let community = CommunityId::from_bytes(bytes);
    let target_node = CoreNode::open(
        EventStore::open(target_path.join("events.sqlite3"))?,
        Some(community),
    )?;
    assert_eq!(target_node.event_ids().len(), 1);
    Ok(())
}
