use chatcommons_protocol::{CommunityId, EventId, SignedEvent, community_id, validate_event};
use chatcommons_storage::{EventStore, InsertOutcome, StorageError};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const MAX_PENDING_EVENTS: usize = 1024;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("protocol validation failed: {0}")]
    Protocol(#[from] chatcommons_protocol::ProtocolError),
    #[error("storage failed: {0}")]
    Storage(#[from] StorageError),
    #[error("unknown community")]
    UnknownCommunity,
    #[error("event belongs to another community")]
    WrongCommunity,
    #[error("event has a missing parent")]
    MissingParent,
    #[error("batch exceeds the pending event limit")]
    PendingLimitExceeded,
    #[error("requested event is unknown")]
    UnknownEvent,
    #[error("event ancestry exceeds its limit")]
    AncestryLimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestReport {
    pub inserted: usize,
    pub already_present: usize,
    pub unresolved: Vec<EventId>,
}

pub struct CoreNode {
    store: EventStore,
    community: Option<CommunityId>,
    known: BTreeSet<EventId>,
}

impl CoreNode {
    pub fn open(store: EventStore, community: Option<CommunityId>) -> Result<Self, NodeError> {
        let mut node = Self {
            store,
            community,
            known: BTreeSet::new(),
        };
        if let Some(id) = community {
            let events = node.store.events(id)?;
            if events.is_empty() {
                return Err(NodeError::UnknownCommunity);
            }
            node.known
                .extend(events.into_iter().map(|event| event.event_id));
        }
        Ok(node)
    }

    pub fn ingest(&mut self, events: Vec<SignedEvent>) -> Result<IngestReport, NodeError> {
        if events.len() > MAX_PENDING_EVENTS {
            return Err(NodeError::PendingLimitExceeded);
        }
        for event in &events {
            validate_event(event)?;
        }
        let community = match self.community {
            Some(id) => id,
            None => events
                .iter()
                .find(|event| event.content.community_id.is_none())
                .map(community_id)
                .transpose()?
                .ok_or(NodeError::UnknownCommunity)?,
        };
        if events.iter().any(|event| match event.content.community_id {
            None => CommunityId::from(event.event_id) != community,
            Some(id) => id != community,
        }) {
            return Err(NodeError::WrongCommunity);
        }

        let mut pending: BTreeMap<EventId, SignedEvent> = events
            .into_iter()
            .map(|event| (event.event_id, event))
            .collect();
        let mut inserted = 0;
        let mut already_present = 0;
        loop {
            let ready: Vec<EventId> = pending
                .iter()
                .filter(|(_, event)| {
                    event
                        .content
                        .parents
                        .iter()
                        .all(|parent| self.known.contains(parent))
                })
                .map(|(id, _)| *id)
                .collect();
            if ready.is_empty() {
                break;
            }
            for id in ready {
                let event = pending.remove(&id).ok_or(NodeError::MissingParent)?;
                match self.store.insert(&event)? {
                    InsertOutcome::Inserted => inserted += 1,
                    InsertOutcome::AlreadyPresent => already_present += 1,
                }
                self.known.insert(id);
                if event.content.community_id.is_none() {
                    self.community = Some(community);
                }
            }
        }
        let unresolved = pending.keys().copied().collect();
        Ok(IngestReport {
            inserted,
            already_present,
            unresolved,
        })
    }

    pub fn event(&self, id: EventId) -> Result<Option<SignedEvent>, NodeError> {
        Ok(self.store.get(id)?)
    }

    pub fn events(&self, ids: &[EventId]) -> Result<Vec<SignedEvent>, NodeError> {
        let mut events = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(event) = self.store.get(*id)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    pub fn all_events(&self) -> Result<Vec<SignedEvent>, NodeError> {
        let ids: Vec<EventId> = self.known.iter().copied().collect();
        self.events(&ids)
    }

    pub fn stored_event_bytes(&self) -> Result<u64, NodeError> {
        Ok(self.store.stored_event_bytes()?)
    }

    pub fn ancestry(&self, event_id: EventId, limit: usize) -> Result<Vec<SignedEvent>, NodeError> {
        if limit == 0 {
            return Err(NodeError::AncestryLimitExceeded);
        }
        let mut pending = vec![event_id];
        let mut discovered = BTreeSet::new();
        let mut events = BTreeMap::new();
        while let Some(id) = pending.pop() {
            if !discovered.insert(id) {
                continue;
            }
            if discovered.len() > limit {
                return Err(NodeError::AncestryLimitExceeded);
            }
            let event = self.event(id)?.ok_or(NodeError::UnknownEvent)?;
            pending.extend(event.content.parents.iter().copied());
            events.insert(id, event);
        }
        Ok(events.into_values().collect())
    }

    pub fn heads(&self) -> Result<Vec<EventId>, NodeError> {
        let mut referenced = BTreeSet::new();
        for id in &self.known {
            if let Some(event) = self.store.get(*id)? {
                referenced.extend(event.content.parents);
            }
        }
        Ok(self.known.difference(&referenced).copied().collect())
    }

    pub fn event_ids(&self) -> &BTreeSet<EventId> {
        &self.known
    }
    pub fn community(&self) -> Option<CommunityId> {
        self.community
    }
}
