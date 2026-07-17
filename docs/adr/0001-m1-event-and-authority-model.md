# ADR 0001: M1 event and authority model

Status: superseded by ADR 0007; retained as protocol-v1 history

## Context

M1 must prove offline identity, authorization and persistence without committing
to a network stack or a full concurrent state-resolution algorithm.

## Decision

The genesis event has no community field; its verified Event ID is the Community
ID. Ed25519 signatures use an explicit hand-written canonical encoding rather
than a serializer configuration. JSON is only the storage and parsing envelope.

The genesis author is the initial administrator. Invitations grant ordinary
membership after an explicit acceptance event. Revocation immediately prevents
later events processed against that local validated state. Per-author sequences
are contiguous, while DAG parents need not form a single chain: independent
authors may share a parent and retain both branches.

M1 accepts events only after their parents. This produces deterministic recovery
for the locally accepted log but does not claim to resolve concurrent conflicting
authorization events received in different orders.

## Consequences

The implementation is small and independently testable. Timestamps cannot decide
authority or uniqueness. Before network synchronization, M2 must define causal
authorization-state resolution and pending-parent ingestion so replicas cannot
diverge when conflicting state events arrive in different orders.
