use chatcommons_crypto::Identity;
use chatcommons_profile_chat::{
    ChatPayload, HomeServerId, InviteCapability, InviteError, MAX_INVITE_PACKAGE_BYTES,
    RejectionReason, create_chat_event, create_chat_genesis, parse_invite_package, resolve,
};
use chatcommons_protocol::{
    CommunityId, EventContent, PROTOCOL_VERSION, SignedEvent, community_id, create_signed,
};

fn accept_invitation(
    capability: &InviteCapability,
    invitation: &SignedEvent,
    community: CommunityId,
    identity: &Identity,
    parents: Vec<chatcommons_protocol::EventId>,
    timestamp_ms: i64,
) -> Result<SignedEvent, Box<dyn std::error::Error>> {
    let package = capability.encode_package(community, invitation.event_id)?;
    Ok(parse_invite_package(&package)?
        .validate(invitation)?
        .create_acceptance(identity, parents, timestamp_ms)?)
}

#[test]
fn chat_profile_interprets_opaque_core_events_deterministically()
-> Result<(), Box<dyn std::error::Error>> {
    let alice = Identity::from_seed([51; 32]);
    let bob = Identity::from_seed([52; 32]);
    let stranger = Identity::from_seed([53; 32]);
    let channel = [7; 32];
    let bob_invite = InviteCapability::from_seed([101; 32]);
    let genesis = create_chat_genesis(&alice, "Friends", 1)?;
    let community = community_id(&genesis)?;
    let create_channel = create_chat_event(
        &alice,
        community,
        vec![genesis.event_id],
        2,
        ChatPayload::ChannelCreate {
            channel_id: channel,
            name: "general".into(),
        },
    )?;
    let invitation = create_chat_event(
        &alice,
        community,
        vec![create_channel.event_id],
        3,
        bob_invite.invitation_payload(),
    )?;
    let acceptance = accept_invitation(
        &bob_invite,
        &invitation,
        community,
        &bob,
        vec![invitation.event_id],
        4,
    )?;
    let alice_message = create_chat_event(
        &alice,
        community,
        vec![acceptance.event_id],
        5,
        ChatPayload::MessageCreate {
            channel_id: channel,
            text: "hello".into(),
        },
    )?;
    let bob_message = create_chat_event(
        &bob,
        community,
        vec![acceptance.event_id],
        5,
        ChatPayload::MessageCreate {
            channel_id: channel,
            text: "hi".into(),
        },
    )?;
    let stranger_message = create_chat_event(
        &stranger,
        community,
        vec![acceptance.event_id],
        6,
        ChatPayload::MessageCreate {
            channel_id: channel,
            text: "intrusion".into(),
        },
    )?;
    let events = vec![
        genesis,
        create_channel,
        invitation,
        acceptance,
        alice_message,
        bob_message,
        stranger_message.clone(),
    ];
    let forward = resolve(&events)?;
    let reverse = resolve(&events.into_iter().rev().collect::<Vec<_>>())?;
    assert_eq!(forward, reverse);
    assert!(forward.snapshot.members.contains(&alice.user_id()));
    assert!(forward.snapshot.members.contains(&bob.user_id()));
    assert_eq!(forward.snapshot.event_ids.len(), 6);
    assert_eq!(
        forward.rejected.get(&stranger_message.event_id),
        Some(&RejectionReason::NotMember)
    );
    Ok(())
}

#[test]
fn governance_is_a_chat_profile_rule_not_a_core_rule() -> Result<(), Box<dyn std::error::Error>> {
    let alice = Identity::from_seed([54; 32]);
    let bob = Identity::from_seed([55; 32]);
    let genesis = create_chat_genesis(&alice, "Profile boundary", 1)?;
    let community = community_id(&genesis)?;
    let transfer = create_chat_event(
        &bob,
        community,
        vec![genesis.event_id],
        2,
        ChatPayload::OwnershipTransfer {
            new_owner: bob.user_id(),
        },
    )?;
    chatcommons_protocol::validate_event(&transfer)?;
    let resolution = resolve(&[genesis, transfer.clone()])?;
    assert_eq!(
        resolution.rejected.get(&transfer.event_id),
        Some(&RejectionReason::NotOwner)
    );
    Ok(())
}

