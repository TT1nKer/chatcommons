use chatcommons_crypto::{Identity, PUBLIC_KEY_LEN, SIGNATURE_LEN, UserId, verify};
use chatcommons_protocol::{
    CommunityId, EventContent, EventId, PROTOCOL_VERSION, ProtocolError, SignedEvent, author_id,
    create_genesis, create_signed, validate_event,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const PROFILE_ID: &str = "chatcommons.chat.v1";
pub const GENESIS_TYPE: &str = "chat.community.create";
pub const INVITE_PACKAGE_VERSION: u16 = 1;
pub const MAX_INVITE_PACKAGE_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatPayload {
    CommunityCreate {
        profile: String,
        name: String,
    },
    ChannelCreate {
        channel_id: [u8; 32],
        name: String,
    },
    MemberInvite {
        capability_public_key: Vec<u8>,
    },
    MemberAccept {
        invitation: EventId,
        capability_signature: Vec<u8>,
    },
    MemberRemove {
        member: UserId,
    },
    MessageCreate {
        channel_id: [u8; 32],
        text: String,
    },
    AdministratorGrant {
        member: UserId,
    },
    AdministratorRevoke {
        member: UserId,
    },
    OwnershipTransfer {
        new_owner: UserId,
    },
}

impl ChatPayload {
    fn event_type(&self) -> &'static str {
        match self {
            Self::CommunityCreate { .. } => GENESIS_TYPE,
            Self::ChannelCreate { .. } => "chat.channel.create",
            Self::MemberInvite { .. } => "chat.member.invite",
            Self::MemberAccept { .. } => "chat.member.accept",
            Self::MemberRemove { .. } => "chat.member.remove",
            Self::MessageCreate { .. } => "chat.message.create",
            Self::AdministratorGrant { .. } => "chat.admin.grant",
            Self::AdministratorRevoke { .. } => "chat.admin.revoke",
            Self::OwnershipTransfer { .. } => "chat.ownership.transfer",
        }
    }
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("core protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("chat payload is malformed: {0}")]
    Payload(#[from] serde_json::Error),
    #[error("event type does not match chat payload")]
    TypeMismatch,
}

#[derive(Debug, Error)]
pub enum InviteError {
    #[error("invite package exceeds {MAX_INVITE_PACKAGE_BYTES} bytes")]
    TooLarge,
    #[error("invite package is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invite package version is unsupported")]
    UnsupportedVersion,
    #[error("invite package secret has an invalid length")]
    InvalidSecret,
    #[error("invite package does not match its signed invitation event")]
    InvitationMismatch,
    #[error("invitation event is invalid: {0}")]
    InvalidInvitationEvent(#[from] ChatError),
}

pub struct InviteCapability {
    seed: [u8; 32],
}

impl InviteCapability {
    pub fn generate() -> Self {
        let mut seed = [0_u8; 32];
        OsRng.fill_bytes(&mut seed);
        Self { seed }
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self { seed }
    }

    pub fn public_key(&self) -> [u8; PUBLIC_KEY_LEN] {
        Identity::from_seed(self.seed).public_key()
    }

    pub fn invitation_payload(&self) -> ChatPayload {
        ChatPayload::MemberInvite {
            capability_public_key: self.public_key().to_vec(),
        }
    }

    pub fn encode_package(
        &self,
        community: CommunityId,
        invitation: EventId,
    ) -> Result<Vec<u8>, InviteError> {
        Ok(serde_json::to_vec(&InvitePackage {
            version: INVITE_PACKAGE_VERSION,
            community,
            invitation,
            capability_secret: self.seed.to_vec(),
        })?)
    }

    fn sign_acceptance(
        &self,
        community: CommunityId,
        invitation: EventId,
        invitee: UserId,
    ) -> Vec<u8> {
        Identity::from_seed(self.seed)
            .sign(&acceptance_bytes(community, invitation, invitee))
            .to_vec()
    }
}

impl Drop for InviteCapability {
    fn drop(&mut self) {
        self.seed.fill(0);
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitePackage {
    version: u16,
    community: CommunityId,
    invitation: EventId,
    capability_secret: Vec<u8>,
}

pub struct ParsedInvitePackage(InvitePackage);

pub struct ValidatedInvite {
    community: CommunityId,
    invitation: EventId,
    capability: InviteCapability,
}

pub fn parse_invite_package(bytes: &[u8]) -> Result<ParsedInvitePackage, InviteError> {
    if bytes.len() > MAX_INVITE_PACKAGE_BYTES {
        return Err(InviteError::TooLarge);
    }
    Ok(ParsedInvitePackage(serde_json::from_slice(bytes)?))
}

impl ParsedInvitePackage {
    pub fn validate(self, invitation_event: &SignedEvent) -> Result<ValidatedInvite, InviteError> {
        if self.0.version != INVITE_PACKAGE_VERSION {
            return Err(InviteError::UnsupportedVersion);
        }
        let seed: [u8; 32] = self
            .0
            .capability_secret
            .as_slice()
            .try_into()
            .map_err(|_| InviteError::InvalidSecret)?;
        let capability = InviteCapability::from_seed(seed);
        validate_event(invitation_event).map_err(ChatError::from)?;
        let payload = decode(invitation_event)?;
        let matches = invitation_event.event_id == self.0.invitation
            && invitation_event.content.community_id == Some(self.0.community)
            && matches!(
                payload,
                ChatPayload::MemberInvite { capability_public_key }
                    if capability_public_key == capability.public_key()
            );
        if !matches {
            return Err(InviteError::InvitationMismatch);
        }
        Ok(ValidatedInvite {
            community: self.0.community,
            invitation: self.0.invitation,
            capability,
        })
    }
}

impl ValidatedInvite {
    pub fn create_acceptance(
        &self,
        identity: &Identity,
        parents: Vec<EventId>,
        timestamp_ms: i64,
    ) -> Result<SignedEvent, ChatError> {
        create_chat_event(
            identity,
            self.community,
            parents,
            timestamp_ms,
            ChatPayload::MemberAccept {
                invitation: self.invitation,
                capability_signature: self.capability.sign_acceptance(
                    self.community,
                    self.invitation,
                    identity.user_id(),
                ),
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    MissingParent,
    ParentRejected,
    WrongCommunity,
    NotMember,
    NotAdministrator,
    NotOwner,
    InvalidInvitation,
    MemberNotActive,
    AlreadyAdministrator,
    CannotRemoveOwner,
    UnknownChannel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSnapshot {
    pub community: Option<CommunityId>,
    pub owner: Option<UserId>,
    pub members: BTreeSet<UserId>,
    pub administrators: BTreeSet<UserId>,
    pub channels: BTreeSet<[u8; 32]>,
    pub event_ids: BTreeSet<EventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatResolution {
    pub accepted_in_order: Vec<EventId>,
    pub rejected: BTreeMap<EventId, RejectionReason>,
    pub snapshot: ChatSnapshot,
}

#[derive(Default)]
struct State {
    community: Option<CommunityId>,
    owner: Option<UserId>,
    members: BTreeSet<UserId>,
    administrators: BTreeSet<UserId>,
    channels: BTreeSet<[u8; 32]>,
    invitations: BTreeMap<EventId, Vec<u8>>,
    accepted_invitations: BTreeSet<EventId>,
}

pub fn create_chat_genesis(
    identity: &Identity,
    name: &str,
    timestamp_ms: i64,
) -> Result<SignedEvent, ChatError> {
    let payload = ChatPayload::CommunityCreate {
        profile: PROFILE_ID.into(),
        name: name.into(),
    };
    Ok(create_genesis(
        identity,
        GENESIS_TYPE,
        serde_json::to_vec(&payload)?,
        timestamp_ms,
    ))
}

pub fn create_chat_event(
    identity: &Identity,
    community: CommunityId,
    parents: Vec<EventId>,
    timestamp_ms: i64,
    payload: ChatPayload,
) -> Result<SignedEvent, ChatError> {
    let event_type = payload.event_type().into();
    let payload = serde_json::to_vec(&payload)?;
    Ok(create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: Some(community),
            parents,
            timestamp_ms,
            event_type,
            payload,
        },
        identity,
    ))
}

pub fn decode(event: &SignedEvent) -> Result<ChatPayload, ChatError> {
    let payload: ChatPayload = serde_json::from_slice(&event.content.payload)?;
    if payload.event_type() != event.content.event_type {
        return Err(ChatError::TypeMismatch);
    }
    if let ChatPayload::CommunityCreate { profile, .. } = &payload
        && profile != PROFILE_ID
    {
        return Err(ChatError::TypeMismatch);
    }
    Ok(payload)
}

pub fn resolve(events: &[SignedEvent]) -> Result<ChatResolution, ChatError> {
    let mut by_id = BTreeMap::new();
    let mut payloads = BTreeMap::new();
    let mut authors = BTreeMap::new();
    for event in events {
        validate_event(event)?;
        payloads.insert(event.event_id, decode(event)?);
        authors.insert(event.event_id, author_id(event)?);
        by_id.entry(event.event_id).or_insert_with(|| event.clone());
    }
    let mut state = State::default();
    let mut accepted = BTreeSet::new();
    let mut accepted_in_order = Vec::new();
    let mut rejected = BTreeMap::new();
    let mut pending: BTreeSet<EventId> = by_id.keys().copied().collect();
    loop {
        let mut ready: Vec<EventId> = pending
            .iter()
            .copied()
            .filter(|id| {
                by_id[id]
                    .content
                    .parents
                    .iter()
                    .all(|parent| !pending.contains(parent))
            })
            .collect();
        if ready.is_empty() {
            break;
        }
        ready.sort_by_key(|id| (priority(&payloads[id]), *id));
        for id in ready {
            pending.remove(&id);
            let event = &by_id[&id];
            if event
                .content
                .parents
                .iter()
                .any(|parent| !by_id.contains_key(parent))
            {
                rejected.insert(id, RejectionReason::MissingParent);
                continue;
            }
            if event
                .content
                .parents
                .iter()
                .any(|parent| rejected.contains_key(parent))
            {
                rejected.insert(id, RejectionReason::ParentRejected);
                continue;
            }
            let payload = &payloads[&id];
            let author = authors[&id];
            match validate_context(&state, event, payload, author, &accepted) {
                Ok(()) => {
                    apply(&mut state, event, payload, author);
                    accepted.insert(id);
                    accepted_in_order.push(id);
                }
                Err(reason) => {
                    rejected.insert(id, reason);
                }
            }
        }
    }
    for id in pending {
        rejected.insert(id, RejectionReason::MissingParent);
    }
    let snapshot = ChatSnapshot {
        community: state.community,
        owner: state.owner,
        members: state.members,
        administrators: state.administrators,
        channels: state.channels,
        event_ids: accepted,
    };
    Ok(ChatResolution {
        accepted_in_order,
        rejected,
        snapshot,
    })
}

fn priority(payload: &ChatPayload) -> u8 {
    match payload {
        ChatPayload::CommunityCreate { .. } => 0,
        ChatPayload::MemberRemove { .. } | ChatPayload::AdministratorRevoke { .. } => 1,
        _ => 2,
    }
}

fn validate_context(
    state: &State,
    event: &SignedEvent,
    payload: &ChatPayload,
    author: UserId,
    accepted: &BTreeSet<EventId>,
) -> Result<(), RejectionReason> {
    if matches!(payload, ChatPayload::CommunityCreate { .. }) {
        return if state.community.is_none() {
            Ok(())
        } else {
            Err(RejectionReason::WrongCommunity)
        };
    }
    if event.content.community_id != state.community {
        return Err(RejectionReason::WrongCommunity);
    }
    if event
        .content
        .parents
        .iter()
        .any(|parent| !accepted.contains(parent))
    {
        return Err(RejectionReason::ParentRejected);
    }
    match payload {
        ChatPayload::CommunityCreate { .. } => Ok(()),
        ChatPayload::ChannelCreate { .. } => require_admin(state, author),
        ChatPayload::MemberInvite {
            capability_public_key,
        } => {
            require_admin(state, author)?;
            if capability_public_key.len() == PUBLIC_KEY_LEN {
                Ok(())
            } else {
                Err(RejectionReason::InvalidInvitation)
            }
        }
        ChatPayload::MemberAccept {
            invitation,
            capability_signature,
        } => {
            let public_key = state
                .invitations
                .get(invitation)
                .ok_or(RejectionReason::InvalidInvitation)?;
            if state.accepted_invitations.contains(invitation)
                || capability_signature.len() != SIGNATURE_LEN
            {
                return Err(RejectionReason::InvalidInvitation);
            }
            verify(
                public_key,
                &acceptance_bytes(
                    event
                        .content
                        .community_id
                        .ok_or(RejectionReason::WrongCommunity)?,
                    *invitation,
                    author,
                ),
                capability_signature,
            )
            .map_err(|_| RejectionReason::InvalidInvitation)
        }
        ChatPayload::MemberRemove { member } => {
            require_admin(state, author)?;
            if state.owner == Some(*member) {
                Err(RejectionReason::CannotRemoveOwner)
            } else if state.members.contains(member) {
                Ok(())
            } else {
                Err(RejectionReason::MemberNotActive)
            }
        }
        ChatPayload::MessageCreate { channel_id, .. } => {
            if !state.members.contains(&author) {
                Err(RejectionReason::NotMember)
            } else if !state.channels.contains(channel_id) {
                Err(RejectionReason::UnknownChannel)
            } else {
                Ok(())
            }
        }
        ChatPayload::AdministratorGrant { member } => {
            require_owner(state, author)?;
            if !state.members.contains(member) {
                Err(RejectionReason::MemberNotActive)
            } else if state.administrators.contains(member) {
                Err(RejectionReason::AlreadyAdministrator)
            } else {
                Ok(())
            }
        }
        ChatPayload::AdministratorRevoke { member } => {
            require_owner(state, author)?;
            if state.owner == Some(*member) {
                Err(RejectionReason::CannotRemoveOwner)
            } else if state.administrators.contains(member) {
                Ok(())
            } else {
                Err(RejectionReason::NotAdministrator)
            }
        }
        ChatPayload::OwnershipTransfer { new_owner } => {
            require_owner(state, author)?;
            if state.members.contains(new_owner) {
                Ok(())
            } else {
                Err(RejectionReason::MemberNotActive)
            }
        }
    }
}

fn require_admin(state: &State, author: UserId) -> Result<(), RejectionReason> {
    if state.administrators.contains(&author) {
        Ok(())
    } else {
        Err(RejectionReason::NotAdministrator)
    }
}
fn require_owner(state: &State, author: UserId) -> Result<(), RejectionReason> {
    if state.owner == Some(author) {
        Ok(())
    } else {
        Err(RejectionReason::NotOwner)
    }
}

fn apply(state: &mut State, event: &SignedEvent, payload: &ChatPayload, author: UserId) {
    match payload {
        ChatPayload::CommunityCreate { .. } => {
            state.community = Some(event.event_id.into());
            state.owner = Some(author);
            state.members.insert(author);
            state.administrators.insert(author);
        }
        ChatPayload::ChannelCreate { channel_id, .. } => {
            state.channels.insert(*channel_id);
        }
        ChatPayload::MemberInvite {
            capability_public_key,
        } => {
            state
                .invitations
                .insert(event.event_id, capability_public_key.clone());
        }
        ChatPayload::MemberAccept { invitation, .. } => {
            state.accepted_invitations.insert(*invitation);
            state.members.insert(author);
        }
        ChatPayload::MemberRemove { member } => {
            state.members.remove(member);
            state.administrators.remove(member);
        }
        ChatPayload::MessageCreate { .. } => {}
        ChatPayload::AdministratorGrant { member } => {
            state.administrators.insert(*member);
        }
        ChatPayload::AdministratorRevoke { member } => {
            state.administrators.remove(member);
        }
        ChatPayload::OwnershipTransfer { new_owner } => {
            state.owner = Some(*new_owner);
            state.administrators.insert(*new_owner);
        }
    }
}

fn acceptance_bytes(community: CommunityId, invitation: EventId, invitee: UserId) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(97);
    bytes.extend_from_slice(b"chatcommons:invite-acceptance:v1\0");
    bytes.extend_from_slice(community.as_bytes());
    bytes.extend_from_slice(invitation.as_bytes());
    bytes.extend_from_slice(invitee.as_bytes());
    bytes
}
