#![cfg(unix)]

use serde_json::Value;
use std::process::{Command, Output};

fn run(arguments: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    Ok(Command::new(env!("CARGO_BIN_EXE_chatcommons-node"))
        .args(arguments)
        .output()?)
}

fn success(arguments: &[&str]) -> Result<Output, Box<dyn std::error::Error>> {
    let output = run(arguments)?;
    if output.status.success() {
        Ok(output)
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
        .ok_or_else(|| format!("missing {name}").into())
}

#[test]
fn signed_channel_and_message_commands_persist_and_filter() -> Result<(), Box<dyn std::error::Error>>
{
    let temporary = tempfile::tempdir()?;
    let owner = temporary.path().join("owner");
    let owner = owner.to_string_lossy();
    success(&["init", "--state", &owner])?;
    let created = success(&[
        "create-community",
        "--state",
        &owner,
        "--name",
        "Friends alpha",
    ])?;
    let community = field(&created, "COMMUNITY_ID")?;
    let channel = success(&[
        "create-channel",
        "--state",
        &owner,
        "--community",
        &community,
        "--name",
        "general",
    ])?;
    let channel = field(&channel, "CHANNEL_ID")?;
    success(&[
        "send-message",
        "--state",
        &owner,
        "--community",
        &community,
        "--channel",
        &channel,
        "--text",
        "hello from a signed event",
    ])?;

    let channels = success(&[
        "list-channels",
        "--state",
        &owner,
        "--community",
        &community,
    ])?;
    let channels: Value = serde_json::from_slice(&channels.stdout)?;
    assert_eq!(channels[0]["channelId"], channel);
    assert_eq!(channels[0]["name"], "general");

    let messages = success(&[
        "list-messages",
        "--state",
        &owner,
        "--community",
        &community,
        "--channel",
        &channel,
    ])?;
    let messages: Value = serde_json::from_slice(&messages.stdout)?;
    assert_eq!(messages.as_array().map(Vec::len), Some(1));
    assert_eq!(messages[0]["channelId"], channel);
    assert_eq!(messages[0]["text"], "hello from a signed event");
    assert_eq!(messages[0]["authorId"].as_str().map(str::len), Some(64));
    assert_eq!(messages[0]["eventId"].as_str().map(str::len), Some(64));
    Ok(())
}

#[test]
fn non_member_cannot_send_a_message_after_importing_public_history()
-> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let owner = temporary.path().join("owner");
    let outsider = temporary.path().join("outsider");
    let archive = temporary.path().join("community.ccarchive");
    let owner = owner.to_string_lossy();
    let outsider = outsider.to_string_lossy();
    let archive = archive.to_string_lossy();
    success(&["init", "--state", &owner])?;
    success(&["init", "--state", &outsider])?;
    let created = success(&[
        "create-community",
        "--state",
        &owner,
        "--name",
        "Member enforcement",
    ])?;
    let community = field(&created, "COMMUNITY_ID")?;
    let channel = success(&[
        "create-channel",
        "--state",
        &owner,
        "--community",
        &community,
        "--name",
        "general",
    ])?;
    let channel = field(&channel, "CHANNEL_ID")?;
    success(&[
        "export-community",
        "--state",
        &owner,
        "--community",
        &community,
        "--output",
        &archive,
    ])?;
    success(&[
        "import-community",
        "--state",
        &outsider,
        "--input",
        &archive,
    ])?;

    let rejected = run(&[
        "send-message",
        "--state",
        &outsider,
        "--community",
        &community,
        "--channel",
        &channel,
        "--text",
        "this must be rejected",
    ])?;
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("did not authorize"));
    Ok(())
}
