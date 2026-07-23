use chatcommons_crypto::Identity;
use chatcommons_node_core::CoreNode;
use chatcommons_profile_chat::{
    InviteCapability, create_chat_event, create_chat_genesis, parse_invite_package,
};
use chatcommons_protocol::community_id;
use chatcommons_relay::{RelayNode, RelayNodeEvent};
use chatcommons_storage::EventStore;
use chatcommons_sync::{
    SyncPeer,
    auth::{DeviceIdentity, RevocationSet, create_device_certificate},
    network::{NetworkEvent, NetworkNode},
};
use libp2p::{Multiaddr, multiaddr::Protocol};
use std::{collections::BTreeSet, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn relay_connection_upgrades_to_direct_quic_and_syncs()
-> Result<(), Box<dyn std::error::Error>> {
    let outcome = run_relay_sync(true).await?;
    assert!(outcome.saw_relayed_connection);
    assert!(outcome.saw_hole_punch);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn relay_remains_a_working_fallback_without_direct_candidates()
-> Result<(), Box<dyn std::error::Error>> {
    let outcome = run_relay_sync(false).await?;
    assert!(outcome.saw_relayed_connection);
    assert!(!outcome.saw_hole_punch);
    Ok(())
}

struct Outcome {
    saw_relayed_connection: bool,
    saw_hole_punch: bool,
}

async fn run_relay_sync(direct_listeners: bool) -> Result<Outcome, Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let alice = Identity::from_seed([111; 32]);
    let bob = Identity::from_seed([112; 32]);
    let alice_device = DeviceIdentity::from_seed([113; 32])?;
    let bob_device = DeviceIdentity::from_seed([114; 32])?;
    let invite_capability = InviteCapability::from_seed([115; 32]);

    let genesis = create_chat_genesis(&alice, "Relay friends", 1)?;
    let community = community_id(&genesis)?;
    let invitation = create_chat_event(
        &alice,
        community,
        vec![genesis.event_id],
        2,
        invite_capability.invitation_payload(),
    )?;
    let package = invite_capability.encode_package(community, invitation.event_id)?;
    let acceptance = parse_invite_package(&package)?
        .validate(&invitation)?
        .create_acceptance(&bob, vec![invitation.event_id], 3)?;

    let mut source_core = CoreNode::open(
        EventStore::open(directory.path().join("relay-source.db"))?,
        None,
    )?;
    source_core.ingest(vec![acceptance, invitation, genesis])?;
    let target_core = CoreNode::open(
        EventStore::open(directory.path().join("relay-target.db"))?,
        None,
    )?;
    let allowed = BTreeSet::from([alice.user_id(), bob.user_id()]);
    let mut source = NetworkNode::new(
        &alice_device,
        create_device_certificate(&alice, &alice_device, 1),
        SyncPeer::new(source_core, community)?,
        allowed.clone(),
        RevocationSet::default(),
    )?;
    let mut target = NetworkNode::new(
        &bob_device,
        create_device_certificate(&bob, &bob_device, 1),
        SyncPeer::new(target_core, community)?,
        allowed,
        RevocationSet::default(),
    )?;
    let source_peer = source.peer_id();
    let target_peer = target.peer_id();

    let mut relay = RelayNode::ephemeral()?;
    let relay_peer = relay.peer_id();
    relay.listen(if direct_listeners {
        "/ip4/127.0.0.1/udp/0/quic-v1".parse()?
    } else {
        "/ip4/127.0.0.1/tcp/0".parse()?
    })?;
    let relay_listen = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let RelayNodeEvent::Listening(address) = relay.next_event().await? {
                return Ok::<Multiaddr, chatcommons_relay::RelayError>(address);
            }
        }
    })
    .await??;
    let relay_base = relay_listen.with(Protocol::P2p(relay_peer));

    if direct_listeners {
        source.listen("/ip4/127.0.0.1/udp/0/quic-v1".parse()?)?;
        target.listen("/ip4/127.0.0.1/udp/0/quic-v1".parse()?)?;
    }
    let relay_route = source.reserve_relay(relay_base)?;

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = relay.next_event() => { event?; }
                event = source.next_event() => {
                    let event = event?;
                    if matches!(event, NetworkEvent::RelayReservationAccepted(peer) if peer == relay_peer) {
                        return Ok::<(), Box<dyn std::error::Error>>(());
                    }
                }
                event = target.next_event(), if direct_listeners => { event?; }
            }
        }
    })
    .await??;

    target.dial(source_peer, relay_route)?;
    let mut outcome = Outcome {
        saw_relayed_connection: false,
        saw_hole_punch: false,
    };
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            tokio::select! {
                event = relay.next_event() => { event?; }
                event = source.next_event() => {
                    observe(event?, &mut outcome);
                }
                event = target.next_event() => {
                    observe(event?, &mut outcome);
                }
            }
            if source.is_authenticated(target_peer)
                && target.is_authenticated(source_peer)
                && source.sync_peer().node().event_ids() == target.sync_peer().node().event_ids()
                && (!direct_listeners || outcome.saw_hole_punch)
            {
                return Ok::<(), Box<dyn std::error::Error>>(());
            }
        }
    })
    .await??;

    assert!(!source.is_authenticated(relay_peer));
    assert!(!target.is_authenticated(relay_peer));
    assert_eq!(target.sync_peer().node().event_ids().len(), 3);
    Ok(outcome)
}

fn observe(event: NetworkEvent, outcome: &mut Outcome) {
    match event {
        NetworkEvent::Connected { relayed: true, .. } => outcome.saw_relayed_connection = true,
        NetworkEvent::HolePunchSucceeded(_) => outcome.saw_hole_punch = true,
        _ => {}
    }
}
