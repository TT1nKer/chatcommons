use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chatcommons_crypto::UserId;
use chatcommons_protocol::{CommunityId, EventId};
use libp2p::{Multiaddr, PeerId, multiaddr::Protocol};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

pub const BOOTSTRAP_VERSION: u16 = 1;
pub const CODE_PREFIX: &str = "cc1_";
pub const MAX_INVITE_PACKAGE_BYTES: usize = 4 * 1024;
pub const MAX_BOOTSTRAP_ENVELOPE_BYTES: usize = 8 * 1024;
pub const MAX_BOOTSTRAP_CODE_CHARS: usize = 12 * 1024;
pub const BOOTSTRAP_NONCE_BYTES: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapEnvelope {
    version: u16,
    invite_package: Vec<u8>,
    peer_id: String,
    address: String,
}

impl Drop for BootstrapEnvelope {
    fn drop(&mut self) {
        self.invite_package.fill(0);
    }
}

pub struct ParsedBootstrapEnvelope(BootstrapEnvelope);

pub struct ValidatedBootstrapEnvelope {
    invite_package: Vec<u8>,
    peer_id: PeerId,
    address: Multiaddr,
}

impl Drop for ValidatedBootstrapEnvelope {
    fn drop(&mut self) {
        self.invite_package.fill(0);
    }
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("bootstrap code exceeds {MAX_BOOTSTRAP_CODE_CHARS} characters")]
    CodeTooLarge,
    #[error("bootstrap code has an unsupported prefix")]
    InvalidPrefix,
    #[error("bootstrap code is not valid base64url")]
    InvalidBase64,
    #[error("bootstrap envelope exceeds {MAX_BOOTSTRAP_ENVELOPE_BYTES} bytes")]
    EnvelopeTooLarge,
    #[error("bootstrap envelope is malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("bootstrap envelope version is unsupported")]
    UnsupportedVersion,
    #[error("invite package is empty or exceeds its size limit")]
    InvalidInvitePackage,
    #[error("bootstrap Peer ID is invalid")]
    InvalidPeerId,
    #[error("bootstrap multiaddress is invalid")]
    InvalidAddress,
}

pub fn create_code(
    invite_package: Vec<u8>,
    peer_id: PeerId,
    address: &Multiaddr,
) -> Result<String, BootstrapError> {
    if invite_package.is_empty() || invite_package.len() > MAX_INVITE_PACKAGE_BYTES {
        return Err(BootstrapError::InvalidInvitePackage);
    }
    validate_address(address)?;
    let envelope = BootstrapEnvelope {
        version: BOOTSTRAP_VERSION,
        invite_package,
        peer_id: peer_id.to_string(),
        address: address.to_string(),
    };
    let encoded = serde_json::to_vec(&envelope)?;
    if encoded.len() > MAX_BOOTSTRAP_ENVELOPE_BYTES {
        return Err(BootstrapError::EnvelopeTooLarge);
    }
    let code = format!("{CODE_PREFIX}{}", URL_SAFE_NO_PAD.encode(encoded));
    if code.len() > MAX_BOOTSTRAP_CODE_CHARS {
        return Err(BootstrapError::CodeTooLarge);
    }
    Ok(code)
}

pub fn parse_code(code: &str) -> Result<ParsedBootstrapEnvelope, BootstrapError> {
    if code.len() > MAX_BOOTSTRAP_CODE_CHARS {
        return Err(BootstrapError::CodeTooLarge);
    }
    let payload = code
        .strip_prefix(CODE_PREFIX)
        .ok_or(BootstrapError::InvalidPrefix)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| BootstrapError::InvalidBase64)?;
    if bytes.len() > MAX_BOOTSTRAP_ENVELOPE_BYTES {
        return Err(BootstrapError::EnvelopeTooLarge);
    }
    Ok(ParsedBootstrapEnvelope(serde_json::from_slice(&bytes)?))
}

impl ParsedBootstrapEnvelope {
    pub fn validate(mut self) -> Result<ValidatedBootstrapEnvelope, BootstrapError> {
        if self.0.version != BOOTSTRAP_VERSION {
            return Err(BootstrapError::UnsupportedVersion);
        }
        if self.0.invite_package.is_empty()
            || self.0.invite_package.len() > MAX_INVITE_PACKAGE_BYTES
        {
            return Err(BootstrapError::InvalidInvitePackage);
        }
        let peer_id =
            PeerId::from_str(&self.0.peer_id).map_err(|_| BootstrapError::InvalidPeerId)?;
        let address =
            Multiaddr::from_str(&self.0.address).map_err(|_| BootstrapError::InvalidAddress)?;
        validate_address(&address)?;
        Ok(ValidatedBootstrapEnvelope {
            invite_package: std::mem::take(&mut self.0.invite_package),
            peer_id,
            address,
        })
    }
}

fn validate_address(address: &Multiaddr) -> Result<(), BootstrapError> {
    if address.iter().next().is_none()
        || address
            .iter()
            .any(|protocol| matches!(protocol, Protocol::P2p(_)))
    {
        return Err(BootstrapError::InvalidAddress);
    }
    Ok(())
}

impl ValidatedBootstrapEnvelope {
    pub fn invite_package(&self) -> &[u8] {
        &self.invite_package
    }

    pub fn peer_id(&self) -> PeerId {
        self.peer_id
    }

    pub fn address(&self) -> &Multiaddr {
        &self.address
    }
}

pub(crate) fn possession_proof_bytes(
    community: CommunityId,
    invitation: EventId,
    invitee: UserId,
    device_id: &[u8; 32],
    client_peer: PeerId,
    server_peer: PeerId,
    nonce: &[u8; BOOTSTRAP_NONCE_BYTES],
) -> Vec<u8> {
    let client_peer = client_peer.to_bytes();
    let server_peer = server_peer.to_bytes();
    let mut bytes = Vec::with_capacity(256);
    bytes.extend_from_slice(b"chatcommons:bootstrap-possession:v1\0");
    bytes.extend_from_slice(community.as_bytes());
    bytes.extend_from_slice(invitation.as_bytes());
    bytes.extend_from_slice(invitee.as_bytes());
    bytes.extend_from_slice(device_id);
    bytes.extend_from_slice(blake3::hash(&client_peer).as_bytes());
    bytes.extend_from_slice(blake3::hash(&server_peer).as_bytes());
    bytes.extend_from_slice(nonce);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[test]
    fn envelope_round_trips_without_padding() -> Result<(), Box<dyn std::error::Error>> {
        let peer = identity::Keypair::generate_ed25519().public().to_peer_id();
        let address: Multiaddr = "/ip4/127.0.0.1/udp/4001/quic-v1".parse()?;
        let code = create_code(vec![1, 2, 3], peer, &address)?;
        assert!(code.starts_with(CODE_PREFIX));
        assert!(!code.contains('='));
        let envelope = parse_code(&code)?.validate()?;
        assert_eq!(envelope.invite_package(), &[1, 2, 3]);
        assert_eq!(envelope.peer_id(), peer);
        assert_eq!(envelope.address(), &address);
        Ok(())
    }

    #[test]
    fn oversized_code_is_rejected_before_decoding() {
        let code = format!("{CODE_PREFIX}{}", "a".repeat(MAX_BOOTSTRAP_CODE_CHARS));
        assert!(matches!(
            parse_code(&code),
            Err(BootstrapError::CodeTooLarge)
        ));
    }
}
