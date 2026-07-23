use chatcommons_crypto::Identity;
use chatcommons_sync::auth::{AuthError, DeviceIdentity};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub const IDENTITY_FILE: &str = "identity.json";
pub const LOCK_FILE: &str = "state.lock";
const STATE_VERSION: u16 = 1;
const MAX_IDENTITY_BYTES: u64 = 4 * 1024;

#[derive(Debug, Error)]
pub enum StateError {
    #[error("state persistence is not supported on this platform")]
    UnsupportedPermissions,
    #[error("state path must not be a symbolic link")]
    SymbolicLink,
    #[error("identity state already exists")]
    AlreadyInitialized,
    #[error("identity state does not exist")]
    NotInitialized,
    #[error("state directory is already in use by another process")]
    AlreadyInUse,
    #[error("state permissions allow access by another user")]
    InsecurePermissions,
    #[error("identity state exceeds its size limit")]
    TooLarge,
    #[error("identity state has an unsupported version")]
    UnsupportedVersion,
    #[error("system clock is before the Unix epoch")]
    InvalidSystemTime,
    #[error("state I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("identity state is malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("device identity is invalid: {0}")]
    Auth(#[from] AuthError),
}

pub struct NodeState {
    user: Identity,
    device: DeviceIdentity,
    created_at_ms: i64,
    directory: PathBuf,
}

pub struct StateLock {
    _file: File,
}

impl NodeState {
    pub fn initialize(directory: impl AsRef<Path>) -> Result<Self, StateError> {
        require_supported_permissions()?;
        let directory = directory.as_ref();
        prepare_directory(directory)?;
        let identity_path = directory.join(IDENTITY_FILE);
        if identity_path.exists() {
            return Err(StateError::AlreadyInitialized);
        }

        let mut user_seed = [0_u8; 32];
        let mut device_seed = [0_u8; 32];
        OsRng.fill_bytes(&mut user_seed);
        OsRng.fill_bytes(&mut device_seed);
        let created_at_ms = now_ms()?;
        let mut persisted = PersistedState {
            version: STATE_VERSION,
            user_seed,
            device_seed,
            created_at_ms,
        };
        let encoded_result = serde_json::to_vec(&persisted);
        persisted.user_seed.fill(0);
        persisted.device_seed.fill(0);
        let mut encoded = encoded_result?;
        if encoded.len() as u64 > MAX_IDENTITY_BYTES {
            encoded.fill(0);
            user_seed.fill(0);
            device_seed.fill(0);
            return Err(StateError::TooLarge);
        }

        let write_result = write_private_file(&identity_path, &encoded);
        encoded.fill(0);
        if let Err(error) = write_result {
            user_seed.fill(0);
            device_seed.fill(0);
            return Err(error);
        }
        let state = Self::from_seeds(directory, user_seed, device_seed, created_at_ms);
        user_seed.fill(0);
        device_seed.fill(0);
        state
    }

    pub fn load(directory: impl AsRef<Path>) -> Result<Self, StateError> {
        require_supported_permissions()?;
        let directory = directory.as_ref();
        validate_directory(directory)?;
        let identity_path = directory.join(IDENTITY_FILE);
        let metadata =
            fs::symlink_metadata(&identity_path).map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => StateError::NotInitialized,
                _ => StateError::Io(error),
            })?;
        if metadata.file_type().is_symlink() {
            return Err(StateError::SymbolicLink);
        }
        validate_private_mode(&metadata)?;
        let mut bytes = Vec::new();
        File::open(&identity_path)?
            .take(MAX_IDENTITY_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_IDENTITY_BYTES {
            bytes.fill(0);
            return Err(StateError::TooLarge);
        }
        let persisted_result = serde_json::from_slice(&bytes);
        bytes.fill(0);
        let mut persisted: PersistedState = persisted_result?;
        if persisted.version != STATE_VERSION {
            persisted.user_seed.fill(0);
            persisted.device_seed.fill(0);
            return Err(StateError::UnsupportedVersion);
        }
        let user_seed = persisted.user_seed;
        let device_seed = persisted.device_seed;
        persisted.user_seed.fill(0);
        persisted.device_seed.fill(0);
        Self::from_seeds(directory, user_seed, device_seed, persisted.created_at_ms)
    }

    pub fn user(&self) -> &Identity {
        &self.user
    }

    pub fn device(&self) -> &DeviceIdentity {
        &self.device
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }

    pub fn database_path(&self) -> PathBuf {
        self.directory.join("events.sqlite3")
    }

    pub fn acquire_lock(&self) -> Result<StateLock, StateError> {
        let lock_path = self.directory.join(LOCK_FILE);
        if fs::symlink_metadata(&lock_path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(StateError::SymbolicLink);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(lock_path)?;
        validate_private_mode(&file.metadata()?)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(StateError::AlreadyInUse),
            Err(TryLockError::Error(error)) => return Err(StateError::Io(error)),
        }
        Ok(StateLock { _file: file })
    }

    fn from_seeds(
        directory: &Path,
        user_seed: [u8; 32],
        device_seed: [u8; 32],
        created_at_ms: i64,
    ) -> Result<Self, StateError> {
        Ok(Self {
            user: Identity::from_seed(user_seed),
            device: DeviceIdentity::from_seed(device_seed)?,
            created_at_ms,
            directory: directory.to_path_buf(),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistedState {
    version: u16,
    user_seed: [u8; 32],
    device_seed: [u8; 32],
    created_at_ms: i64,
}

fn now_ms() -> Result<i64, StateError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StateError::InvalidSystemTime)?;
    i64::try_from(duration.as_millis()).map_err(|_| StateError::InvalidSystemTime)
}

#[cfg(unix)]
fn require_supported_permissions() -> Result<(), StateError> {
    Ok(())
}

#[cfg(not(unix))]
fn require_supported_permissions() -> Result<(), StateError> {
    Ok(())
}

#[cfg(unix)]
fn prepare_directory(directory: &Path) -> Result<(), StateError> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(StateError::SymbolicLink);
            }
            if !metadata.is_dir() {
                return Err(StateError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotADirectory,
                    "state path is not a directory",
                )));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(directory)?;
        }
        Err(error) => return Err(StateError::Io(error)),
    }
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn prepare_directory(directory: &Path) -> Result<(), StateError> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(StateError::SymbolicLink),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(directory)?;
            Ok(())
        }
        Err(error) => Err(StateError::Io(error)),
    }
}

fn validate_directory(directory: &Path) -> Result<(), StateError> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => StateError::NotInitialized,
        _ => StateError::Io(error),
    })?;
    if metadata.file_type().is_symlink() {
        return Err(StateError::SymbolicLink);
    }
    if !metadata.is_dir() {
        return Err(StateError::NotInitialized);
    }
    validate_private_mode(&metadata)
}

#[cfg(unix)]
fn validate_private_mode(metadata: &fs::Metadata) -> Result<(), StateError> {
    if metadata.permissions().mode() & 0o077 == 0 {
        Ok(())
    } else {
        Err(StateError::InsecurePermissions)
    }
}

#[cfg(not(unix))]
fn validate_private_mode(_metadata: &fs::Metadata) -> Result<(), StateError> {
    // Windows application-data directories inherit the current user's ACL.
    // The friends alpha does not claim protection from local administrators.
    Ok(())
}

#[cfg(unix)]
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), StateError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => StateError::AlreadyInitialized,
            _ => StateError::Io(error),
        })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), StateError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => StateError::AlreadyInitialized,
            _ => StateError::Io(error),
        })?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
