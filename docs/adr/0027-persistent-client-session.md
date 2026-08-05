# ADR 0027: Persistent client session for low-latency synchronization

Status: proposed

## Context

The friends-alpha desktop currently starts `chatcommons-node
sync-home-server` for every synchronization attempt. The command acquires the
profile lock, creates a new libp2p node, binds a listener, establishes and
authenticates a QUIC connection, exchanges events, waits 1.5 seconds for idle
completion and exits. The UI waits another two seconds before repeating the
same sequence.

Revision `938574d` was measured on the macOS and Windows friend-test devices
against the Silicon Valley Home Server. The server round-trip time was 163 ms
from macOS and 136 ms from Windows with no ICMP loss. In contrast, the latest
100 macOS synchronization attempts had a 2746 ms median and 3793 ms p95; 21
Windows attempts had a 3060 ms median and 3679 ms p95. The local operation-lock
wait was normally zero. Repeated connection setup and idle completion, rather
than SQLite or rendering, dominate the delay and continuously consume network
and process resources.

Reducing the polling delay or idle timeout would improve a benchmark while
creating more handshakes. Moving the same behavior to a nearer server would
reduce physical RTT but preserve the structural cost. Neither is a durable
latency design.

## Decision

Keep the existing sidecar trust boundary, but replace per-operation sidecars
with one supervised client-session sidecar per local profile.

The session sidecar will:

- exclusively own the profile lock, SQLite store and live `NetworkNode`;
- maintain one authenticated QUIC connection to the signed Home Server target;
- accept bounded, versioned local commands over inherited stdin and emit
  bounded responses and state-change notifications over stdout;
- apply send, invitation and synchronization operations to the same validated
  core before announcing new heads on the existing connection;
- reconnect with bounded backoff while retaining locally validated history;
- never log message bodies, invitation capabilities, identity keys or media
  grants.

The desktop host will supervise that child without a shell. React will continue
to send user intent and render validated snapshots; it will not own protocol
truth. A crashed session may be restarted, but the client must not silently
switch to an untrusted server, Relay or recovery provider.

`NetworkNode` will gain an explicit operation for restarting synchronization on
an already authenticated connection. A Home Server that ingests new signed
events will announce current heads to its other authenticated peers, allowing
connected clients to receive changes without reconnecting or polling.

This change does not alter signed event bytes, Community IDs, Home Server
bindings, governance, invitation semantics or on-disk event data. Existing
one-shot CLI commands remain diagnostic tools and compatibility surfaces.

## Implementation slices

1. Add and test re-synchronization on an existing mutually authenticated
   `NetworkNode` connection.
2. Define a small bounded local session command/response codec and test
   malformed, oversized and out-of-order input.
3. Move profile-owning send, snapshot and sync flows behind the long-lived
   session process; keep voice token acquisition separate until text is stable.
4. Supervise the session from Tauri and replace periodic UI polling with
   state-change notifications plus explicit manual refresh.
5. Re-run the same two-device marker test before changing regions, then repeat
   it against a nearer Home Server as a controlled A/B comparison.

## Consequences

This introduces a local process protocol and session lifecycle that require
careful failure handling. It removes repeated identity-file reads, lock
acquisition, process startup and QUIC authentication from the steady-state
message path. A connected sender can publish immediately and a connected
receiver can observe the server announcement without waiting for a poll.

P2P and Relay routing remain transport choices. They may reduce server traffic
or survive a Home Server interruption later, but they are not prerequisites
for fixing the current text latency.
