use chatcommons_crypto::Identity;
use chatcommons_sync::auth::{
    AuthError, DeviceIdentity, RevocationSet, create_device_certificate, create_device_revocation,
    peer_id_from_certificate, validate_device_certificate,
};

#[test]
fn certificate_binds_user_device_and_libp2p_peer_id() -> Result<(), AuthError> {
    let user = Identity::from_seed([81; 32]);
    let device = DeviceIdentity::from_seed([82; 32])?;
    let certificate = create_device_certificate(&user, &device, 1_700_000_000_000);
    let authenticated = validate_device_certificate(&certificate)?;

    assert_eq!(authenticated.user_id, user.user_id());
    assert_eq!(authenticated.device_id, device.device_id());
    assert_eq!(peer_id_from_certificate(&certificate)?, device.peer_id());
    Ok(())
}

#[test]
fn rejects_tampered_certificate() -> Result<(), AuthError> {
    let user = Identity::from_seed([83; 32]);
    let device = DeviceIdentity::from_seed([84; 32])?;
    let mut certificate = create_device_certificate(&user, &device, 1);
    certificate.device_public_key[0] ^= 1;

    assert_eq!(
        validate_device_certificate(&certificate),
        Err(AuthError::InvalidSignature)
    );
    Ok(())
}

#[test]
fn valid_revocation_is_scoped_to_user_and_device() -> Result<(), AuthError> {
    let user = Identity::from_seed([86; 32]);
    let device = DeviceIdentity::from_seed([87; 32])?;
    let revocation = create_device_revocation(&user, device.device_id(), 2);
    let mut revocations = RevocationSet::default();
    revocations.apply(&revocation)?;
    assert!(revocations.contains(user.user_id(), device.device_id()));

    let mut tampered = create_device_revocation(&user, device.device_id(), 3);
    tampered.device_id = DeviceIdentity::from_seed([88; 32])?.device_id();
    assert_eq!(
        revocations.apply(&tampered),
        Err(AuthError::InvalidSignature)
    );
    Ok(())
}