#[test]
fn core_valid_malformed_profile_payload_is_rejected_without_losing_projection()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([59; 32]);
    let genesis = create_chat_genesis(&owner, "Untrusted payload", 1)?;
    let community = community_id(&genesis)?;
    let malformed = create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents: vec![genesis.event_id],
            timestamp_ms: 2,
            event_type: "chat.message.create".into(),
            payload: b"not-json".to_vec(),
        },
        &owner,
    );
    let child = create_chat_event(
        &owner,
        community,
        vec![malformed.event_id],
        3,
        ChatPayload::OwnershipTransfer {
            new_owner: owner.user_id(),
        },
    )?;

    let resolution = resolve(&[genesis.clone(), malformed.clone(), child.clone()])?;
    assert_eq!(
        resolution.rejected.get(&malformed.event_id),
        Some(&RejectionReason::InvalidPayload)
    );
    assert_eq!(
        resolution.rejected.get(&child.event_id),
        Some(&RejectionReason::ParentRejected)
    );
    assert_eq!(resolution.snapshot.owner, Some(owner.user_id()));
    assert_eq!(resolution.snapshot.event_ids, [genesis.event_id].into());
    Ok(())
}

#[test]
fn owner_can_replace_home_server_without_changing_community_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([56; 32]);
    let first_server = Identity::from_seed([57; 32]);
    let replacement_server = Identity::from_seed([58; 32]);
    let genesis = create_chat_genesis(&owner, "Portable community", 1)?;
    let community = community_id(&genesis)?;
    let first_binding = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        2,
        ChatPayload::HomeServerSet {
            server_public_key: first_server.public_key().to_vec(),
            endpoints: vec!["https://old.example.test".into()],
        },
    )?;
    let replacement = create_chat_event(
        &owner,
        community,
        vec![first_binding.event_id],
        3,
        ChatPayload::HomeServerSet {
            server_public_key: replacement_server.public_key().to_vec(),
            endpoints: vec![
                "/dns4/new.example.test/udp/443/quic-v1".into(),
                "https://new.example.test".into(),
            ],
        },
    )?;

    let resolution = resolve(&[genesis, first_binding.clone(), replacement.clone()])?;
    let binding = resolution
        .snapshot
        .home_server
        .expect("owner-authorized binding should resolve");
    assert_eq!(resolution.snapshot.community, Some(community));
    assert_eq!(binding.declaration, replacement.event_id);
    assert_eq!(
        binding.server_id,
        HomeServerId::from_public_key(&replacement_server.public_key())
    );
    assert_eq!(binding.history_heads, vec![first_binding.event_id]);
    Ok(())
}

#[test]
fn current_server_operator_cannot_redirect_the_community() -> Result<(), Box<dyn std::error::Error>>
{
    let owner = Identity::from_seed([71; 32]);
    let operator = Identity::from_seed([72; 32]);
    let attacker = Identity::from_seed([73; 32]);
    let genesis = create_chat_genesis(&owner, "Owner controls hosting", 1)?;
    let community = community_id(&genesis)?;
    let initial = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        2,
        ChatPayload::HomeServerSet {
            server_public_key: operator.public_key().to_vec(),
            endpoints: vec!["https://operator.example.test".into()],
        },
    )?;
    let forged_redirect = create_chat_event(
        &operator,
        community,
        vec![initial.event_id],
        i64::MAX,
        ChatPayload::HomeServerSet {
            server_public_key: attacker.public_key().to_vec(),
            endpoints: vec!["https://attacker.example.test".into()],
        },
    )?;

    let resolution = resolve(&[genesis, initial.clone(), forged_redirect.clone()])?;
    assert_eq!(
        resolution.rejected.get(&forged_redirect.event_id),
        Some(&RejectionReason::NotOwner)
    );
    assert_eq!(
        resolution
            .snapshot
            .home_server
            .map(|binding| binding.declaration),
        Some(initial.event_id)
    );
    Ok(())
}

