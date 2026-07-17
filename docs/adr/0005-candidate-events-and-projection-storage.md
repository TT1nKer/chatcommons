# ADR 0005: Candidate events and projection storage

Status: superseded by ADR 0007; retained as protocol-v1 history

## Context

A cryptographically valid event can later become authorization-ineffective when a
node learns about a concurrent revocation. Deleting that signed event would lose
audit evidence, while treating every stored event as effective would make SQLite
arrival order authoritative.

## Decision

SQLite stores signed candidate events in the append-only `events` table and keeps
their current projection result separately in `event_resolution`. A row is either
effective or rejected with a stable machine-readable reason.

Candidate insertion and replacement of the community's resolution rows occur in
one SQLite transaction. The resolver runs before that transaction over the full
candidate set. After commit, `NodeCore` rebuilds its in-memory community projection
from `accepted_in_order` rather than replaying database row order.

Database recovery always reloads all candidates and reruns the pure resolver. This
also supports M1 databases that have no resolution rows yet. `NodeCore::submit`
remains a local non-conflicting fast path; `ingest_candidates` is the M2 API for
candidate-set changes and projection persistence.

## Consequences

Learning a new event can change an older event from effective to rejected without
destroying its signature or payload. Audit queries can retrieve both the event and
its rejection code. Recovery no longer trusts insertion order.

The rejected event body may contain sensitive content, so future retention,
redaction and encrypted-at-rest policy must distinguish audit necessity from
indefinite storage. M2 does not yet implement rejected-event expiry or compaction.
