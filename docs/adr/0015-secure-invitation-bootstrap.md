# ADR 0015: Secure single-endpoint invitation bootstrap

Status: accepted and implemented locally; route format extended by ADR 0016

## Context

The M1.1 invite package proves possession of a single-use admission capability,
but a new user still needed invitation ancestry before that package could be
validated. Ordinary M2b authentication could not solve this circular dependency:
the joining user was not a member yet and therefore could not receive ordinary
community synchronization.

The first product experience should require one shared value, while keeping
discovery and relay services out of the protocol. A bootstrap endpoint must not
become a new community authority or an unrestricted pre-membership data source.

## Decision

M2d adds a private, bounded bootstrap envelope around the unchanged M1.1 invite
package. Its `cc1_` Base64URL code contains:

- envelope version;
- the opaque invite package;
- exactly one libp2p Peer ID;
- exactly one validated endpoint route. M2d initially allowed a direct QUIC
  multiaddress without an embedded Peer ID; ADR 0016 additionally allows one
  Relay v2 circuit route.

The envelope is at most 8 KiB and its textual code at most 12 KiB. Parsing and
validation remain separate. The invite package stays limited to 4 KiB. Secret
buffers owned by parsed package objects are cleared on drop.

The joining device and endpoint then use this restricted flow:

1. both devices present user-signed device certificates bound to transport Peer
   IDs;
2. the endpoint verifies that the requested invitation is currently active and
   returns a random 32-byte challenge;
3. the invite capability signs a domain-separated proof binding the community,
   invitation, joining user, joining device, both Peer IDs and challenge;
4. only after valid proof does the endpoint return the invitation event and its
   ancestors, capped at 256 events and the existing 1 MiB network frame;
5. the joiner validates the package, profile projection and endpoint membership,
   then submits one signed `chat.member.accept` candidate;
6. the endpoint's chat profile validates that candidate against its event set;
   ordinary synchronization starts only after the candidate makes the joining
   user an active member.

Any active member holding the required history may be the packaged endpoint. It
receives no new governance authority. The generic network layer pauses the
acceptance response while the caller applies profile policy, preserving the Core
and profile boundary.

A future HTTPS link should use a fragment such as
`https://example/chatcommons/join/#<code>`. URL fragments are processed locally
and are not sent in the HTTP request. No landing page or short-link service is
implemented in M2d.

## Consequences

One copyable code is sufficient for joining when its packaged endpoint route is
reachable. A stolen code remains a bearer capability and may win redemption.
The challenge is not an expiry or cancellation mechanism.

One endpoint is intentionally a temporary availability weakness. If it is
offline, stale, or maliciously censoring, joining fails without changing
community state. ADR 0016 adds hole punching and one replaceable relay route but
does not add backup endpoints or discovery.

The diagnostic CLI passes the bearer code as a process argument and prints it to
the terminal. This is acceptable only for development identities; a product UI
must receive and retain the fragment locally without logging it. Invitation
ancestry is delivered as one bounded response, so large communities will need a
paged design before this path can scale.

Two independent endpoints can temporarily approve competing redemptions before
their histories converge. The deterministic profile still selects one final
winner, and a restarted diagnostic node authorizes only that resolved membership.
M2d does not yet proactively terminate an already-open session that later loses
such a conflict. Continuous membership re-evaluation is therefore required
before this becomes a public service.