#[test]
fn concurrent_home_server_changes_use_event_ids_not_arrival_order()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([74; 32]);
    let server_a = Identity::from_seed([75; 32]);
    let server_b = Identity::from_seed([76; 32]);
    let genesis = create_chat_genesis(&owner, "Deterministic hosting", 1)?;
    let community = community_id(&genesis)?;
    let change_a = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        i64::MAX,
        ChatPayload::HomeServerSet {
            server_public_key: server_a.public_key().to_vec(),
            endpoints: vec!["https://a.example.test".into()],
        },
    )?;
    let change_b = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        i64::MIN,
        ChatPayload::HomeServerSet {
            server_public_key: server_b.public_key().to_vec(),
            endpoints: vec!["https://b.example.test".into()],
        },
    )?;
    let expected = change_a.event_id.max(change_b.event_id);
    let forward = resolve(&[genesis.clone(), change_a.clone(), change_b.clone()])?;
    let reverse = resolve(&[genesis, change_b, change_a])?;

    assert_eq!(forward, reverse);
    assert_eq!(
        forward
            .snapshot
            .home_server
            .map(|binding| binding.declaration),
        Some(expected)
    );
    Ok(())
}

#[test]
fn malformed_home_server_bindings_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([77; 32]);
    let server = Identity::from_seed([78; 32]);
    let genesis = create_chat_genesis(&owner, "Bounded hosting", 1)?;
    let community = community_id(&genesis)?;
    let empty = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        2,
        ChatPayload::HomeServerSet {
            server_public_key: server.public_key().to_vec(),
            endpoints: Vec::new(),
        },
    )?;
    let duplicate = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        3,
        ChatPayload::HomeServerSet {
            server_public_key: server.public_key().to_vec(),
            endpoints: vec![
                "https://same.example.test".into(),
                "https://same.example.test".into(),
            ],
        },
    )?;
    let bad_key = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        4,
        ChatPayload::HomeServerSet {
            server_public_key: vec![0; 31],
            endpoints: vec!["https://valid.example.test".into()],
        },
    )?;
    let resolution = resolve(&[genesis, empty.clone(), duplicate.clone(), bad_key.clone()])?;

    for event in [empty, duplicate, bad_key] {
        assert_eq!(
            resolution.rejected.get(&event.event_id),
            Some(&RejectionReason::InvalidHomeServer)
        );
    }
    assert!(resolution.snapshot.home_server.is_none());
    Ok(())
}

#[test]
fn revoked_administrator_cannot_authorize_later_events() -> Result<(), Box<dyn std::error::Error>> {
    let alice = Identity::from_seed([61; 32]);
    let bob = Identity::from_seed([62; 32]);
    let bob_invite_capability = InviteCapability::from_seed([102; 32]);
    let charlie_invite_capability = InviteCapability::from_seed([103; 32]);
    let genesis = create_chat_genesis(&alice, "Revocation", 1)?;
    let community = community_id(&genesis)?;
    let invite_bob = create_chat_event(
        &alice,
        community,
        vec![genesis.event_id],
        2,
        bob_invite_capability.invitation_payload(),
    )?;
    let accept_bob = accept_invitation(
        &bob_invite_capability,
        &invite_bob,
        community,
        &bob,
        vec![invite_bob.event_id],
        3,
    )?;
    let grant = create_chat_event(
        &alice,
        community,
        vec![accept_bob.event_id],
        4,
        ChatPayload::AdministratorGrant {
            member: bob.user_id(),
        },
    )?;
    let revoke = create_chat_event(
        &alice,
        community,
        vec![grant.event_id],
        5,
        ChatPayload::AdministratorRevoke {
            member: bob.user_id(),
        },
    )?;
    let unauthorized = create_chat_event(
        &bob,
        community,
        vec![revoke.event_id],
        6,
        charlie_invite_capability.invitation_payload(),
    )?;
    let resolution = resolve(&[
        genesis,
        invite_bob,
        accept_bob,
        grant,
        revoke,
        unauthorized.clone(),
    ])?;
    assert_eq!(
        resolution.rejected.get(&unauthorized.event_id),
        Some(&RejectionReason::NotAdministrator)
    );
    Ok(())
}

