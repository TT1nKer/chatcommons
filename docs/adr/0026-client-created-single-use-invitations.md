# ADR 0026: Client-created single-use invitations

Status: accepted; friends-alpha implementation in progress

## Context

The protocol already supports administrator-signed, single-use invitation
events, but friends-alpha users previously had to run the node CLI and share a
large raw code manually. The shared client could join with an invitation but
could not create one.

An invitation created only in a local database is misleading: a friend cannot
redeem it until the event reaches the Community Home Server. The UI is also an
untrusted boundary and cannot decide whether the local identity is allowed to
invite members.

## Decision

The client exposes invitation creation through the existing one-way boundary:

```text
React intent
→ ClientAdapter
→ Tauri command
→ serialized Rust runtime
→ protocol CLI
```

The CLI's resolved community snapshot reports `canInvite` only when the current
identity is the owner or an administrator. This is presentation metadata; the
protocol validates authorization again when creating the signed event.

Invitation creation is ordered as follows:

1. verify that the selected Community ID matches the local client
   configuration;
2. synchronize successfully with the declared Community Home Server;
3. create the signed, single-use invitation event locally;
4. synchronize again to publish that event;
5. return the secret invitation code to the UI only after publication succeeds.

The UI shows the invitation for explicit copying and never writes it to logs,
URLs, telemetry or feedback. It explains that each invitation is for one person
and should be shared privately.

This milestone does not add contacts, public join requests, reusable links,
short-link infrastructure, deep links or a directory service.

## Consequences

- A regular member does not see invitation controls, while protocol validation
  remains the actual security boundary.
- A temporarily unreachable Home Server prevents creation of an apparently
  usable invitation.
- If the second synchronization fails, the append-only local history contains
  an unused signed invitation event, but the code is not returned to the user.
  Signed history is not rolled back. A later invitation can be created after
  synchronization succeeds.
- The raw code remains large because it carries the current bootstrap material.
  Improving how it is transported is a separate protocol and product decision.
