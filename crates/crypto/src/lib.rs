use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

pub const PUBLIC_KEY_LEN: usize = 32;
pub const SIGNATURE_LEN: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct UserId([u8; 32]);

impl UserId {
    pub fn from_public_key(public_key: &[u8; PUBLIC_KEY_LEN]) -> Self {
        Self(*blake3::hash(public_key).as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for UserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(self.0))
    }
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("public key must be 32 bytes")]
    InvalidPublicKeyLength,
    #[error("signature must be 64 bytes")]
    InvalidSignatureLength,
    #[error("invalid public key")]
    InvalidPublicKey,
    #[error("signature verification failed")]
    InvalidSignature,
}

pub struct Identity(SigningKey);

impl Identity {
    pub fn generate() -> Self {
        Self(SigningKey::generate(&mut OsRng))
    }

    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self(SigningKey::from_bytes(&seed))
    }

    pub fn public_key(&self) -> [u8; PUBLIC_KEY_LEN] {
        self.0.verifying_key().to_bytes()
    }

    pub fn user_id(&self) -> UserId {
        UserId::from_public_key(&self.public_key())
    }

    pub fn sign(&self, bytes: &[u8]) -> [u8; SIGNATURE_LEN] {
        self.0.sign(bytes).to_bytes()
    }
}

pub fn verify(public_key: &[u8], bytes: &[u8], signature: &[u8]) -> Result<(), CryptoError> {
    let public_key: [u8; PUBLIC_KEY_LEN] = public_key
        .try_into()
        .map_err(|_| CryptoError::InvalidPublicKeyLength)?;
    let signature: [u8; SIGNATURE_LEN] = signature
        .try_into()
        .map_err(|_| CryptoError::InvalidSignatureLength)?;
    let verifying_key =
        VerifyingKey::from_bytes(&public_key).map_err(|_| CryptoError::InvalidPublicKey)?;
    verifying_key
        .verify(bytes, &Signature::from_bytes(&signature))
        .map_err(|_| CryptoError::InvalidSignature)
}