#[test]
fn bearer_invitation_requires_the_capability_and_is_consumed_once()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([64; 32]);
    let invited = Identity::from_seed([65; 32]);
    let other = Identity::from_seed([66; 32]);
    let capability = InviteCapability::from_seed([104; 32]);
    let genesis = create_chat_genesis(&owner, "Invitation defaults", 1)?;
    let community = community_id(&genesis)?;
    let invitation = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        2,
        capability.invitation_payload(),
    )?;
    let missing_capability_acceptance = create_chat_event(
        &other,
        community,
        vec![invitation.event_id],
        3,
        ChatPayload::MemberAccept {
            invitation: invitation.event_id,
            capability_signature: vec![0; 64],
        },
    )?;
    let acceptance = accept_invitation(
        &capability,
        &invitation,
        community,
        &invited,
        vec![invitation.event_id],
        4,
    )?;
    let repeated_acceptance = accept_invitation(
        &capability,
        &invitation,
        community,
        &other,
        vec![acceptance.event_id],
        5,
    )?;
    let resolution = resolve(&[
        genesis,
        invitation,
        missing_capability_acceptance.clone(),
        acceptance,
        repeated_acceptance.clone(),
    ])?;

    assert_eq!(
        resolution
            .rejected
            .get(&missing_capability_acceptance.event_id),
        Some(&RejectionReason::InvalidInvitation)
    );
    assert_eq!(
        resolution.rejected.get(&repeated_acceptance.event_id),
        Some(&RejectionReason::InvalidInvitation)
    );
    assert!(resolution.snapshot.members.contains(&invited.user_id()));
    assert!(!resolution.snapshot.members.contains(&other.user_id()));
    Ok(())
}

#[test]
fn invite_package_must_match_the_signed_invitation() -> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([67; 32]);
    let capability = InviteCapability::from_seed([105; 32]);
    let different_capability = InviteCapability::from_seed([106; 32]);
    let genesis = create_chat_genesis(&owner, "Package binding", 1)?;
    let community = community_id(&genesis)?;
    let invitation = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        2,
        capability.invitation_payload(),
    )?;
    let wrong_package = different_capability.encode_package(community, invitation.event_id)?;

    assert!(
        parse_invite_package(&wrong_package)?
            .validate(&invitation)
            .is_err()
    );
    Ok(())
}

#[test]
fn concurrent_redemptions_choose_one_bearer_deterministically()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = Identity::from_seed([68; 32]);
    let alice = Identity::from_seed([69; 32]);
    let bob = Identity::from_seed([70; 32]);
    let capability = InviteCapability::from_seed([107; 32]);
    let genesis = create_chat_genesis(&owner, "Concurrent redemption", 1)?;
    let community = community_id(&genesis)?;
    let invitation = create_chat_event(
        &owner,
        community,
        vec![genesis.event_id],
        2,
        capability.invitation_payload(),
    )?;
    let alice_acceptance = accept_invitation(
        &capability,
        &invitation,
        community,
        &alice,
        vec![invitation.event_id],
        3,
    )?;
    let bob_acceptance = accept_invitation(
        &capability,
        &invitation,
        community,
        &bob,
        vec![invitation.event_id],
        3,
    )?;
    let events = vec![
        genesis,
        invitation,
        alice_acceptance.clone(),
        bob_acceptance.clone(),
    ];
    let forward = resolve(&events)?;
    let reverse = resolve(&events.into_iter().rev().collect::<Vec<_>>())?;

    assert_eq!(forward, reverse);
    assert_eq!(
        usize::from(forward.snapshot.members.contains(&alice.user_id()))
            + usize::from(forward.snapshot.members.contains(&bob.user_id())),
        1
    );
    assert_eq!(forward.rejected.len(), 1);
    assert!(
        forward.rejected.contains_key(&alice_acceptance.event_id)
            || forward.rejected.contains_key(&bob_acceptance.event_id)
    );
    Ok(())
}

#[test]
fn oversized_invite_package_is_rejected_before_json_parsing() {
    assert!(matches!(
        parse_invite_package(&vec![b' '; MAX_INVITE_PACKAGE_BYTES + 1]),
        Err(InviteError::TooLarge)
    ));
}
