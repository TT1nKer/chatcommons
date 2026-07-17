# ADR 0012: Single-use bearer invitations

Status: accepted and implemented

## Context

The first admission design named an existing `UserId`. That assumed a contact or
account-discovery system before the product had one. The intended experience is
instead a link or QR code that works whether or not the recipient has previously
installed the client.

Cheap identity generation also means that accepting an unsolicited join request
cannot be the default small-community admission rule.

## Decision

`chatcommons.chat.v1` uses community-originated bearer capabilities:

- an administrator creates a fresh Ed25519 invitation capability;
- the signed `chat.member.invite` event contains only its public key;
- a private JSON invite package contains the 32-byte capability secret,
  Community ID and invitation Event ID;
- package parsing is bounded and separate from validation;
- validation requires a valid invitation event with the same community, event ID
  and capability public key;
- `chat.member.accept` is signed by the joining user and includes a capability
  signature over the Community ID, invitation Event ID and joining `UserId`;
- deterministic profile resolution accepts at most one redemption.

Package validation proves that the private capability corresponds to a validly
signed invitation event. It does not independently prove that the author was an
administrator; final authorization remains a deterministic profile projection
over the community event set.

The package is a protocol object, not a URL. HTTPS landing pages, custom URL
schemes, QR encoding, download hosting and bootstrap addresses remain later
product and network work.

## Consequences

No contact list, phone number, email address or central account lookup is needed.
An invitation is intended for one person but technically authorizes whoever first
wins deterministic valid redemption. Copying or stealing the complete package
therefore transfers the capability.

Link previews and scanners do not consume an invitation because redemption
requires a signed community event. Expiry and cancellation are intentionally not
implemented yet. They require separate conflict semantics and must not rely on
timestamps as an authority or ordering source.
