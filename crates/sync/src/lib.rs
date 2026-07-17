pub mod auth;
pub mod network;

use chatcommons_node_core::{CoreNode, NodeError};
use chatcommons_protocol::{CommunityId, EventId, PROTOCOL_VERSION, SignedEvent, validate_event};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const SYNC_VERSION: u16 = 1;
pub const MAX_SYNC_JSON_BYTES: usize = 512 * 1024;
pub const MAX_VERSIONS: usize = 8;
pub const MAX_IDS_PER_MESSAGE: usize = 64;
pub const MAX_EVENTS_PER_MESSAGE: usize = 64;
pub const MAX_PENDING_SYNC_EVENTS: usize = 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMessage {
    Hello {
        sync_version: u16,
        protocol_versions: Vec<u16>,
        community_id: CommunityId,
    },
    Heads {
        community_id: CommunityId,
        event_ids: Vec<EventId>,
    },
    Want {
        community_id: CommunityId,
        event_ids: Vec<EventId>,
    },
    Events {
        community_id: CommunityId,
        events: Vec<SignedEvent>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSyncMessage(SyncMessage);

impl ParsedSyncMessage {
    pub fn validate(self) -> Result<SyncMessage, SyncError> {
        validate_message(&self.0)?;
        Ok(self.0)
    }
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("encoded sync message exceeds {MAX_SYNC_JSON_BYTES} bytes")]
    TooLarge,
    #[error("sync message is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported sync or core protocol version")]
    UnsupportedVersion,
    #[error("sync message exceeds its item limit")]
    ItemLimitExceeded,
    #[error("event IDs must be sorted and unique")]
    InvalidEventIds,
    #[error("sync message is for another community")]
    WrongCommunity,
    #[error("event failed core validation: {0}")]
    Protocol(#[from] chatcommons_protocol::ProtocolError),
    #[error("node operation failed: {0}")]
    Node(#[from] NodeError),
}

pub fn parse_json(bytes: &[u8]) -> Result<ParsedSyncMessage, SyncError> {
    if bytes.len() > MAX_SYNC_JSON_BYTES {
        return Err(SyncError::TooLarge);
    }
    Ok(ParsedSyncMessage(serde_json::from_slice(bytes)?))
}

pub fn to_json(message: &SyncMessage) -> Result<Vec<u8>, SyncError> {
    validate_message(message)?;
    let bytes = serde_json::to_vec(message)?;
    if bytes.len() > MAX_SYNC_JSON_BYTES {
        return Err(SyncError::TooLarge);
    }
    Ok(bytes)
}

pub fn validate_message(message: &SyncMessage) -> Result<(), SyncError> {
    match message {
        SyncMessage::Hello {
            sync_version,
            protocol_versions,
            ..
        } => {
            if *sync_version != SYNC_VERSION
                || protocol_versions.is_empty()
                || protocol_versions.len() > MAX_VERSIONS
                || !protocol_versions.contains(&PROTOCOL_VERSION)
            {
                return Err(SyncError::UnsupportedVersion);
            }
        }
        SyncMessage::Heads { event_ids, .. } | SyncMessage::Want { event_ids, .. } => {
            validate_ids(event_ids)?;
        }
        SyncMessage::Events { events, .. } => {
            if events.is_empty() || events.len() > MAX_EVENTS_PER_MESSAGE {
                return Err(SyncError::ItemLimitExceeded);
            }
            if !events
                .windows(2)
                .all(|pair| pair[0].event_id < pair[1].event_id)
            {
                return Err(SyncError::InvalidEventIds);
            }
            for event in events {
                validate_event(event)?;
            }
        }
    }
    if serde_json::to_vec(message)?.len() > MAX_SYNC_JSON_BYTES {
        return Err(SyncError::TooLarge);
    }
    Ok(())
}

fn validate_ids(ids: &[EventId]) -> Result<(), SyncError> {
    if ids.len() > MAX_IDS_PER_MESSAGE {
        return Err(SyncError::ItemLimitExceeded);
    }
    if !ids.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(SyncError::InvalidEventIds);
    }
    Ok(())
}

pub struct SyncPeer {
    node: CoreNode,
    community: CommunityId,
    pending: BTreeMap<EventId, SignedEvent>,
}

impl SyncPeer {
    pub fn new(node: CoreNode, community: CommunityId) -> Result<Self, SyncError> {
        if node.community().is_some_and(|known| known != community) {
            return Err(SyncError::WrongCommunity);
        }
        Ok(Self {
            node,
            community,
            pending: BTreeMap::new(),
        })
    }

    pub fn hello(&self) -> SyncMessage {
        SyncMessage::Hello {
            sync_version: SYNC_VERSION,
            protocol_versions: vec![PROTOCOL_VERSION],
            community_id: self.community,
        }
    }

    pub fn receive(&mut self, message: SyncMessage) -> Result<Vec<SyncMessage>, SyncError> {
        validate_message(&message)?;
        if message_community(&message) != self.community {
            return Err(SyncError::WrongCommunity);
        }
        match message {
            SyncMessage::Hello { .. } => self.head_messages(),
            SyncMessage::Heads { event_ids, .. } => Ok(self.want_unknown(event_ids)),
            SyncMessage::Want { event_ids, .. } => self.serve(event_ids),
            SyncMessage::Events { events, .. } => self.accept(events),
        }
    }

    pub fn node(&self) -> &CoreNode {
        &self.node
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    fn head_messages(&self) -> Result<Vec<SyncMessage>, SyncError> {
        let heads = self.node.heads()?;
        if heads.is_empty() {
            return Ok(vec![SyncMessage::Heads {
                community_id: self.community,
                event_ids: Vec::new(),
            }]);
        }
        Ok(heads
            .chunks(MAX_IDS_PER_MESSAGE)
            .map(|chunk| SyncMessage::Heads {
                community_id: self.community,
                event_ids: chunk.to_vec(),
            })
            .collect())
    }

    fn want_unknown(&self, ids: Vec<EventId>) -> Vec<SyncMessage> {
        let wanted: Vec<EventId> = ids
            .into_iter()
            .filter(|id| !self.node.event_ids().contains(id) && !self.pending.contains_key(id))
            .collect();
        want_messages(self.community, wanted)
    }

    fn serve(&self, ids: Vec<EventId>) -> Result<Vec<SyncMessage>, SyncError> {
        let events = self.node.events(&ids)?;
        if events.is_empty() {
            return Ok(Vec::new());
        }
        pack_events(self.community, events)
    }

    fn accept(&mut self, events: Vec<SignedEvent>) -> Result<Vec<SyncMessage>, SyncError> {
        for event in &events {
            let belongs = match event.content.community_id {
                None => CommunityId::from(event.event_id) == self.community,
                Some(id) => id == self.community,
            };
            if !belongs {
                return Err(SyncError::WrongCommunity);
            }
        }
        let new_ids: BTreeSet<EventId> = events
            .iter()
            .map(|event| event.event_id)
            .filter(|id| !self.node.event_ids().contains(id) && !self.pending.contains_key(id))
            .collect();
        if self.pending.len() + new_ids.len() > MAX_PENDING_SYNC_EVENTS {
            return Err(SyncError::ItemLimitExceeded);
        }
        for event in events {
            if new_ids.contains(&event.event_id) {
                self.pending.entry(event.event_id).or_insert(event);
            }
        }

        let candidates: Vec<SignedEvent> = self.pending.values().cloned().collect();
        let can_ingest = self.node.community().is_some()
            || candidates.iter().any(|event| {
                event.content.community_id.is_none()
                    && CommunityId::from(event.event_id) == self.community
            });
        if can_ingest && !candidates.is_empty() {
            self.node.ingest(candidates)?;
            self.pending
                .retain(|id, _| !self.node.event_ids().contains(id));
        }

        let mut missing = BTreeSet::new();
        for event in self.pending.values() {
            for parent in &event.content.parents {
                if !self.node.event_ids().contains(parent) && !self.pending.contains_key(parent) {
                    missing.insert(*parent);
                }
            }
        }
        Ok(want_messages(self.community, missing.into_iter().collect()))
    }
}

fn message_community(message: &SyncMessage) -> CommunityId {
    match message {
        SyncMessage::Hello { community_id, .. }
        | SyncMessage::Heads { community_id, .. }
        | SyncMessage::Want { community_id, .. }
        | SyncMessage::Events { community_id, .. } => *community_id,
    }
}

fn want_messages(community: CommunityId, ids: Vec<EventId>) -> Vec<SyncMessage> {
    ids.chunks(MAX_IDS_PER_MESSAGE)
        .map(|chunk| SyncMessage::Want {
            community_id: community,
            event_ids: chunk.to_vec(),
        })
        .collect()
}

fn pack_events(
    community: CommunityId,
    events: Vec<SignedEvent>,
) -> Result<Vec<SyncMessage>, SyncError> {
    let mut messages = Vec::new();
    let mut batch = Vec::new();
    for event in events {
        let mut candidate = batch.clone();
        candidate.push(event.clone());
        let candidate_message = SyncMessage::Events {
            community_id: community,
            events: candidate.clone(),
        };
        let candidate_result = validate_message(&candidate_message);
        if batch.len() == MAX_EVENTS_PER_MESSAGE
            || matches!(&candidate_result, Err(SyncError::TooLarge))
        {
            if batch.is_empty() {
                candidate_result?;
            }
            messages.push(SyncMessage::Events {
                community_id: community,
                events: std::mem::take(&mut batch),
            });
            batch.push(event);
            validate_message(&SyncMessage::Events {
                community_id: community,
                events: batch.clone(),
            })?;
        } else {
            candidate_result?;
            batch = candidate;
        }
    }
    if !batch.is_empty() {
        messages.push(SyncMessage::Events {
            community_id: community,
            events: batch,
        });
    }
    Ok(messages)
}
