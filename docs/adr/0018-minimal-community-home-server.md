# ADR 0018: Minimal authenticated community home server

Status: accepted; diagnostic implementation complete

## Context

ADR 0017 lets a community owner select a replaceable Home Server, but a signed
address alone does not keep history available when members are offline. The
existing diagnostic node already has bounded QUIC synchronization, device
certificates, deterministic event validation, and SQLite persistence. A second
server protocol would duplicate those mechanisms and enlarge the trust surface.

## Decision

The first Home Server is a long-running role of the existing synchronization
node. `chatcommons-node serve-community` opens the same per-community SQLite
event store and sync protocol as a peer. It accepts current community members,
persists their valid Core event DAG, and serves missing events to members that
connect later. The members need not be online at the same time.

The `server_public_key` in `HomeServerSet` is the server's Ed25519 libp2p device
public key. A server process starts in the Home Server role only when its local
device key exactly matches the current signed declaration. A client admits that
exact device as infrastructure even though the server operator's user identity
is not a community member. The transport Peer ID, device certificate, and
declared device key must therefore describe the same device.

This exception grants synchronization access only. It does not add the server
operator to the membership or administrator projection, and it cannot authorize
chat or governance events. Ordinary members remain authorized from the signed
chat-profile projection.

After synchronization changes local history, the process recomputes membership
and the current Home Server binding. Removed members immediately lose network
sync authorization. A `serve-community` process whose device is no longer the
selected Home Server fails closed instead of continuing under the official
role. An old or malicious process can remain reachable, but updated clients no
longer trust it as infrastructure.

Core-valid events with malformed or mismatched chat payloads remain auditable
candidate facts but resolve to `InvalidPayload`. They cannot abort the complete
profile projection or stop the Home Server merely by failing application-level
decoding.

The diagnostic CLI adds:

```text
info                 -> prints DEVICE_PUBLIC_KEY
set-home-server      -> owner signs key and endpoint hints
serve-community      -> verifies the local key and serves the community
```

Before `serve-community` can start, its SQLite database must already contain the
genesis, governance ancestry, and current declaration. At this milestone an
operator could seed it through the existing authenticated `run` synchronization
path. ADR 0019 later adds bounded manual export/import, while automated backup
remains deferred.

## Consequences

A real QUIC integration test now covers three independent identities and
databases: one member uploads a signed message, disconnects, and another member
later retrieves it from the Home Server. It also proves that an undeclared
device cannot start in the Home Server role and is rejected by a client that
trusts only signed community state.

This is not a production public service. Identity seeds and community events are
stored locally by the diagnostic CLI, there is no process lock, endpoint/domain
proof, storage quota, rate limiter, monitoring, backup scheduler, attachment
store, service policy, or discovery directory. Public operation remains blocked
by the engineering and legal gates; this milestone proves the protocol and
availability path only.
