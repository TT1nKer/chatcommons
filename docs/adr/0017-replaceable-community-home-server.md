# ADR 0017: Replaceable community home server

Status: accepted; protocol declaration implemented, server operation deferred

## Context

Pure peer-to-peer availability makes a new device dependent on whichever member
happens to be online and holding a complete replica. A durable chat community
instead needs one predictable, long-running default synchronization endpoint.
Making that endpoint the community identity, however, would let a domain owner,
hosting vendor, or ChatCommons-operated service permanently lock in the
community.

ChatCommons therefore needs community-local operational centralization without
ecosystem-wide ownership centralization. The home server is responsible for
availability. The signed community history remains the source of identity and
authority.

## Decision

Every community may project one current logical `HomeServerBinding`. The binding
is selected by an ordinary signed chat-profile event:

```text
HomeServerSet {
    server_public_key,
    endpoints[],
}
```

The current owner is the only author accepted by the reference profile. The
server public key derives a stable `HomeServerId` using BLAKE3 with the domain
separator `chatcommons:home-server-id:v1`. It never becomes a community owner,
administrator, or event author merely by being selected.

The declaration event's DAG parents are its history checkpoint. A separate
timestamp, old-server approval, official permit, domain ownership proof, or
mutable platform record does not determine validity. Endpoint strings are signed
discovery hints: one to eight unique ASCII values, each at most 512 bytes and
without whitespace or control characters. A later transport profile must define
their concrete syntax and prove possession of the selected server key.

Concurrent owner-authored declarations are applied in deterministic Event ID
order, independent of arrival order. The projected binding is the last accepted
declaration in that order. Timestamps retain their Core meaning as signed
metadata and receive no separate migration precedence. Because the owner can
author all competing declarations, deterministic selection provides convergence,
not protection from an owner intentionally publishing conflicting endpoints.

The reference profile advances from `chatcommons.chat.v1` to
`chatcommons.chat.v2`. Core protocol v2, Event IDs, Community IDs, invitation
capabilities, and generic storage remain unchanged. Existing diagnostic v1
state is development-only and must be recreated; no automatic profile migration
is defined.

## Operational boundary

A home server is expected eventually to provide complete event history, offline
delivery, indexes, attachment storage, and a default bootstrap endpoint. Clients
still verify events and profile authorization locally. A server can withhold,
delay, observe metadata, or delete its own copies, but cannot forge a valid owner
migration or change the genesis-derived Community ID.

An owner may select a replacement without cooperation from the old server or
ChatCommons. Data recovery is separate from that right: recovered history is
limited by old-server export, backups, and member replicas. At minimum, a usable
recovery must retain genesis and the governance ancestry needed to authenticate
the current owner and home-server declaration.

Community ID alone does not locate a new endpoint. A signed declaration must
reach clients through at least one available channel, such as member gossip, a
new server presenting its proof chain, a fresh invite, a community announcement,
or a replaceable directory. Directories aid discovery but never authorize a
migration.

When the home server is briefly unavailable, already connected clients may keep
signed events locally and exchange them directly. This is bounded degraded
operation, not an obligation for peers or ChatCommons to provide indefinite
hosting. Sovereign Realm policy may forbid every official endpoint; clients must
fail closed rather than silently fall back.

## Consequences

Community identity, governance, and membership survive a server replacement.
The current operator can stop serving or refuse export, but cannot veto a valid
owner-authored declaration. Official hosting can end its own service without
creating a protocol-level revocation of the community.

The implemented foundation does not yet provide a home-server binary, endpoint
transport authentication, server discovery, export/import tooling, signed state
snapshots, attachment migration, backup automation, Realm policy enforcement, or
cross-Realm federation. Those require separate milestones and tests rather than
empty interfaces in this change.
