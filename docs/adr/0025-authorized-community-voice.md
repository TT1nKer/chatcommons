# ADR 0025: Authorized community voice through a replaceable SFU

Status: accepted; friends-alpha implementation in progress

## Context

Voice is a defining part of the friends-alpha experience, but a pure peer mesh
becomes unreliable behind NAT and multiplies each participant's upload traffic.
The current test community has one small Home Server and targets rooms of at
most ten participants. Video, recording, broadcasting and end-to-end media
encryption are not requirements for this milestone.

Giving the desktop client a long-lived media-server secret would let any copied
client mint arbitrary room credentials. Trusting a room name supplied only by
the UI would also bypass community membership and channel authorization.

## Decision

Friends-alpha voice uses one self-hosted LiveKit SFU beside the Community Home
Server. This is a replaceable media transport, not protocol truth:

1. an authenticated member device requests voice access for a community channel
   over the existing mutually authenticated sync connection;
2. the Home Server resolves the current signed community state again;
3. only a current member targeting an existing channel receives a five-minute
   LiveKit participant token;
4. the token grants join, microphone publish and subscribe only;
5. the desktop client connects directly to the configured `wss` endpoint and
   releases microphone, room and audio-element resources on leave.

The room identifier is derived from the complete Community ID and Channel ID.
The API secret exists only in the Home Server environment and the LiveKit
configuration. Voice grants are bounded untrusted input at every process and
network boundary and are never persisted or logged.

The first deployment limits each room to ten participants. It exposes one TCP
fallback port and a small UDP media range. The web review adapter cannot request
real voice credentials.

No signed community event format changes. A future server migration can replace
the voice endpoint and issuer configuration without changing Community ID,
membership or message history.

## Consequences

- The feature remains usable when two clients cannot establish direct media
  paths, at the cost of routing their audio through the community-operated SFU.
- The Home Server remains the authorization boundary but does not carry media.
- Five-minute bearer tokens limit, but do not eliminate, replay risk.
- The current SFU can observe media and connection metadata. End-to-end media
  encryption requires a separate design and is not implied by signed text.
- Voice is unavailable while either the Home Server token path or the SFU is
  down. Text messaging continues independently.
- A single small server is suitable only for invited alpha testing. Capacity,
  monitoring and abuse controls must be measured before raising the room limit.
