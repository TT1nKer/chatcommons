use chatcommons_protocol::{CommunityId, EventId, SignedEvent, validate_event};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    AlreadyPresent,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("stored event is corrupt: {0}")]
    Corrupt(#[from] serde_json::Error),
    #[error("event failed protocol validation: {0}")]
    Protocol(#[from] chatcommons_protocol::ProtocolError),
}

pub struct EventStore {
    connection: Connection,
}

impl EventStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS events (event_id BLOB PRIMARY KEY, community_id BLOB, body BLOB NOT NULL);")?;
        Ok(Self { connection })
    }

    pub fn insert(&mut self, event: &SignedEvent) -> Result<InsertOutcome, StorageError> {
        validate_event(event)?;
        let body = serde_json::to_vec(event)?;
        let community = event.content.community_id.map(|id| id.as_bytes().to_vec());
        let changed = self.connection.execute(
            "INSERT OR IGNORE INTO events(event_id,community_id,body) VALUES(?1,?2,?3)",
            params![event.event_id.as_bytes().as_slice(), community, body],
        )?;
        Ok(if changed == 1 {
            InsertOutcome::Inserted
        } else {
            InsertOutcome::AlreadyPresent
        })
    }

    pub fn get(&self, id: EventId) -> Result<Option<SignedEvent>, StorageError> {
        let body: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT body FROM events WHERE event_id=?1",
                [id.as_bytes().as_slice()],
                |row| row.get(0),
            )
            .optional()?;
        body.map(|bytes| decode_stored(&bytes)).transpose()
    }

    pub fn events(&self, community: CommunityId) -> Result<Vec<SignedEvent>, StorageError> {
        let mut statement = self.connection.prepare("SELECT body FROM events WHERE community_id=?1 OR (community_id IS NULL AND event_id=?1) ORDER BY event_id")?;
        let rows = statement.query_map([community.as_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })?;
        rows.map(|row| decode_stored(&row?)).collect()
    }
}

fn decode_stored(bytes: &[u8]) -> Result<SignedEvent, StorageError> {
    let event = serde_json::from_slice(bytes)?;
    validate_event(&event)?;
    Ok(event)
}
