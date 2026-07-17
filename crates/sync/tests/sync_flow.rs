use chatcommons_crypto::Identity;
use chatcommons_node_core::CoreNode;
use chatcommons_protocol::{
    CommunityId, EventContent, MAX_PAYLOAD_BYTES, PROTOCOL_VERSION, SignedEvent, community_id,
    create_genesis, create_signed,
};
use chatcommons_storage::EventStore;
use chatcommons_sync::{
    MAX_IDS_PER_MESSAGE, MAX_SYNC_JSON_BYTES, SyncError, SyncMessage, SyncPeer, parse_json,
    to_json, validate_message,
};
use std::collections::VecDeque;

fn event(
    identity: &Identity,
    community: CommunityId,
    parents: Vec<chatcommons_protocol::EventId>,
    marker: u8,
) -> SignedEvent {
    create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents,
            timestamp_ms: i64::from(marker),
            event_type: "test.sync".into(),
            payload: vec![marker],
        },
        identity,
    )
}

fn drive(a: &mut SyncPeer, b: &mut SyncPeer) -> Result<(), SyncError> {
    let mut queue = VecDeque::from([(false, a.hello()), (true, b.hello())]);
    let mut steps = 0;
    while let Some((to_a, message)) = queue.pop_front() {
        steps += 1;
        assert!(steps < 10_000, "sync transcript did not terminate");
        let responses = if to_a {
            a.receive(message)?
        } else {
            b.receive(message)?
        };
        queue.extend(responses.into_iter().map(|response| (!to_a, response)));
    }
    Ok(())
}

#[test]
fn empty_node_recursively_fetches_missing_parents() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let identity = Identity::from_seed([71; 32]);
    let genesis = create_genesis(&identity, "sync.genesis", vec![], 1);
    let community = community_id(&genesis)?;
    let middle = event(&identity, community, vec![genesis.event_id], 2);
    let head = event(&identity, community, vec![middle.event_id], 3);

    let mut source_node =
        CoreNode::open(EventStore::open(directory.path().join("source.db"))?, None)?;
    source_node.ingest(vec![head.clone(), middle.clone(), genesis.clone()])?;
    let target_node = CoreNode::open(EventStore::open(directory.path().join("target.db"))?, None)?;
    let mut source = SyncPeer::new(source_node, community)?;
    let mut target = SyncPeer::new(target_node, community)?;

    drive(&mut source, &mut target)?;
    assert_eq!(target.node().event_ids(), source.node().event_ids());
    assert_eq!(target.pending_len(), 0);
    Ok(())
}

#[test]
fn independent_branches_converge_in_both_directions() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let alice = Identity::from_seed([72; 32]);
    let bob = Identity::from_seed([73; 32]);
    let genesis = create_genesis(&alice, "sync.genesis", vec![], 1);
    let community = community_id(&genesis)?;
    let left = event(&alice, community, vec![genesis.event_id], 2);
    let right = event(&bob, community, vec![genesis.event_id], 3);

    let mut a_node = CoreNode::open(EventStore::open(directory.path().join("a.db"))?, None)?;
    a_node.ingest(vec![genesis.clone(), left])?;
    let mut b_node = CoreNode::open(EventStore::open(directory.path().join("b.db"))?, None)?;
    b_node.ingest(vec![genesis, right])?;
    let mut a = SyncPeer::new(a_node, community)?;
    let mut b = SyncPeer::new(b_node, community)?;

    drive(&mut a, &mut b)?;
    assert_eq!(a.node().event_ids(), b.node().event_ids());
    assert_eq!(a.node().event_ids().len(), 3);
    drive(&mut a, &mut b)?;
    assert_eq!(a.node().event_ids().len(), 3);
    Ok(())
}

