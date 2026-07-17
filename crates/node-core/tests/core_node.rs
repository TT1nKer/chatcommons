use chatcommons_crypto::Identity;
use chatcommons_node_core::CoreNode;
use chatcommons_protocol::{
    EventContent, PROTOCOL_VERSION, community_id, create_genesis, create_signed,
};
use chatcommons_storage::EventStore;

#[test]
fn opaque_branches_ingest_out_of_order_and_survive_reopen() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("core.db");
    let alice = Identity::from_seed([41; 32]);
    let bob = Identity::from_seed([42; 32]);
    let genesis = create_genesis(&alice, "any.genesis", vec![1], 1);
    let community = community_id(&genesis)?;
    let left = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 2,
            event_type: "any.left".into(),
            payload: vec![2],
        },
        &alice,
    );
    let right = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 2,
            event_type: "another.profile.event".into(),
            payload: vec![3],
        },
        &bob,
    );

    {
        let mut node = CoreNode::open(EventStore::open(&database)?, None)?;
        let report = node.ingest(vec![right.clone(), left.clone(), genesis.clone()])?;
        assert_eq!(report.inserted, 3);
        assert!(report.unresolved.is_empty());
        let duplicate = node.ingest(vec![left.clone()])?;
        assert_eq!(duplicate.already_present, 1);
        assert_eq!(node.event_ids().len(), 3);
    }

    let reopened = CoreNode::open(EventStore::open(&database)?, Some(community))?;
    assert_eq!(reopened.event_ids().len(), 3);
    assert!(reopened.event(left.event_id)?.is_some());
    assert!(reopened.event(right.event_id)?.is_some());
    Ok(())
}
