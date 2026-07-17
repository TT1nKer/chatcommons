# ADR 0003: Bounded input and pending events

Status: superseded by ADR 0007; retained as protocol-v1 history

## Context

Future peers may deliver valid events out of causal order and malicious peers may
send inputs designed to consume memory, CPU or storage. Network transport remains
deferred, but these rules must exist before it is introduced.

## Decision

Protocol v1 limits an encoded JSON event to 256 KiB, parents to 32, community and
channel names to 128 UTF-8 bytes, and message text to 16 KiB. Limits use encoded
bytes rather than character counts so resource use is predictable across scripts.

An unordered local batch is capped at 1,024 events. Every event receives structural
and cryptographic preflight before insertion begins. Events blocked only by an
unknown community, missing parent or missing earlier author sequence are retried.
Other authorization failures reject the batch operation rather than being hidden
as indefinitely pending events.

## Consequences

Valid causal histories converge under tested arrival permutations without an
unbounded queue. The current API returns unresolved Event IDs after reaching a
fixed point; persistence and eviction of pending events remain deferred until a
network synchronization design exists.

Batch ingestion is not an atomic database transaction in M2: valid events accepted
before a later contextual error remain stored. Callers must treat the report as
incremental ingestion, not all-or-nothing import.

This does not resolve conflicting concurrent authorization events. Such conflicts
need a separate deterministic rule before M2 is complete.
