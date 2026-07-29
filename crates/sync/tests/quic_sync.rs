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
    network::{NetworkEvent, NetworkNode, VoiceGrant},
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
    let channel_id = [7; 32];
    target.request_voice_token(source_peer, channel_id)?;
    let received_grant = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            tokio::select! {
                event = source.next_event() => {
                    if let NetworkEvent::VoiceTokenRequest {
                        peer,
                        user_id,
                        device_id,
                        channel_id: requested_channel,
                    } = event?
                    {
                        assert_eq!(peer, target_peer);
                        assert_eq!(user_id, bob.user_id());
                        assert_eq!(device_id, bob_device.device_id());
                        assert_eq!(requested_channel, channel_id);
                        source.resolve_voice_token(peer, Ok(VoiceGrant {
                            server_url: "wss://voice.example.test".into(),
                            participant_token: "header.payload.signature".into(),
                            expires_at_ms: 60_000,
                        }))?;
                    }
                }
                event = target.next_event() => {
                    if let NetworkEvent::VoiceToken { peer, grant } = event? {
                        assert_eq!(peer, source_peer);
                        break Ok::<VoiceGrant, chatcommons_sync::network::NetworkError>(grant);
                    }
                }
            }
        }
    })
    .await??;
    assert_eq!(received_grant.server_url, "wss://voice.example.test");
    assert_eq!(received_grant.participant_token, "header.payload.signature");
    assert!(!format!("{received_grant:?}").contains("header.payload.signature"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_quic_handshake_does_not_stop_the_listener()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let server_user = Identity::from_seed([101; 32]);
    let client_user = Identity::from_seed([102; 32]);
    let server_device = DeviceIdentity::from_seed([103; 32])?;
    let client_device = DeviceIdentity::from_seed([104; 32])?;
    let unrelated_device = DeviceIdentity::from_seed([105; 32])?;
    let genesis = create_chat_genesis(&server_user, "Resilient listener", 1)?;
    let community = community_id(&genesis)?;
    let mut server_core = CoreNode::open(
        EventStore::open(directory.path().join("resilient-server.db"))?,
        None,
    )?;
    server_core.ingest(vec![genesis])?;
    let client_core = CoreNode::open(
        EventStore::open(directory.path().join("resilient-client.db"))?,
        None,
    )?;
    let allowed = BTreeSet::from([server_user.user_id(), client_user.user_id()]);
    let mut server = NetworkNode::new(
        &server_device,
        create_device_certificate(&server_user, &server_device, 1),
        SyncPeer::new(server_core, community)?,
        allowed.clone(),
        RevocationSet::default(),
    )?;
    let mut client = NetworkNode::new(
        &client_device,
        create_device_certificate(&client_user, &client_device, 1),
        SyncPeer::new(client_core, community)?,
        allowed,
        RevocationSet::default(),
    )?;
    let server_peer = server.peer_id();
    let client_peer = client.peer_id();
    server.listen("/ip4/127.0.0.1/udp/0/quic-v1".parse::<Multiaddr>()?)?;
    let listen_address = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let NetworkEvent::Listening(address) = server.next_event().await? {
                return Ok::<Multiaddr, chatcommons_sync::network::NetworkError>(address);
            }
        }
    })
    .await??;

    client.dial(unrelated_device.peer_id(), listen_address.clone())?;
    let invalid_handshake = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            tokio::select! {
                event = server.next_event() => { event?; }
                event = client.next_event() => {
                    if event.is_err() {
                        return Ok::<(), chatcommons_sync::network::NetworkError>(());
                    }
                }
            }
        }
    })
    .await;
    if let Ok(result) = invalid_handshake {
        result?;
    }

    client.dial(server_peer, listen_address)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = server.next_event() => { event?; }
                event = client.next_event() => { event?; }
            }
            if server.is_authenticated(client_peer) && client.is_authenticated(server_peer) {
                return Ok::<(), chatcommons_sync::network::NetworkError>(());
            }
        }
    })
    .await??;
    Ok(())
}
