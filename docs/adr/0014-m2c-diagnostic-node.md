# ADR 0014: M2c diagnostic direct node

Status: accepted and implemented locally; physical two-machine measurement pending

## Context

M2b tested two QUIC swarms in one process. The next useful proof is a real
executable with persistent Peer IDs and observable connection state. Combining
this with ordinary invitation bootstrap would create a circular dependency:
before synchronization the new user is not yet in the resolved member allowlist,
but without narrow bootstrap access they cannot fetch invitation ancestry and
publish their acceptance.

Production key storage is also a separate design. The high-security identity mode,
OS keychains, Windows ACLs, encrypted key files and recovery UX are intentionally
deferred.

## Decision

M2c provides a developer-only `chatcommons-node` executable:

- `init` creates persistent user and device Ed25519 keys;
- `info` prints only UserId, PeerId and creation metadata;
- `create-community` stores one signed genesis in the local SQLite database;
- `run` accepts an explicit Community ID, listen multiaddress and bounded remote
  UserId allowlist;
- an optional explicit PeerId/multiaddress pair starts a direct QUIC dial;
- connection, authentication and synchronization progress are printed as
  machine-readable lines;
- an optional event-count exit condition exists for deterministic tests.

On Unix, the state directory is forced to mode `0700` and the identity file is
created with mode `0600`. Symlink state paths, oversized/malformed state and
group/world-accessible state are rejected. The state format is bounded and
versioned. Plaintext state initialization is refused on platforms without Unix
permission enforcement; Windows support waits for an explicit ACL or key-store
decision.

Peers exchange UserId, PeerId and address out of band and manually allow each
other. This is diagnostic configuration, not the `chatcommons.chat.v1` invitation
experience and not a new admission rule.

## Consequences

Two independent executable processes can retain stable identities, mutually
authenticate over real QUIC and synchronize an empty SQLite database from a peer
that holds a community genesis. A real LAN/WAN measurement can now be performed
without adding discovery or relay infrastructure.

The user root key is online and stored as plaintext protected by filesystem
permissions. This is acceptable only for M2c development identities. There is no
state-directory process lock, graceful shutdown protocol, discovery, NAT traversal,
relay, rate limiting, revocation persistence or secure invitation bootstrap.
Only one process may use a state directory, and listeners must not be exposed to
untrusted networks.
