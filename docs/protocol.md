# Protocol v2: minimal signed event core

An identity is an Ed25519 public key. `UserId` is BLAKE3 of that key and is
derived rather than included in signed content. The Core does not define members,
roles, channels, bans, ownership, or messages. Those semantics belong to profiles.

## Event lifecycle

Untrusted JSON is first parsed into `ParsedEvent`. Parsing does not grant
validity. Calling `validate` checks structural limits, signature, and Event ID.

The signature covers a manually encoded canonical byte sequence with fixed field
order, big-endian integers, and length-prefixed bytes. Event ID is BLAKE3 over a
domain separator, canonical content, public key, and deterministic Ed25519
signature. It is never random. `event_type` is UTF-8; `payload` is opaque bytes.

The genesis event omits `community_id` and parents; its verified Event ID becomes
the Community ID. Every later event carries that ID and at least one sorted,
unique parent. Timestamps are metadata only and never establish uniqueness,
causal order, or authority.

The canonical content fields, in order, are: domain separator, protocol version,
optional Community ID, parent count and parent IDs, timestamp, event type, and
payload. There is no claimed author field or per-author sequence. The signer is
derived from the public key, and the DAG expresses causality. Two events may
reference the same parent; both branches remain valid facts.

## Core and profile boundary

`chatcommons-node-core` accepts only cryptographically valid events for one community
whose parents are present, and stores them idempotently. It does not decide whether
an author is allowed to perform an application action.

`chatcommons-profile-chat` is the single reference profile. It currently demonstrates
community creation, invitations, membership, text channels, messages, one owner,
and administrators. Its deterministic resolver is application policy, not a
universal governance engine. Other applications may define different profiles
without changing Core.

The default admission rule in `chatcommons.chat.v1` is community-initiated only.
There is no join-request event. An administrator invitation publishes a fresh
Ed25519 capability public key. The corresponding private capability exists only
in a bounded invite package shared by link, QR code, or another out-of-band
channel; it never enters the community event DAG.

The accepting user signs the normal event with their identity and also proves
possession of the invitation capability. The capability signature binds the
Community ID, invitation Event ID, and accepting `UserId`. A valid invitation can
therefore add one bearer and can be consumed only once. Concurrent redemptions
are resolved by the profile's deterministic event ordering, not by timestamps or
arrival order. Anyone who steals the complete invite package can attempt to
redeem it; transport confidentiality and user care remain necessary.

Invite package JSON is parsed separately from validation, has a 4 KiB limit, and
must match a cryptographically valid invitation event before an acceptance can be
created. This package check proves correspondence, not administrator authority;
the invitation and acceptance receive final authorization only when the profile
resolves them with the complete community event set. Expiry, cancellation and
contacts are deliberately deferred. M2d can wrap the unchanged package with one
explicit direct endpoint and uses a restricted challenge flow to fetch invitation
ancestry without granting ordinary synchronization. A community that wants
another admission model must select or define a different profile; Core does not
impose this policy globally. See
[`ADR 0012`](adr/0012-single-use-bearer-invitations.md) and
[`ADR 0015`](adr/0015-secure-invitation-bootstrap.md).

## Resource limits

Protocol v2 rejects JSON envelopes above 256 KiB, more than 32 parents, event types
over 64 bytes, and payloads over 64 KiB. Unordered local batch ingestion is capped
at 1,024 unique events and resolves only parent dependencies.

The fixed v2 genesis vector is published at
[`docs/test-vectors/core-v2-genesis.json`](test-vectors/core-v2-genesis.json). It
locks canonical bytes, public key, deterministic signature, Event ID, and derived
Community ID. Protocol v2 is intentionally not wire-compatible with v1.
