use chatcommons_crypto::Identity;
use chatcommons_node_core::CoreNode;
use chatcommons_profile_chat::{
    InviteCapability, create_chat_event, create_chat_genesis, parse_invite_package,
};
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
async fn two_real_quic_swarms_authenticate_and_sync_sqlite()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let alice = Identity::from_seed([91; 32]);
    let bob = Identity::from_seed([92; 32]);
    let alice_device = DeviceIdentity::from_seed([93; 32])?;
    let bob_device = DeviceIdentity::from_seed([94; 32])?;
    let invite_capability = InviteCapability::from_seed([95; 32]);

    let genesis = create_chat_genesis(&alice, "QUIC friends", 1)?;
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
        EventStore::open(directory.path().join("quic-source.db"))?,
        None,
    )?;
    source_core.ingest(vec![acceptance, invitation, genesis])?;
    let target_core = CoreNode::open(
        EventStore::open(directory.path().join("quic-target.db"))?,
        None,
    )?;

    let allowed = BTreeSet::from([alice.user_id(), bob.user_id()]);
    let alice_certificate = create_device_certificate(&alice, &alice_device, 1);
    let bob_certificate = create_device_certificate(&bob, &bob_device, 1);
    let mut source = NetworkNode::new(
        &alice_device,
        alice_certificate,
        SyncPeer::new(source_core, community)?,
        allowed.clone(),
        RevocationSet::default(),
    )?;
    let mut target = NetworkNode::new(
        &bob_device,
        bob_certificate,
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

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = source.next_event() => { event?; }
                event = target.next_event() => { event?; }
            }
            if source.is_authenticated(target_peer)
                && target.is_authenticated(source_peer)
                && source.sync_peer().node().event_ids() == target.sync_peer().node().event_ids()
            {
                return Ok::<(), chatcommons_sync::network::NetworkError>(());
            }
        }
    })
    .await??;

    assert_eq!(target.sync_peer().node().event_ids().len(), 3);
    Ok(())
}
