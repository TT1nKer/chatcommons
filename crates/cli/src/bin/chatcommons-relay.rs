use chatcommons_relay::{RelayNode, RelayNodeEvent};
use libp2p::Multiaddr;
use std::{env, process::ExitCode, str::FromStr};

const USAGE: &str = "ChatCommons M2e diagnostic relay\n\nUsage:\n  chatcommons-relay --listen <multiaddr>\n\nThe relay identity is ephemeral. Do not deploy this binary as a public service.\n";

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

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args().skip(1);
    let first = arguments.next();
    if first.as_deref().is_some_and(|value| value == "--help") || first.is_none() {
        print!("{USAGE}");
        return Ok(());
    }
    if first.as_deref() != Some("--listen") {
        return Err("expected --listen".into());
    }
    let address = arguments
        .next()
        .ok_or("missing value for --listen")
        .and_then(|value| Multiaddr::from_str(&value).map_err(|_| "invalid listen multiaddr"))?;
    if arguments.next().is_some() {
        return Err("unexpected extra arguments".into());
    }

    let mut relay = RelayNode::ephemeral()?;
    println!("RELAY_PEER_ID={}", relay.peer_id());
    relay.listen(address)?;
    loop {
        match relay.next_event().await? {
            RelayNodeEvent::Listening(address) => println!("RELAY_LISTEN_ADDRESS={address}"),
            RelayNodeEvent::ConnectionEstablished(peer) => {
                println!("RELAY_CONNECTED={peer}")
            }
            RelayNodeEvent::ConnectionClosed(peer) => println!("RELAY_DISCONNECTED={peer}"),
            RelayNodeEvent::RelayActivity(activity) => println!("RELAY_ACTIVITY={activity}"),
        }
    }
}
