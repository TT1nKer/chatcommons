use chatcommons_crypto::Identity;
use chatcommons_protocol::{
    EventContent, EventId, PROTOCOL_VERSION, community_id, create_genesis, create_signed,
};
use chatcommons_storage::{
    EventStore,
    archive::{ArchiveError, MAX_ARCHIVE_BYTES, encode, parse},
};

fn fixture() -> Result<
    (
        chatcommons_protocol::CommunityId,
        Vec<chatcommons_protocol::SignedEvent>,
    ),
    Box<dyn std::error::Error>,
> {
    let identity = Identity::from_seed([31; 32]);
    let genesis = create_genesis(&identity, "fixture.create", b"genesis".to_vec(), 1);
    let community = community_id(&genesis)?;
    let child = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 2,
            event_type: "fixture.event".into(),
            payload: b"child".to_vec(),
        },
        &identity,
    );
    let branch = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 3,
            event_type: "fixture.event".into(),
            payload: b"branch".to_vec(),
        },
        &identity,
    );
    Ok((community, vec![genesis, child, branch]))
}

#[test]
fn archive_is_deterministic_and_survives_sqlite_reopen() -> Result<(), Box<dyn std::error::Error>> {
    let (community, events) = fixture()?;
    let mut reversed = events.clone();
    reversed.reverse();
    let forward = encode(community, events)?;
    let reverse = encode(community, reversed)?;
    assert_eq!(forward, reverse);

    let validated = parse(&forward)?.validate()?;
    assert_eq!(validated.community(), community);
    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("archive.sqlite3");
    {
        let mut store = EventStore::open(&path)?;
        for event in validated.events() {
            store.insert(event)?;
        }
    }
    let reopened = EventStore::open(&path)?;
    assert_eq!(reopened.events(community)?, validated.events());
    Ok(())
}

#[test]
fn archive_rejects_tampering_missing_parents_and_oversized_input()
-> Result<(), Box<dyn std::error::Error>> {
    let (community, events) = fixture()?;
    let encoded = encode(community, events.clone())?;
    let mut tampered: serde_json::Value = serde_json::from_slice(&encoded)?;
    let payload = tampered
        .get_mut("events")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|events| {
            events
                .iter_mut()
                .find(|event| event["content"]["community_id"] != serde_json::Value::Null)
        })
        .and_then(|event| event.get_mut("content"))
        .and_then(|content| content.get_mut("payload"))
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("fixture payload was not found")?;
    payload.push(serde_json::Value::from(0));
    let tampered = serde_json::to_vec(&tampered)?;
    assert!(matches!(
        parse(&tampered)?.validate(),
        Err(ArchiveError::Protocol(_))
    ));

    let identity = Identity::from_seed([32; 32]);
    let incomplete = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![EventId::from_bytes([9; 32])],
            timestamp_ms: 4,
            event_type: "fixture.event".into(),
            payload: b"incomplete".to_vec(),
        },
        &identity,
    );
    assert!(matches!(
        encode(community, vec![events[0].clone(), incomplete]),
        Err(ArchiveError::MissingParent)
    ));
    assert!(matches!(
        parse(&vec![0; MAX_ARCHIVE_BYTES + 1]),
        Err(ArchiveError::TooLarge)
    ));
    Ok(())
}
