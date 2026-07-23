# ADR 0022: Friends-alpha desktop and Home Server invitation bootstrap

Status: accepted; friends-alpha implementation complete locally

## Context

The protocol, diagnostic CLI and Community Home Server can already synchronize
signed events, but a friend cannot use them without learning commands, Peer IDs,
Multiaddrs and local state paths. Invitation bootstrap also encoded the inviting
member's device as the only endpoint, so a durable community still depended on
that member being online when a new person joined.

The first user test needs one complete path, not a general client framework:
create a local identity, consume one invite, see accepted channels and messages,
send a signed message and synchronize it with the declared Home Server.

## Decision

`chatcommons-desktop` is a small native eframe application distributed beside
the existing `chatcommons-node` executable. The desktop process provides only
product state and UI; it invokes the sidecar without a shell and renders only
the channels and messages returned after Core validation and deterministic chat
profile resolution. Protocol crates do not depend on either UI or CLI.

The friends alpha supports one local user/device identity and one joined
community. It automatically initializes state under the platform application
data directory. Unix retains mandatory private directory and file modes.
Windows inherits the current user's application-data ACL and makes no claim of
protection from local administrators. These identities are explicitly test
identities.

The release workflow produces the macOS and Windows client bundles plus the
Linux x86-64 Home Server binary from the same revision. This keeps the deployed
infrastructure behavior aligned with the invitation logic used by clients.

When `create-invite` receives no explicit address, it derives the target Peer ID
and first supported QUIC Multiaddr from the current signed `HomeServerSet`.
During join, the client accepts the bootstrap endpoint only when its presented
device is either an active member or the exact device in that signed Home Server
binding. Infrastructure status grants bootstrap and synchronization only; it
does not make the server operator a member or administrator.

Home Servers replace their active bootstrap-grant set whenever signed community
state changes. Consumed invitations are therefore removed, and newly
synchronized invitations become available without giving the server authority
to invent them.

One-shot desktop synchronization starts its idle-completion timer only after a
real sync request or response has been processed. Authentication and transport
response notifications are not sync completion signals. This prevents a client
from disconnecting before a newly created message or invitation reaches the
Home Server under load.

## Consequences

Friends can join while the owner is offline, and the same signed invitation,
membership and server declarations remain valid outside the desktop UI. The
desktop application remains replaceable because it delegates to documented
protocol operations rather than a private hosted API.

This alpha has important limitations:

- the invitation bearer secret is briefly present in a local sidecar process
  argument;
- there is no account recovery, multi-device authorization or keychain-backed
  identity container;
- binaries are not notarized or signed by a trusted publisher;
- there is no automatic update, notification, attachment, voice or background
  reconnect loop;
- one client state cannot yet join multiple communities;
- public Home Server operation remains outside the production engineering and
  legal gates.

These constraints are visible product limitations, not empty extension
interfaces. The next client milestone should remove sidecar argument secrets,
add OS credential storage and test migration before expanding community count.
