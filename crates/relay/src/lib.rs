use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, Swarm, SwarmBuilder, identify, identity, noise, ping, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use std::{num::NonZeroU32, time::Duration};
use thiserror::Error;

pub const RELAY_IDENTIFY_PROTOCOL: &str = "/chatcommons/relay/1";
pub const MAX_RELAY_RESERVATIONS: usize = 128;
pub const MAX_RELAY_CIRCUITS: usize = 256;
pub const MAX_RELAY_CIRCUIT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(NetworkBehaviour)]
struct Behaviour {
    relay: relay::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayNodeEvent {
    Listening(Multiaddr),
    ConnectionEstablished(PeerId),
    ConnectionClosed(PeerId),
    RelayActivity(String),
}

#[derive(Debug, Error)]
pub enum RelayError {
    #[error("relay transport setup failed: {0}")]
    Transport(String),
    #[error("relay behaviour setup failed: {0}")]
    Behaviour(String),
    #[error("relay listen failed: {0}")]
    Listen(String),
    #[error("relay listener failed: {0}")]
    Listener(String),
}

pub struct RelayNode {
    swarm: Swarm<Behaviour>,
}

impl RelayNode {
    pub fn ephemeral() -> Result<Self, RelayError> {
        Self::new(identity::Keypair::generate_ed25519())
    }

    pub fn new(keypair: identity::Keypair) -> Result<Self, RelayError> {
        let builder = SwarmBuilder::with_existing_identity(keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|error| RelayError::Transport(error.to_string()))?
            .with_quic()
            .with_behaviour(|key| Behaviour {
                relay: relay::Behaviour::new(key.public().to_peer_id(), bounded_config()),
                identify: identify::Behaviour::new(identify::Config::new(
                    RELAY_IDENTIFY_PROTOCOL.to_owned(),
                    key.public(),
                )),
                ping: ping::Behaviour::new(ping::Config::new()),
            });
        let builder = builder.map_err(|error| RelayError::Behaviour(error.to_string()))?;
        Ok(Self {
            swarm: builder.build(),
        })
    }

    pub fn peer_id(&self) -> PeerId {
        *self.swarm.local_peer_id()
    }

    pub fn listen(&mut self, address: Multiaddr) -> Result<(), RelayError> {
        self.swarm
            .listen_on(address)
            .map(|_| ())
            .map_err(|error| RelayError::Listen(format!("{error:?}")))
    }

    pub async fn next_event(&mut self) -> Result<RelayNodeEvent, RelayError> {
        loop {
            match self.swarm.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => {
                    return Ok(RelayNodeEvent::Listening(address));
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    return Ok(RelayNodeEvent::ConnectionEstablished(peer_id));
                }
                SwarmEvent::ConnectionClosed { peer_id, .. } => {
                    return Ok(RelayNodeEvent::ConnectionClosed(peer_id));
                }
                SwarmEvent::Behaviour(BehaviourEvent::Relay(event)) => {
                    return Ok(RelayNodeEvent::RelayActivity(format!("{event:?}")));
                }
                SwarmEvent::Behaviour(BehaviourEvent::Identify(identify::Event::Received {
                    info,
                    ..
                })) => {
                    self.swarm.add_external_address(info.observed_addr);
                }
                SwarmEvent::ListenerError { error, .. } => {
                    return Err(RelayError::Listener(error.to_string()));
                }
                _ => {}
            }
        }
    }
}

fn bounded_config() -> relay::Config {
    let mut config = relay::Config {
        max_reservations: MAX_RELAY_RESERVATIONS,
        max_reservations_per_peer: 1,
        reservation_duration: Duration::from_secs(60 * 60),
        max_circuits: MAX_RELAY_CIRCUITS,
        max_circuits_per_peer: 4,
        max_circuit_duration: Duration::from_secs(2 * 60),
        max_circuit_bytes: MAX_RELAY_CIRCUIT_BYTES,
        ..Default::default()
    };
    let reservation_limit = NonZeroU32::new(8).expect("constant is non-zero");
    let circuit_limit = NonZeroU32::new(60).expect("constant is non-zero");
    config = config
        .reservation_rate_per_peer(reservation_limit, Duration::from_secs(60))
        .reservation_rate_per_ip(reservation_limit, Duration::from_secs(60))
        .circuit_src_per_peer(circuit_limit, Duration::from_secs(60))
        .circuit_src_per_ip(circuit_limit, Duration::from_secs(60));
    config
}
