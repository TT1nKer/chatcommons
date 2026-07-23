use chatcommons_protocol::{CommunityId, EventId, SignedEvent, validate_event};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const ARCHIVE_VERSION: u16 = 1;
pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_ARCHIVE_EVENTS: usize = 65_536;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchiveWire {
    version: u16,
    community_id: CommunityId,
    events: Vec<SignedEvent>,
}

pub struct ParsedArchive(ArchiveWire);

pub struct ValidatedArchive {
    community: CommunityId,
    events: Vec<SignedEvent>,
}

impl ParsedArchive {
    pub fn validate(self) -> Result<ValidatedArchive, ArchiveError> {
        validate_wire(self.0)
    }
}

impl ValidatedArchive {
    pub fn community(&self) -> CommunityId {
        self.community
    }

    pub fn events(&self) -> &[SignedEvent] {
        &self.events
    }

    pub fn into_parts(self) -> (CommunityId, Vec<SignedEvent>) {
        (self.community, self.events)
    }
}

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("community archive exceeds {MAX_ARCHIVE_BYTES} bytes")]
    TooLarge,
    #[error("community archive is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported community archive version")]
    UnsupportedVersion,
    #[error("community archive must contain between 1 and {MAX_ARCHIVE_EVENTS} events")]
    EventLimit,
    #[error("community archive events must be sorted and unique")]
    InvalidOrder,
    #[error("community archive must contain exactly one matching genesis")]
    InvalidGenesis,
    #[error("community archive contains an event from another community")]
    WrongCommunity,
    #[error("community archive is missing a referenced parent")]
    MissingParent,
    #[error("community archive event failed protocol validation: {0}")]
    Protocol(#[from] chatcommons_protocol::ProtocolError),
}

pub fn parse(bytes: &[u8]) -> Result<ParsedArchive, ArchiveError> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(ArchiveError::TooLarge);
    }
    Ok(ParsedArchive(serde_json::from_slice(bytes)?))
}

pub fn encode(
    community: CommunityId,
    mut events: Vec<SignedEvent>,
) -> Result<Vec<u8>, ArchiveError> {
    events.sort_by_key(|event| event.event_id);
    let validated = validate_wire(ArchiveWire {
        version: ARCHIVE_VERSION,
        community_id: community,
        events,
    })?;
    let bytes = serde_json::to_vec(&ArchiveWire {
        version: ARCHIVE_VERSION,
        community_id: validated.community,
        events: validated.events,
    })?;
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(ArchiveError::TooLarge);
    }
    Ok(bytes)
}

fn validate_wire(wire: ArchiveWire) -> Result<ValidatedArchive, ArchiveError> {
    if wire.version != ARCHIVE_VERSION {
        return Err(ArchiveError::UnsupportedVersion);
    }
    if wire.events.is_empty() || wire.events.len() > MAX_ARCHIVE_EVENTS {
        return Err(ArchiveError::EventLimit);
    }
    if !wire
        .events
        .windows(2)
        .all(|pair| pair[0].event_id < pair[1].event_id)
    {
        return Err(ArchiveError::InvalidOrder);
    }
    for event in &wire.events {
        validate_event(event)?;
    }
    let genesis: Vec<&SignedEvent> = wire
        .events
        .iter()
        .filter(|event| event.content.community_id.is_none())
        .collect();
    if !matches!(genesis.as_slice(), [event] if CommunityId::from(event.event_id) == wire.community_id)
    {
        return Err(ArchiveError::InvalidGenesis);
    }
    if wire.events.iter().any(|event| {
        event
            .content
            .community_id
            .is_some_and(|community| community != wire.community_id)
    }) {
        return Err(ArchiveError::WrongCommunity);
    }
    let event_ids: BTreeSet<EventId> = wire.events.iter().map(|event| event.event_id).collect();
    if wire.events.iter().any(|event| {
        event
            .content
            .parents
            .iter()
            .any(|parent| !event_ids.contains(parent))
    }) {
        return Err(ArchiveError::MissingParent);
    }
    Ok(ValidatedArchive {
        community: wire.community_id,
        events: wire.events,
    })
}
