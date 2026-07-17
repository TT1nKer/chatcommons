use chatcommons_crypto::Identity;
use chatcommons_protocol::{
    EventContent, MAX_EVENT_JSON_BYTES, PROTOCOL_VERSION, ParseError, ProtocolError, author_id,
    canonical_content, community_id, create_genesis, create_signed, parse_json, validate_event,
};

#[test]
fn core_verifies_opaque_events_and_detects_tampering() -> Result<(), Box<dyn std::error::Error>> {
    let alice = Identity::from_seed([7; 32]);
    let genesis = create_genesis(&alice, "example.genesis", b"opaque".to_vec(), 1);
    let community = community_id(&genesis)?;
    let event = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 2,
            event_type: "unknown.application.event".into(),
            payload: vec![0, 1, 2, 255],
        },
        &alice,
    );
    validate_event(&event)?;
    assert_eq!(author_id(&event)?, alice.user_id());
    let mut tampered = event;
    tampered.content.payload.push(9);
    assert!(validate_event(&tampered).is_err());

    let mut replaced_author = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 3,
            event_type: "unknown.application.event".into(),
            payload: vec![],
        },
        &alice,
    );
    replaced_author.public_key = Identity::from_seed([8; 32]).public_key().to_vec();
    assert_eq!(
        validate_event(&replaced_author),
        Err(ProtocolError::InvalidSignature)
    );
    Ok(())
}

#[test]
fn oversized_wire_input_is_rejected_before_json() {
    assert!(matches!(
        parse_json(&vec![b' '; MAX_EVENT_JSON_BYTES + 1]),
        Err(ParseError::TooLarge)
    ));
}

#[test]
fn v2_genesis_vector() {
    let identity = Identity::from_seed([7; 32]);
    let event = create_genesis(
        &identity,
        "test.genesis",
        b"ChatCommons v2".to_vec(),
        1_700_000_000_000,
    );
    assert!(validate_event(&event).is_ok());
    assert_eq!(
        hex::encode(canonical_content(&event.content)),
        "63686174636f6d6d6f6e733a636f6e74656e743a763200000200000000000000018bcfe568000000000c746573742e67656e657369730000000e43686174436f6d6d6f6e73207632"
    );
    assert_eq!(
        hex::encode(event.event_id.as_bytes()),
        "9907bedb98c7cb6587f9a037ee5cdc29cd71ba1b71c3382eef20a118fadca530"
    );
    assert_eq!(
        hex::encode(&event.signature),
        "2e55180cc7d069bb65d46e9aab0d86b1d53f3e48c6f6f09a34065834ed76549885360d7baa0687ef2cce15d6b954281f60bb12b52104397342b31cc44187d905"
    );
}

#[test]
fn non_genesis_requires_parent() {
    let identity = Identity::from_seed([8; 32]);
    let event = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(chatcommons_protocol::CommunityId::from_bytes([1; 32])),
            parents: vec![],
            timestamp_ms: 0,
            event_type: "x".into(),
            payload: vec![],
        },
        &identity,
    );
    assert_eq!(
        validate_event(&event),
        Err(ProtocolError::InvalidCommunityReference)
    );
}
