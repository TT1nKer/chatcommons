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

## Explicitly deferred

CRDT state resolution, MLS, libp2p, relays, attachments, voice, GUI, discovery,
multi-device identity and large-community scaling are not part of M1.

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

## Suggested M2c

Add a minimal executable that persists device keys and dials an explicitly supplied
IP/port/Peer ID. Then measure direct connectivity across two real machines.
Persisting and distributing revocations must be decided before authentication is
called production-ready. Add address discovery and a replaceable relay only after
direct dialing is observable; do not add voice, files, or generic policy machinery.
