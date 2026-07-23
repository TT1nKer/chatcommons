# Roadmap

## M1 — offline protocol core v2 (implemented)

- minimal opaque signed event envelope and stable IDs
- genesis-derived community identity
- parsing separated from cryptographic validation
- idempotent SQLite persistence and recovery
- unordered parent ingestion and concurrent DAG branches
- one optional reference chat profile, advanced to `chatcommons.chat.v2` in M3a

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

Direct-address physical measurement remains pending; the secure invitation flow
was exercised during the M2e cross-NAT relay measurement. See
[`ADR 0015`](adr/0015-secure-invitation-bootstrap.md).

## M2e — relay-assisted NAT traversal (implemented locally)

- direct QUIC remains the preferred path
- a validated invite may contain one Relay v2 circuit route
- Identify supplies observed addresses and DCUtR attempts a direct upgrade
- the relay circuit remains usable when direct candidates fail
- relay peers are infrastructure peers and never receive community authentication
- the diagnostic relay enforces circuit, byte, duration and rate bounds
- real in-process and three-process tests cover fallback and direct upgrade

One physical cross-NAT measurement is complete: macOS on a phone hotspot and
Windows/WSL on a separate home connection completed bootstrap and SQLite
convergence over an independently operated IPFS Relay v2 peer. The direct
candidate timed out and the relay remained the working fallback. The included
relay still has an ephemeral identity and is not a public-service binary. See
[`ADR 0016`](adr/0016-relay-assisted-hole-punching.md).

## M3a — replaceable community home-server declaration (implemented)

- one logical default home-server binding per community
- owner-only signed replacement without old-server or official approval
- server identity derived independently from the genesis-derived Community ID
- bounded endpoint hints and DAG-parent history checkpoints
- deterministic concurrent migration resolution without timestamp precedence
- no production server, discovery service, backup, attachment or migration tool

See [`ADR 0017`](adr/0017-replaceable-community-home-server.md).

## M3b — minimal authenticated Community Home Server (implemented locally)

- diagnostic `set-home-server` and `serve-community` commands
- exact binding between the signed server key, device certificate and Peer ID
- server operator remains outside community membership and governance
- SQLite persistence and later delivery between members that are never online together
- live membership and server-binding authorization refresh after synchronization
- rejection of undeclared server devices and fail-closed behavior after migration
- no production operations, discovery, export/import, quotas, attachments or backup automation

See [`ADR 0018`](adr/0018-minimal-community-home-server.md).

## M3c — portable provisioning and declared dialing (implemented locally)

- bounded deterministic Community Archive v1 with parent-closure validation
- idempotent export/import without identity or device secrets
- topologically batched SQLite import under existing ingestion limits
- automatic Peer ID and Multiaddr selection from signed Home Server state
- real export/import/serve/offline-member integration coverage
- no encrypted, incremental or attachment backup; no scheduler or object store

See
[`ADR 0019`](adr/0019-bounded-community-archives-and-declared-dialing.md).

## M3d — private Home Server runtime boundaries (implemented locally)

- one private advisory process lock per node state
- bounded logical event-body storage, including unresolved sync events
- configurable 512 MiB default Home Server quota
- least-privilege systemd template with per-instance state directories
- loopback-first deployment guidance that leaves firewall and cloud rules unchanged
- no claim of public-service readiness or filesystem-level quota enforcement

See
[`ADR 0020`](adr/0020-private-home-server-runtime-boundaries.md).

## M3e — private Home Server snapshot and recovery (implemented locally)

- root-only, bounded snapshot directories with atomic completion
- consistent stopped-service event export plus server identity and runtime config
- restart-on-success-or-failure behavior for instances stopped by backup
- checksum, identity and full archive validation during restore
- fail-closed restore into a new state without overwriting an existing instance
- no provider lock-in, upload, encryption, retention schedule or automatic deletion

See
[`ADR 0021`](adr/0021-private-server-snapshots.md).

## Suggested M2f

Repeat M2e across a small NAT/firewall matrix, record direct-upgrade latency,
success, fallback traffic and session stability, then define a replaceable relay
selection profile. Do not add voice, files or a project-operated public relay
until their respective engineering gates are met.

## M4a — friends-alpha text client (implemented locally)

- native Chinese/English desktop shell for macOS and Windows
- automatic test identity initialization in the platform application-data path
- one-community invite, channel, signed-message and history workflow
- bounded one-shot Home Server synchronization for interactive clients
- Home Server bootstrap derived from its signed declaration
- dynamic removal and addition of single-use bootstrap grants
- macOS arm64 and Windows x64 artifact workflow
- no recovery, multi-device identity, trusted code signing, attachments, voice,
  automatic update or public-service readiness

See [`ADR 0022`](adr/0022-friends-alpha-desktop-and-server-bootstrap.md).

## M4b — one reviewed client UI (in progress)

- one React/TypeScript product interface under `apps/client-ui`
- review adapter with demo data and authorized Annotate integration
- Tauri adapter boundary for validated Rust snapshots and commands
- website-only product explanation and download wrapper
- current eframe client retained only as a protocol diagnostic harness
- macOS-first Tauri integration before the next friend-facing desktop release

Browser review and desktop packaging must render the same client component
source. See [`ADR 0024`](adr/0024-single-client-ui-source.md).
