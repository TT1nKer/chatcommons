use chatcommons_crypto::Identity;
use chatcommons_profile_chat::{
    ChatPayload, InviteCapability, InviteError, MAX_INVITE_PACKAGE_BYTES, RejectionReason,
    create_chat_event, create_chat_genesis, parse_invite_package, resolve,
};
use chatcommons_protocol::{CommunityId, SignedEvent, community_id};

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
