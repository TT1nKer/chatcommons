use chatcommons_cli::{IDENTITY_FILE, NodeState, StateError};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
#[cfg(unix)]
fn identity_state_is_private_and_stable() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("node");
    let initial = NodeState::initialize(&directory)?;
    let user = initial.user().user_id();
    let peer = initial.device().peer_id();
    drop(initial);

    let directory_mode = std::fs::metadata(&directory)?.permissions().mode() & 0o777;
    let file_mode = std::fs::metadata(directory.join(IDENTITY_FILE))?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(directory_mode, 0o700);
    assert_eq!(file_mode, 0o600);

    let reopened = NodeState::load(&directory)?;
    assert_eq!(reopened.user().user_id(), user);
    assert_eq!(reopened.device().peer_id(), peer);
    Ok(())
}

#[test]
#[cfg(unix)]
fn insecure_identity_permissions_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("node");
    NodeState::initialize(&directory)?;
    let identity = directory.join(IDENTITY_FILE);
    std::fs::set_permissions(&identity, std::fs::Permissions::from_mode(0o644))?;

    assert!(matches!(
        NodeState::load(&directory),
        Err(StateError::InsecurePermissions)
    ));
    Ok(())
}
