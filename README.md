# ChatCommons

ChatCommons is an open, offline-first protocol for community-owned chat. Its
goal is simple: your community, your rules, your chat. The current workspace
contains protocol v2, one deliberately small reference chat profile, single-use
bearer invitations, secure direct invitation bootstrap, and minimal direct QUIC
synchronization. It contains no hosted service, release binary, voice
implementation, or GUI.

## Workspace

- `chatcommons-crypto`: Ed25519 identities and byte-level verification
- `chatcommons-cli`: Unix-only M2c/M2d diagnostic node executable
- `chatcommons-protocol`: opaque signed envelopes, canonical encoding, parsing and IDs
- `chatcommons-storage`: idempotent SQLite event persistence
- `chatcommons-node-core`: generic DAG validation and local ingestion
- `chatcommons-profile-chat`: the optional `chatcommons.chat.v1` reference semantics
- `chatcommons-sync`: bounded DAG synchronization and authenticated direct QUIC

## M2c/M2d diagnostic node

The current executable is a developer connectivity tool, not an end-user client.
It persists plaintext development keys only on Unix, with a `0700` state
directory and `0600` identity file. Do not reuse these keys for high-value or
production identities.

```sh
cargo run --bin chatcommons-node -- init --state <node-a-directory>
cargo run --bin chatcommons-node -- init --state <node-b-directory>
cargo run --bin chatcommons-node -- create-community \
  --state <node-a-directory> --name "Friends"
```

Create a single-use invite containing node A's reachable QUIC address:

```sh
cargo run --bin chatcommons-node -- create-invite \
  --state <node-a-directory> \
  --community <community-id> \
  --address /ip4/<node-a-public-ip>/udp/4001/quic-v1
```

Start node A at the same address and let node B join using only `INVITE_CODE`:

```sh
cargo run --bin chatcommons-node -- run \
  --state <node-a-directory> \
  --community <community-id> \
  --listen /ip4/0.0.0.0/udp/4001/quic-v1

cargo run --bin chatcommons-node -- join \
  --state <node-b-directory> \
  --invite-code <cc1-code>
```

The code contains a bearer secret and the diagnostic CLI exposes it in terminal
and process arguments. Use development identities only. The command has no
discovery, hole punching, relay or process lock. Run one process per state
directory and restrict the diagnostic listener to a test environment. See
[ADR 0014](docs/adr/0014-m2c-diagnostic-node.md) and
[ADR 0015](docs/adr/0015-secure-invitation-bootstrap.md).

Run all quality gates:

```sh
cargo fmt --all -- --check
cargo build --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Core verifies cryptographic facts; profiles decide what those facts mean. See
[docs/protocol.md](docs/protocol.md) and
[ADR 0007](docs/adr/0007-less-is-more-core-profile-boundary.md). The current
interoperability fixture is
[docs/test-vectors/core-v2-genesis.json](docs/test-vectors/core-v2-genesis.json).
The v1 fixture remains available as historical test material only.

The reference product has two compatible use cases: durable communities provide
stable membership and replicated history; temporary rooms will later provide
low-friction voice and screen sharing without durable history. Only the durable
text-community foundation is implemented today. See
[ADR 0013](docs/adr/0013-durable-communities-and-temporary-sessions.md).

Security and governance baselines live in
[docs/security/threat-model.md](docs/security/threat-model.md),
[docs/governance/control-boundaries.md](docs/governance/control-boundaries.md), and
[docs/governance/china-launch-checklist.md](docs/governance/china-launch-checklist.md).
The executable go/no-go checklist is
[docs/governance/engineering-gates.md](docs/governance/engineering-gates.md).
