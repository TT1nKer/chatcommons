# ChatCommons

ChatCommons is an open, offline-first protocol for community-owned chat. Its
goal is simple: your community, your rules, your chat. The current workspace
contains protocol v2, one deliberately small reference chat profile, single-use
bearer invitations, and minimal direct QUIC synchronization. It contains no
hosted service, release binary, voice implementation, or GUI.

## Workspace

- `chatcommons-crypto`: Ed25519 identities and byte-level verification
- `chatcommons-cli`: Unix-only M2c diagnostic node executable
- `chatcommons-protocol`: opaque signed envelopes, canonical encoding, parsing and IDs
- `chatcommons-storage`: idempotent SQLite event persistence
- `chatcommons-node-core`: generic DAG validation and local ingestion
- `chatcommons-profile-chat`: the optional `chatcommons.chat.v1` reference semantics
- `chatcommons-sync`: bounded DAG synchronization and authenticated direct QUIC

## M2c diagnostic node

The current executable is a developer connectivity tool, not an end-user client.
It persists plaintext development keys only on Unix, with a `0700` state
directory and `0600` identity file. Do not reuse these keys for high-value or
production identities.

```sh
cargo run --bin chatcommons-node -- init --state <node-a-directory>
cargo run --bin chatcommons-node -- init --state <node-b-directory>
cargo run --bin chatcommons-node -- create-community +  --state <node-a-directory> --name "Friends"
```

Exchange the printed User IDs out of band. Start node A:

```sh
cargo run --bin chatcommons-node -- run +  --state <node-a-directory> +  --community <community-id> +  --listen /ip4/<node-a-ip>/udp/0/quic-v1 +  --allow-user <node-b-user-id>
```

Copy node A's printed `PEER_ID` and `LISTEN_ADDRESS`, then dial it from node B:

```sh
cargo run --bin chatcommons-node -- run +  --state <node-b-directory> +  --community <community-id> +  --listen /ip4/0.0.0.0/udp/0/quic-v1 +  --allow-user <node-a-user-id> +  --dial-peer <node-a-peer-id> +  --dial-address <node-a-listen-address>
```

The command has no discovery, hole punching, relay, process lock, or secure
invitation bootstrap. Run only one process per state directory and do not expose
the diagnostic listener to untrusted networks. See
[ADR 0014](docs/adr/0014-m2c-diagnostic-node.md).

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
