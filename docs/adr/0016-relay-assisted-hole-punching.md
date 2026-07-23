# ADR 0016: Relay-assisted hole punching with bounded fallback

Status: accepted, implemented and physically measured across two NATs

## Context

M2d could join through one direct QUIC address, but most users should not need a
public IP address or manual router configuration. Direct dialing alone cannot
reliably cross NATs, while making all traffic depend on one hosted application
server would conflict with ChatCommons' replaceable-infrastructure goal.

The minimum useful design needs a rendezvous path, a direct-upgrade attempt and a
working fallback. It must not move community identity, authorization, history or
profile policy into the relay.

## Decision

M2e composes the existing authenticated sync behavior with libp2p Identify,
ping, Circuit Relay v2 client and DCUtR behaviors. TCP with Noise/Yamux and QUIC
are available as relay transports; ChatCommons application traffic keeps the
same device authentication and bounded request-response protocol.

An M2e bootstrap route is either:

- the existing direct UDP/QUIC multiaddress; or
- one TCP or QUIC relay base ending in the relay Peer ID, followed by
  `p2p-circuit`.

The existing `cc1_` envelope version and field layout do not change. M2d readers
continue to accept direct codes but will reject relay routes they do not
understand. Parsing and route validation remain separate.

For a relay route, each node first establishes and identifies a normal connection
to the relay. A serving node requests a reservation; a joining node then opens a
circuit to the serving Peer ID. Once the relayed connection exists, DCUtR uses
observed addresses to attempt a direct connection. A failed direct candidate or
hole punch does not tear down a working relayed connection.

Relay Peer IDs are tracked as infrastructure peers. They never receive a
ChatCommons device certificate, bootstrap capability proof, community allowlist
decision or DAG synchronization request. The relay forwards the Noise-protected
stream and owns no community state.

The included diagnostic relay is deliberately bounded to 128 reservations, 256
circuits, one reservation and four circuits per peer, two minutes and 8 MiB per
circuit, plus per-peer and per-IP request limits. Its identity is ephemeral and
it stores no application history. These bounds make tests finite; they are not a
production capacity or abuse policy.

Only one pending reservation and one pending target dial per relay are supported
by a node in M2e. This is sufficient for the single-endpoint invitation flow and
avoids a general connection manager before it is needed.

## Consequences

The normal path is direct when possible, with a replaceable relay as availability
fallback. A relay is infrastructure rather than a trusted community server, but
it can observe connection metadata and consume real bandwidth. End-to-end
transport confidentiality does not hide IP addresses, Peer IDs, timing or byte
counts from it.

The route still names one relay and one serving endpoint, so stale invites and
relay outages can prevent joining. M2e has no relay discovery, rotation, scoring,
DNS policy, offline mailbox, process lock, persistent relay identity or graceful
shutdown. Symmetric NATs and restrictive firewalls may keep the session relayed.

The diagnostic relay must not be exposed publicly. Operating any shared relay
requires Gate B data-flow review, retention decisions, monitoring, abuse handling,
legal classification and production identity/key management. Physical testing
must use separately approved infrastructure; a borrowed application server is
not implicitly authorized to carry ChatCommons traffic.

## Physical measurement

On 2026-07-18, a macOS node on a phone hotspot and a Windows/WSL node on a
separate home connection completed invitation bootstrap through an independently
operated IPFS Circuit Relay v2 peer. Kubo was used only to discover a public peer
that explicitly advertised the Relay v2 hop protocol; it was stopped before the
ChatCommons connection. Tailscale carried the SSH control session only and its
addresses did not appear in the invitation route or application connection.

Both peers connected with `via=relay`, authenticated devices, proved the invite
capability, accepted the member and persisted the same five-event ancestry. The
relay observed different public IPv4 endpoints for the two networks. A direct
candidate timed out for this NAT combination, after which the relay fallback
remained available and the serving node stayed alive.

The measurement exposed and fixed two ordering defects: a delayed DCUtR candidate
failure after clean application disconnect was incorrectly fatal, and multiple
connections to one Peer could start duplicate authentication and bootstrap
exchanges. Supplemental dial errors are now reported as hole-punch failures,
authentication is started once per connected Peer, and repeated ancestry after
the first validated response is handled idempotently.

This proves the fallback path for one real network pair; it is not a success-rate
claim. A broader NAT/firewall matrix, sustained sessions and traffic accounting
remain future measurement work. The discovered third-party relay is not a
ChatCommons default or an operated project service.
