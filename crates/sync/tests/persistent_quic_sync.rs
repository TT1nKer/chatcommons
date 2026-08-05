use chatcommons_crypto::Identity;
use chatcommons_node_core::CoreNode;
use chatcommons_profile_chat::{ChatPayload, create_chat_event, create_chat_genesis};
use chatcommons_protocol::community_id;
use chatcommons_storage::EventStore;
use chatcommons_sync::{
    SyncPeer,
    auth::{DeviceIdentity, RevocationSet, create_device_certificate},
    network::{NetworkEvent, NetworkNode},
};
use libp2p::Multiaddr;
use std::{collections::BTreeSet, time::Duration};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authenticated_quic_peers_resynchronize_without_reconnecting()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let alice = Identity::from_seed([111; 32]);
    let bob = Identity::from_seed([112; 32]);
    let alice_device = DeviceIdentity::from_seed([113; 32])?;
    let bob_device = DeviceIdentity::from_seed([114; 32])?;
    let genesis = create_chat_genesis(&alice, "Persistent QUIC", 1)?;
    let community = community_id(&genesis)?;

    let mut source_core = CoreNode::open(
        EventStore::open(directory.path().join("persistent-source.db"))?,
        None,
    )?;
    source_core.ingest(vec![genesis])?;
    let target_core = CoreNode::open(
        EventStore::open(directory.path().join("persistent-target.db"))?,
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
    source.listen("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>()?)?;

    let listen_address = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let NetworkEvent::Listening(address) = source.next_event().await? {
                return Ok::<Multiaddr, chatcommons_sync::network::NetworkError>(address);
            }
        }
    })
    .await??;
    target.dial(source_peer, listen_address)?;

    drive_until(
        Duration::from_secs(10),
        &mut source,
        &mut target,
        |source, target| {
            source.is_mutually_authenticated(target_peer)
                && target.is_mutually_authenticated(source_peer)
                && source.sync_peer().node().event_ids() == target.sync_peer().node().event_ids()
        },
    )
    .await?;

    let channel = create_chat_event(
        &alice,
        community,
        source.sync_peer().node().heads()?,
        2,
        ChatPayload::ChannelCreate {
            channel_id: [7; 32],
            name: "general".into(),
        },
    )?;
    let channel_id = channel.event_id;
    source.sync_peer_mut().node_mut().ingest(vec![channel])?;

    assert_eq!(source.announce_local_state()?, 1);
    drive_until(
        Duration::from_secs(5),
        &mut source,
        &mut target,
        |_, target| target.sync_peer().node().event_ids().contains(&channel_id),
    )
    .await?;
    assert!(source.is_mutually_authenticated(target_peer));
    assert!(target.is_mutually_authenticated(source_peer));
    Ok(())
}

async fn drive_until(
    timeout: Duration,
    source: &mut NetworkNode,
    target: &mut NetworkNode,
    complete: impl Fn(&NetworkNode, &NetworkNode) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::timeout(timeout, async {
        loop {
            tokio::select! {
                event = source.next_event() => { event?; }
                event = target.next_event() => { event?; }
            }
            if complete(source, target) {
                return Ok::<(), chatcommons_sync::network::NetworkError>(());
            }
        }
    })
    .await??;
    Ok(())
}
