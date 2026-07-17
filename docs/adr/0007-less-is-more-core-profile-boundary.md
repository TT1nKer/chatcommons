# ADR 0007: Less-is-more Core and profile boundary

Status: accepted

## Context

Protocol v1 embedded a particular chat community's membership, administrator,
ownership, and conflict-resolution rules into the signed protocol core. That made
one governance model universal and increased the compatibility surface before any
network synchronization existed.

ChatCommons's purpose is narrower: make signed community facts portable between
replaceable nodes, while allowing communities and applications to interpret those
facts through an explicitly selected profile.

## Decision

Protocol v2 Core defines only:

- an Ed25519 signer public key;
- a genesis-derived Community ID;
- sorted, unique DAG parent references;
- metadata timestamp, event type, and opaque payload;
- deterministic canonical bytes, signature verification, and Event ID;
- bounded parsing, generic parent validation, and idempotent local storage.

Core derives the author identity from the public key. It has no redundant claimed
author, author sequence, roles, members, bans, owner, administrator, channel, or
message types.

The workspace includes exactly one small reference profile,
`chatcommons.chat.v1`. It owns the current chat payload encoding and governance
projection. Core and storage do not depend on that profile. No generic policy VM,
plugin interface, or role language is introduced.

## Consequences

Protocol v2 is a deliberate breaking change from v1. Old signed events remain
historical test material and require an explicit migration tool if compatibility
is ever chosen.

Core acceptance means “valid cryptographic community fact,” not “authorized chat
action.” A profile can reject a Core-valid event. Nodes that want a chat view must
run the same named profile and version. Profile negotiation and migration remain
deferred until real synchronization requires them.

This boundary keeps the irreplaceable consensus surface small. It does not solve
availability, moderation, legal obligations, or malicious storage; those remain
properties of profiles, node operators, and optional services.
