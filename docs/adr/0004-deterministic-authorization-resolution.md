# ADR 0004: Deterministic authorization resolution

Status: superseded by ADR 0007; retained as protocol-v1 history

## Context

Arrival order and timestamps cannot decide concurrent community authority. A node
may first accept an invitation and later learn that a concurrent revocation should
make that invitation ineffective. Append-only incremental mutation cannot safely
handle this case without recomputing a projection from the candidate event set.

## Decision

`resolve_events` is a pure function over a complete candidate set. It verifies
every event, walks causal dependencies, and returns accepted Event IDs in a stable
application order, rejected IDs with reasons, and the resulting community state.

The current event model uses these rules:

1. causal ancestors are evaluated before descendants;
2. a member revocation supersedes concurrent channel/invitation authorization
   authored by the revoked member, while authorizations causally before the
   revocation remain effective;
3. descendants of a rejected parent are rejected;
4. ready revocations are applied before other ready event types;
5. remaining concurrent choices use Event ID ordering, never timestamp or arrival
   order;
6. competing events occupying the same per-author sequence slot have one stable
   winner and the others are rejected.

Signed rejected events remain cryptographic facts, but this M2 resolver does not
yet define a rejected-event audit table in SQLite.

## Consequences

Permutations of the same tested candidate set produce byte-for-byte equal
resolution output and state snapshots. Adding a newly discovered concurrent event
may change the effective projection, so callers must recompute from the full set.
The existing `NodeCore::submit` method remains a fast path for causally ordered,
non-conflicting local histories and must not be treated as the future network
ingestion API.

This ADR covers the current single-initial-administrator model. Administrator
grants, ownership transfer, multisignature authority and conflict compaction need
new rules before those event types are added.
