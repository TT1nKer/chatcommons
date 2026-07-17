# ADR 0009: Transport-independent event synchronization

Status: accepted for M2a

## Context

Core v2 can validate and persist an event DAG but does not move facts between
nodes. Adding libp2p, NAT traversal, relay operation, peer authentication, and DAG
reconciliation in one step would make failures difficult to isolate and would
prematurely bind the event protocol to one transport.

## Decision

M2a defines four bounded, transport-independent messages:

- `Hello`: announces the sync version, supported Core versions, and Community ID;
- `Heads`: announces the current DAG tips;
- `Want`: requests events by Event ID;
- `Events`: returns signed Core events.

After receiving unknown heads, a node requests them. If an event arrives before its
parents, it remains in a bounded in-memory pending set and the missing parents are
requested recursively. Once all parents are present, the existing Core validation
and SQLite insertion path is used. Both peers send `Hello`, so independent branches
are exchanged in both directions. Duplicate facts remain idempotent.

Every message has a Community ID. IDs must be sorted and unique. JSON envelopes,
version lists, ID lists, event batches, and pending events have hard limits.
Parsing does not imply validity. A peer never serves an Event ID that is absent from
its locally validated community store.

M2a deliberately does not authenticate a remote node or equate a node with a user.
Authentication belongs to the future secure transport handshake, and the choice
between user, device, and node keys requires a separate multi-device decision.

## Consequences

Two in-process peers with independent SQLite databases can now converge without a
network stack. Heads keep the normal transcript small, while recursive parent
retrieval restores history without sending a complete inventory first.

This is not yet a production anti-entropy protocol. A malicious peer can advertise
unavailable heads, withhold parents, or repeatedly reconnect. Session request
budgets, timeouts, peer reputation, privacy leakage from Event IDs, and transport
authentication must be decided before exposing the protocol to the public network.
