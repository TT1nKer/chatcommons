use chatcommons_crypto::{Identity, PUBLIC_KEY_LEN, SIGNATURE_LEN, UserId};
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

pub const PROTOCOL_VERSION: u16 = 2;
pub const MAX_EVENT_JSON_BYTES: usize = 256 * 1024;
pub const MAX_PARENTS: usize = 32;
pub const MAX_EVENT_TYPE_BYTES: usize = 64;
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name([u8; 32]);
        impl $name {
            pub fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            pub fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&hex::encode(self.0))
            }
        }
    };
}

id_type!(EventId);
id_type!(CommunityId);

impl From<EventId> for CommunityId {
    fn from(value: EventId) -> Self {
        Self(*value.as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventContent {
    pub protocol_version: u16,
    pub community_id: Option<CommunityId>,
    pub parents: Vec<EventId>,
    pub timestamp_ms: i64,
    pub event_type: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedEvent {
    pub content: EventContent,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
    pub event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEvent(SignedEvent);

impl ParsedEvent {
    pub fn validate(self) -> Result<SignedEvent, ProtocolError> {
        validate_event(&self.0)?;
        Ok(self.0)
    }
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("encoded event exceeds {MAX_EVENT_JSON_BYTES} bytes")]
    TooLarge,
    #[error("event is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unsupported protocol version")]
    UnsupportedVersion,
    #[error("event type is empty or exceeds its protocol limit")]
    InvalidEventType,
    #[error("payload exceeds its protocol limit")]
    PayloadTooLarge,
    #[error("event has too many parents")]
    TooManyParents,
    #[error("parents must be sorted and unique")]
    InvalidParents,
    #[error("genesis must have no parents")]
    InvalidGenesis,
    #[error("non-genesis event must have a community and at least one parent")]
    InvalidCommunityReference,
    #[error("public key length is invalid")]
    InvalidPublicKeyLength,
    #[error("signature length is invalid")]
    InvalidSignatureLength,
    #[error("signature verification failed")]
    InvalidSignature,
    #[error("event id does not match canonical event bytes")]
    EventIdMismatch,
}

pub fn parse_json(bytes: &[u8]) -> Result<ParsedEvent, ParseError> {
    if bytes.len() > MAX_EVENT_JSON_BYTES {
        return Err(ParseError::TooLarge);
    }
    Ok(ParsedEvent(serde_json::from_slice(bytes)?))
}

pub fn to_json(event: &SignedEvent) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(event)
}

pub fn create_signed(mut content: EventContent, identity: &Identity) -> SignedEvent {
    content.parents.sort();
    content.parents.dedup();
    let canonical = canonical_content(&content);
    let public_key = identity.public_key();
    let signature = identity.sign(&canonical);
    let event_id = calculate_event_id(&canonical, &public_key, &signature);
    SignedEvent {
        content,
        public_key: public_key.to_vec(),
        signature: signature.to_vec(),
        event_id,
    }
}

pub fn create_genesis(
    identity: &Identity,
    event_type: impl Into<String>,
    payload: Vec<u8>,
    timestamp_ms: i64,
) -> SignedEvent {
    create_signed(
        EventContent {
            protocol_version: PROTOCOL_VERSION,
            community_id: None,
            parents: vec![],
            timestamp_ms,
            event_type: event_type.into(),
            payload,
        },
        identity,
    )
}

pub fn community_id(genesis: &SignedEvent) -> Result<CommunityId, ProtocolError> {
    validate_event(genesis)?;
    if genesis.content.community_id.is_some() {
        return Err(ProtocolError::InvalidGenesis);
    }
    Ok(genesis.event_id.into())
}

pub fn author_id(event: &SignedEvent) -> Result<UserId, ProtocolError> {
    let public_key: [u8; PUBLIC_KEY_LEN] = event
        .public_key
        .as_slice()
        .try_into()
        .map_err(|_| ProtocolError::InvalidPublicKeyLength)?;
    Ok(UserId::from_public_key(&public_key))
}

pub fn validate_event(event: &SignedEvent) -> Result<(), ProtocolError> {
    let content = &event.content;
    if content.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion);
    }
    if content.event_type.is_empty() || content.event_type.len() > MAX_EVENT_TYPE_BYTES {
        return Err(ProtocolError::InvalidEventType);
    }
    if content.payload.len() > MAX_PAYLOAD_BYTES {
        return Err(ProtocolError::PayloadTooLarge);
    }
    if content.parents.len() > MAX_PARENTS {
        return Err(ProtocolError::TooManyParents);
    }
    if !strictly_sorted(&content.parents) {
        return Err(ProtocolError::InvalidParents);
    }
    match content.community_id {
        None if !content.parents.is_empty() => return Err(ProtocolError::InvalidGenesis),
        Some(_) if content.parents.is_empty() => {
            return Err(ProtocolError::InvalidCommunityReference);
        }
        _ => {}
    }
    let public_key: [u8; PUBLIC_KEY_LEN] = event
        .public_key
        .as_slice()
        .try_into()
        .map_err(|_| ProtocolError::InvalidPublicKeyLength)?;
    let signature: [u8; SIGNATURE_LEN] = event
        .signature
        .as_slice()
        .try_into()
        .map_err(|_| ProtocolError::InvalidSignatureLength)?;
    let canonical = canonical_content(content);
    chatcommons_crypto::verify(&public_key, &canonical, &signature)
        .map_err(|_| ProtocolError::InvalidSignature)?;
    if calculate_event_id(&canonical, &public_key, &signature) != event.event_id {
        return Err(ProtocolError::EventIdMismatch);
    }
    Ok(())
}

fn strictly_sorted(values: &[EventId]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn calculate_event_id(canonical: &[u8], public_key: &[u8; 32], signature: &[u8; 64]) -> EventId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"chatcommons:event:v2\0");
    hasher.update(canonical);
    hasher.update(public_key);
    hasher.update(signature);
    EventId::from_bytes(*hasher.finalize().as_bytes())
}

pub fn canonical_content(content: &EventContent) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"chatcommons:content:v2\0");
    out.extend_from_slice(&content.protocol_version.to_be_bytes());
    match content.community_id {
        Some(id) => {
            out.push(1);
            out.extend_from_slice(id.as_bytes());
        }
        None => out.push(0),
    }
    out.extend_from_slice(&(content.parents.len() as u32).to_be_bytes());
    for parent in &content.parents {
        out.extend_from_slice(parent.as_bytes());
    }
    out.extend_from_slice(&content.timestamp_ms.to_be_bytes());
    put_bytes(&mut out, content.event_type.as_bytes());
    put_bytes(&mut out, &content.payload);
    out
}

fn put_bytes(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u32).to_be_bytes());
    out.extend_from_slice(value);
}
