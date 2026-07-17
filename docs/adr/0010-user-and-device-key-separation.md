# ADR 0010: User and device key separation

Status: accepted for M2b authentication foundations

## Context

A long-lived user identity and a network endpoint have different lifecycles. Using
one key for both would make transport rotation a user identity change and would
make it impossible for cooperating nodes to reject one lost device independently.

M2a deliberately left remote identity undefined. Before carrying synchronization
over a real connection, peers need a small proof that a fresh connection comes
from a device authorized by a user identity.

## Decision

User and device Ed25519 keys are separate:

- the user key remains the author of current Core community events;
- every installation generates an independent device key for network handshakes;
- the user signs a deterministic `DeviceCertificate` binding the device public key
  to the user public key;
- the same device key derives both the ChatCommons `DeviceId` and libp2p `PeerId`;
- libp2p's authenticated transport proves possession of the device private key;
- ChatCommons verifies that the certificate public key derives the connected Peer ID;
- a user-signed `DeviceRevocation` permanently rejects that user/device pair once
  the verifier has learned it.

Issue and revocation timestamps are signed metadata only. They do not determine
ordering, expiry, or conflict resolution. ChatCommons does not add a second custom
challenge signature on top of an already authenticated libp2p connection.

This layer does not put certificates or revocations in the Core event DAG, does not
define a global identity log, and does not yet decide their persistence or
distribution. Those choices require the real connection and multi-device recovery
design.

## Consequences

Transport connections can use device identities without changing community
identity, and a verifier that knows a valid revocation can reject a lost device.
Changing transport does not require introducing another application identity, but
the chosen transport must authenticate the same Peer ID.

This is not yet protection against compromise of the user private key. Current
clients still need that key to author community events; an attacker holding it can
sign events and issue another device certificate. Offline root keys, device-signed
community events, certificate expiry, social recovery, revocation distribution,
and hardware-backed storage are explicitly deferred to the later high-security
identity design.