#[test]
fn rejects_tampering_wrong_community_and_unbounded_input() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let identity = Identity::from_seed([74; 32]);
    let genesis = create_genesis(&identity, "sync.genesis", vec![], 1);
    let community = community_id(&genesis)?;
    let node = CoreNode::open(EventStore::open(directory.path().join("target.db"))?, None)?;
    let mut peer = SyncPeer::new(node, community)?;

    let mut tampered = genesis.clone();
    tampered.content.payload.push(1);
    assert!(matches!(
        peer.receive(SyncMessage::Events {
            community_id: community,
            events: vec![tampered],
        }),
        Err(SyncError::Protocol(_))
    ));

    let other = create_genesis(&identity, "other.genesis", vec![], 2);
    let other_community = community_id(&other)?;
    assert!(matches!(
        peer.receive(SyncMessage::Events {
            community_id: community,
            events: vec![other],
        }),
        Err(SyncError::WrongCommunity)
    ));
    assert!(matches!(
        peer.receive(SyncMessage::Heads {
            community_id: other_community,
            event_ids: vec![],
        }),
        Err(SyncError::WrongCommunity)
    ));

    let too_many = (0..=MAX_IDS_PER_MESSAGE)
        .map(|index| chatcommons_protocol::EventId::from_bytes([index as u8; 32]))
        .collect();
    assert!(matches!(
        validate_message(&SyncMessage::Want {
            community_id: community,
            event_ids: too_many,
        }),
        Err(SyncError::ItemLimitExceeded)
    ));
    assert!(matches!(
        parse_json(&vec![b' '; MAX_SYNC_JSON_BYTES + 1]),
        Err(SyncError::TooLarge)
    ));
    assert_eq!(peer.pending_len(), 0);
    Ok(())
}

#[test]
fn head_inventory_is_chunked_without_losing_branches() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let identity = Identity::from_seed([75; 32]);
    let genesis = create_genesis(&identity, "sync.genesis", vec![], 1);
    let community = community_id(&genesis)?;
    let branches: Vec<SignedEvent> = (0_u8..65)
        .map(|marker| event(&identity, community, vec![genesis.event_id], marker))
        .collect();
    let mut source_events = branches;
    source_events.push(genesis);

    let mut source_node = CoreNode::open(
        EventStore::open(directory.path().join("many-source.db"))?,
        None,
    )?;
    source_node.ingest(source_events)?;
    let target_node = CoreNode::open(
        EventStore::open(directory.path().join("many-target.db"))?,
        None,
    )?;
    let mut source = SyncPeer::new(source_node, community)?;
    let mut target = SyncPeer::new(target_node, community)?;
    drive(&mut source, &mut target)?;

    assert_eq!(target.node().event_ids(), source.node().event_ids());
    assert_eq!(target.node().event_ids().len(), 66);
    Ok(())
}

#[test]
fn large_event_batches_are_split_by_encoded_size() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let identity = Identity::from_seed([76; 32]);
    let genesis = create_genesis(&identity, "sync.genesis", vec![], 1);
    let community = community_id(&genesis)?;
    let mut large_events: Vec<SignedEvent> = (1_i64..=3)
        .map(|timestamp_ms| {
            create_signed(
                EventContent {
                    protocol_version: PROTOCOL_VERSION,
                    community_id: Some(community),
                    parents: vec![genesis.event_id],
                    timestamp_ms,
                    event_type: "test.large".into(),
                    payload: vec![255; MAX_PAYLOAD_BYTES],
                },
                &identity,
            )
        })
        .collect();
    let mut stored = large_events.clone();
    stored.push(genesis);
    let mut node = CoreNode::open(EventStore::open(directory.path().join("large.db"))?, None)?;
    node.ingest(stored)?;
    let mut peer = SyncPeer::new(node, community)?;
    large_events.sort_by_key(|event| event.event_id);

    let responses = peer.receive(SyncMessage::Want {
        community_id: community,
        event_ids: large_events.iter().map(|event| event.event_id).collect(),
    })?;
    assert!(responses.len() > 1);
    for response in responses {
        assert!(to_json(&response)?.len() <= MAX_SYNC_JSON_BYTES);
    }
    Ok(())
}
