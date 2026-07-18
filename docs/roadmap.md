# Roadmap

## M1 — offline protocol core v2 (implemented)

- minimal opaque signed event envelope and stable IDs
- genesis-derived community identity
- parsing separated from cryptographic validation
- idempotent SQLite persistence and recovery
- unordered parent ingestion and concurrent DAG branches
- one optional `chatcommons.chat.v1` reference profile

## M1.1 — single-use invitation capability (implemented)

- community administrators create invitations; there is no active join request
- the public event contains only an ephemeral capability public key
- a bounded private invite package can create an identity-bound acceptance
- malformed, mismatched and repeated redemptions are rejected deterministically
- contacts, short-link services, expiry and cancellation remain deferred

See [`ADR 0012`](adr/0012-single-use-bearer-invitations.md).

## G1 — security and governance baseline (documented)

- threat model and small-group abuse-reducing defaults
- protocol/community/node/official-service control boundaries
- report-bundle privacy and evidence requirements
- China launch legal review checklist and official source baseline
- pre-network, operated-service and public-launch engineering gates

Exit condition for public infrastructure is not yet met. It requires qualified
legal review, service-specific data flows, policies, an incident runbook and
implemented resource limits.

## Deferred from M1

CRDT state resolution, MLS, libp2p, relays, attachments, voice, GUI, discovery,
multi-device identity and large-community scaling were not part of M1. Later
milestones may implement them independently; libp2p transport and a minimal relay
fallback now exist in M2.

Small desktop screen sharing has an accepted but deferred peer-assisted direction:
an adaptive n-ary distribution tree with bounded voluntary upload and replaceable
SFU fallback. See
[`ADR 0008`](adr/0008-adaptive-peer-assisted-screen-sharing.md).

The product model distinguishes durable communities from temporary live sessions.
Both will reuse identity, invitations and transport, but temporary voice/session
state must not enter durable community history by default. See
[`ADR 0013`](adr/0013-durable-communities-and-temporary-sessions.md).

## M2a — transport-independent event synchronization (implemented)

- bounded `Hello`, `Heads`, `Want`, and `Events` messages
- recursive missing-parent retrieval
- bidirectional convergence of independent DAG branches
- bounded in-memory pending events
- no transport or remote identity assumption

See [`ADR 0009`](adr/0009-transport-independent-event-sync.md).

## M2b — authenticated direct QUIC synchronization (implemented)

The user/device key split, deterministic device certificates, Peer ID binding,
and signed revocations are implemented as authentication foundations. See
[`ADR 0010`](adr/0010-user-and-device-key-separation.md).

Two real rust-libp2p Swarms can carry M2a over QUIC, mutually verify device
certificates against transport Peer IDs and an allowed-user set, and converge
independent SQLite databases. See
[`ADR 0011`](adr/0011-minimal-direct-quic-sync.md).

## M2c — diagnostic direct node (implemented)

- Unix-only local user/device key persistence with private filesystem modes
- `init`, `info`, `create-community`, and `run` commands
- explicit listen address, Peer ID, dial address, and bounded user allowlist
- observable connection, mutual authentication and synchronization progress
- a two-process integration test over real loopback QUIC and independent SQLite

Physical two-machine measurement remains pending. The executable is deliberately
not an end-user client and must not be exposed as a public service. See
[`ADR 0014`](adr/0014-m2c-diagnostic-node.md).

## M2d — secure invitation bootstrap (implemented locally)

- one bounded `cc1_` code wraps the existing private invite package and one
  explicit Peer ID/direct QUIC address
- a device-bound random challenge proves capability possession before ancestry
  is disclosed
- pre-membership access is limited to invitation ancestry and one acceptance
  candidate
- both sides apply the chat profile before promoting the connection to ordinary
  synchronization
- forged capabilities and repeated redemption are rejected in a real two-process
  QUIC test

Public two-machine measurement remains pending. See
[`ADR 0015`](adr/0015-secure-invitation-bootstrap.md).

## M2e — relay-assisted NAT traversal (implemented locally)

- direct QUIC remains the preferred path
- a validated invite may contain one Relay v2 circuit route
- Identify supplies observed addresses and DCUtR attempts a direct upgrade
- the relay circuit remains usable when direct candidates fail
- relay peers are infrastructure peers and never receive community authentication
- the diagnostic relay enforces circuit, byte, duration and rate bounds
- real in-process and three-process tests cover fallback and direct upgrade

Physical cross-NAT measurement remains pending because no public relay has been
approved for this project. The included relay has an ephemeral identity and is
not a public-service binary. See
[`ADR 0016`](adr/0016-relay-assisted-hole-punching.md).

## Suggested M2f

Measure the existing M2e path between two physical networks using a separately
approved relay, record direct-upgrade success and fallback behavior, and remove
all temporary endpoint state afterward. Do not add discovery, voice, files or a
public relay operation until their respective engineering gates are met.
