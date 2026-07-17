# ADR 0011: Minimal direct QUIC synchronization

Status: accepted and implemented for M2b

## Context

M2a proves convergence in memory but not across real sockets. The first network
milestone should test encrypted authenticated transport without also introducing
discovery, NAT traversal, relay operation, or a public service.

The default `chatcommons.chat.v1` admission model is community-initiated: an
administrator publishes a one-time capability invitation and a bearer proves
possession while binding it to their `UserId`. The invitation is consumed once.
Network access must not turn cheap public keys into community access.

## Decision

M2b carries the existing synchronization messages through rust-libp2p QUIC
request-response streams:

- each device's persistent Ed25519 key is its libp2p Peer ID identity;
- peers exchange user-signed device certificates before synchronization;
- the certificate device key must derive the authenticated connection Peer ID;
- the certificate user must be in a caller-provided allowed-user set;
- known user/device revocations reject authentication;
- neither side sends community heads until it has verified the remote certificate
  and received confirmation that the remote accepted its certificate;
- network requests and responses have a 1 MiB frame limit and at most 64 embedded
  synchronization messages;
- `Want` requests are normalized to one Event ID per network request so a response
  always remains bounded when an event is near the maximum payload size.

The implementation accepts an already resolved allowed-user set instead of
depending on `chatcommons.chat.v1`. A different Profile can therefore provide a
different admission projection without changing the transport.

## Consequences

Two real libp2p Swarms can authenticate over loopback QUIC and synchronize
independent SQLite databases. A caller can dial a known IP/port/Peer ID tuple.

This is not yet internet connectivity for ordinary users. There is no address
discovery, DNS bootstrap, hole punching, relay, TCP fallback, certificate storage,
revocation distribution, connection rate limiting, or CLI. Device certificates
are currently presented to every established peer, so identity-metadata privacy
must be tightened before accepting arbitrary public inbound connections.
