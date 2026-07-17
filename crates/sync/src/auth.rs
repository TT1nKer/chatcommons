use chatcommons_crypto::{Identity, PUBLIC_KEY_LEN, SIGNATURE_LEN, UserId, verify};
use libp2p::{PeerId, identity};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt};
use thiserror::Error;

pub const DEVICE_AUTH_VERSION: u16 = 1;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DeviceId([u8; 32]);

impl DeviceId {
    pub fn from_public_key(public_key: &[u8; PUBLIC_KEY_LEN]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"chatcommons:device-id:v1\0");
        hasher.update(public_key);
        Self(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

pub struct DeviceIdentity {
    keypair: identity::Keypair,
    public_key: [u8; PUBLIC_KEY_LEN],
}

impl DeviceIdentity {
    pub fn generate() -> Result<Self, AuthError> {
        Self::from_keypair(identity::Keypair::generate_ed25519())
    }

    pub fn from_seed(mut seed: [u8; 32]) -> Result<Self, AuthError> {
        let keypair = identity::Keypair::ed25519_from_bytes(&mut seed)
            .map_err(|_| AuthError::InvalidDeviceKey)?;
        Self::from_keypair(keypair)
    }

    fn from_keypair(keypair: identity::Keypair) -> Result<Self, AuthError> {
        let public_key = keypair
            .public()
            .try_into_ed25519()
            .map_err(|_| AuthError::InvalidDeviceKey)?
            .to_bytes();
        Ok(Self {
            keypair,
            public_key,
        })
    }

    pub fn public_key(&self) -> [u8; PUBLIC_KEY_LEN] {
        self.public_key
    }

    pub fn device_id(&self) -> DeviceId {
        DeviceId::from_public_key(&self.public_key)
    }

    pub fn peer_id(&self) -> PeerId {
        self.keypair.public().to_peer_id()
    }

    pub fn keypair(&self) -> identity::Keypair {
        self.keypair.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCertificate {
    pub version: u16,
    pub user_public_key: Vec<u8>,
    pub device_public_key: Vec<u8>,
    pub issued_at_ms: i64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRevocation {
    pub version: u16,
    pub user_public_key: Vec<u8>,
    pub device_id: DeviceId,
    pub issued_at_ms: i64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedDevice {
    pub user_id: UserId,
    pub device_id: DeviceId,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("unsupported device authentication version")]
    UnsupportedVersion,
    #[error("device key is invalid")]
    InvalidDeviceKey,
    #[error("public key length is invalid")]
    InvalidPublicKeyLength,
    #[error("signature length is invalid")]
    InvalidSignatureLength,
    #[error("signature verification failed")]
    InvalidSignature,
    #[error("device has been revoked")]
    Revoked,
}

pub fn create_device_certificate(
    user: &Identity,
    device: &DeviceIdentity,
    issued_at_ms: i64,
) -> DeviceCertificate {
    let mut certificate = DeviceCertificate {
        version: DEVICE_AUTH_VERSION,
        user_public_key: user.public_key().to_vec(),
        device_public_key: device.public_key().to_vec(),
        issued_at_ms,
        signature: Vec::new(),
    };
    certificate.signature = user.sign(&certificate_bytes(&certificate)).to_vec();
    certificate
}

pub fn validate_device_certificate(
    certificate: &DeviceCertificate,
) -> Result<AuthenticatedDevice, AuthError> {
    validate_version_and_lengths(
        certificate.version,
        &certificate.user_public_key,
        Some(&certificate.device_public_key),
        &certificate.signature,
    )?;
    verify(
        &certificate.user_public_key,
        &certificate_bytes(certificate),
        &certificate.signature,
    )
    .map_err(|_| AuthError::InvalidSignature)?;
    let user_public_key: [u8; PUBLIC_KEY_LEN] = certificate
        .user_public_key
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::InvalidPublicKeyLength)?;
    let device_public_key: [u8; PUBLIC_KEY_LEN] = certificate
        .device_public_key
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::InvalidPublicKeyLength)?;
    Ok(AuthenticatedDevice {
        user_id: UserId::from_public_key(&user_public_key),
        device_id: DeviceId::from_public_key(&device_public_key),
    })
}

pub fn peer_id_from_certificate(certificate: &DeviceCertificate) -> Result<PeerId, AuthError> {
    validate_device_certificate(certificate)?;
    let public_key = identity::ed25519::PublicKey::try_from_bytes(&certificate.device_public_key)
        .map_err(|_| AuthError::InvalidDeviceKey)?;
    Ok(identity::PublicKey::from(public_key).to_peer_id())
}

pub fn create_device_revocation(
    user: &Identity,
    device_id: DeviceId,
    issued_at_ms: i64,
) -> DeviceRevocation {
    let mut revocation = DeviceRevocation {
        version: DEVICE_AUTH_VERSION,
        user_public_key: user.public_key().to_vec(),
        device_id,
        issued_at_ms,
        signature: Vec::new(),
    };
    revocation.signature = user.sign(&revocation_bytes(&revocation)).to_vec();
    revocation
}

pub fn validate_device_revocation(revocation: &DeviceRevocation) -> Result<UserId, AuthError> {
    validate_version_and_lengths(
        revocation.version,
        &revocation.user_public_key,
        None,
        &revocation.signature,
    )?;
    verify(
        &revocation.user_public_key,
        &revocation_bytes(revocation),
        &revocation.signature,
    )
    .map_err(|_| AuthError::InvalidSignature)?;
    let public_key: [u8; PUBLIC_KEY_LEN] = revocation
        .user_public_key
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::InvalidPublicKeyLength)?;
    Ok(UserId::from_public_key(&public_key))
}

#[derive(Default)]
pub struct RevocationSet {
    entries: BTreeSet<(UserId, DeviceId)>,
}

impl RevocationSet {
    pub fn apply(&mut self, revocation: &DeviceRevocation) -> Result<(), AuthError> {
        let user_id = validate_device_revocation(revocation)?;
        self.entries.insert((user_id, revocation.device_id));
        Ok(())
    }

    pub fn contains(&self, user_id: UserId, device_id: DeviceId) -> bool {
        self.entries.contains(&(user_id, device_id))
    }
}

fn validate_version_and_lengths(
    version: u16,
    user_public_key: &[u8],
    device_public_key: Option<&[u8]>,
    signature: &[u8],
) -> Result<(), AuthError> {
    if version != DEVICE_AUTH_VERSION {
        return Err(AuthError::UnsupportedVersion);
    }
    if user_public_key.len() != PUBLIC_KEY_LEN
        || device_public_key.is_some_and(|key| key.len() != PUBLIC_KEY_LEN)
    {
        return Err(AuthError::InvalidPublicKeyLength);
    }
    if signature.len() != SIGNATURE_LEN {
        return Err(AuthError::InvalidSignatureLength);
    }
    Ok(())
}

fn certificate_bytes(certificate: &DeviceCertificate) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(82);
    bytes.extend_from_slice(b"chatcommons:device-certificate:v1\0");
    bytes.extend_from_slice(&certificate.version.to_be_bytes());
    bytes.extend_from_slice(&certificate.user_public_key);
    bytes.extend_from_slice(&certificate.device_public_key);
    bytes.extend_from_slice(&certificate.issued_at_ms.to_be_bytes());
    bytes
}

fn revocation_bytes(revocation: &DeviceRevocation) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(74);
    bytes.extend_from_slice(b"chatcommons:device-revocation:v1\0");
    bytes.extend_from_slice(&revocation.version.to_be_bytes());
    bytes.extend_from_slice(&revocation.user_public_key);
    bytes.extend_from_slice(revocation.device_id.as_bytes());
    bytes.extend_from_slice(&revocation.issued_at_ms.to_be_bytes());
    bytes
}
