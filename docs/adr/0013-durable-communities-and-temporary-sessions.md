# ADR 0013: Durable communities and temporary sessions

Status: accepted product boundary; temporary sessions are not implemented

## Context

Low-friction temporary voice rooms are useful when a few friends want to talk or
share a screen immediately. Long-lived Discord-like communities solve a different
problem: stable membership, topic channels, accumulated context and finding
people with shared interests inside a known community.

Treating these as unrelated products would duplicate identity, invitations and
transport. Treating live media as durable history would waste resources and make
deletion promises misleading.

## Decision

ChatCommons will expose two product entry points over shared foundations:

- durable communities use signed membership facts and replicated event history;
- temporary sessions use the same identities, invitations and connectivity but
  do not enter durable history by default.

The first implementation remains durable text communities. Temporary rooms,
voice and screen sharing are deferred. Community-local channel directories,
roles and live-session listings may later help members find like-minded people;
a global public directory or recommendation feed is not part of the protocol
core.

## Consequences

A temporary session may naturally become unavailable when no participant or
optional service remains online. A client may clear local transient state, but
the protocol cannot guarantee that another participant did not record content.

Persistent community availability still requires willing replica nodes. Live
media may use peer-to-peer transfer for small rooms and replaceable relay/SFU
services when necessary without making those services owners of community
identity or durable history.
