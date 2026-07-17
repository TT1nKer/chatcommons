# ADR 0008: Adaptive peer-assisted screen sharing

Status: accepted as a deferred media direction; not implemented

## Context

ChatCommons wants Discord-like screen sharing for small desktop communities without
making every session consume central video bandwidth. A full peer-to-peer fanout
makes the broadcaster upload one stream per viewer, while a mandatory central SFU
reintroduces an infrastructure cost and control point.

The initial target is one desktop broadcaster and no more than ten viewers. Mobile,
large public broadcasts, recording, and CDN-scale distribution are out of scope.

## Decision

Screen sharing will use an adaptive n-ary distribution tree when direct broadcaster
fanout is no longer appropriate:

- the broadcaster is the root for that media session;
- every viewer has at most one active parent;
- a viewer may relay encrypted media to zero or more children;
- each viewer advertises a bounded relay capacity derived from its selected upload
  limit and current network conditions;
- topology assignment is automatic; users select contribution capacity, not a
  position in the tree;
- implementations must prevent cycles and bound tree depth and per-node fanout;
- a child keeps enough information to select another parent if its parent leaves;
- direct delivery is preferred for very small sessions, and a replaceable SFU is
  the fallback when the peer tree cannot provide acceptable quality.

Joining a screen-sharing session means participating in cooperative distribution
when the device has capacity. The client must disclose this behavior, bound upload
usage, show active contribution, and allow the user to disable relaying. A viewer
that disables relaying remains eligible to receive media, subject to available
peer or SFU capacity.

Media payloads are encrypted and authenticated end to end. Relay peers forward
encoded ciphertext and do not decode or re-encode the screen content. Topology is
ephemeral session state: it is not stored in the community event DAG, does not
create a community role, and disappears when the broadcast ends.

## Consequences

For a ten-person session, upload can be distributed across capable desktop peers
instead of concentrating nine copies at the broadcaster or an official server.
The total network work does not disappear; participating peers voluntarily supply
part of it.

Playback may briefly pause when a forwarding peer leaves. Reparenting, keyframe
requests, congestion control, capacity measurement, and SFU migration must be
specified and tested before implementation. Fixed tree arity, reward systems,
reputation, token incentives, and permanent contribution accounting are explicitly
rejected for the first media version.

This decision does not add media functionality to Core and does not change the M2
priority of bounded text-event synchronization and replaceable network assistance.
