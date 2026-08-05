---
name: develop-chatcommons-client
description: Implement or review ChatCommons friends-alpha client changes across apps/client-ui and apps/desktop, including React interactions, localization, Tauri IPC, identity and invitation onboarding, signed text synchronization, voice controls, private feedback, and client tests. Use for user-facing desktop behavior or the frontend-to-Rust adapter boundary; do not use for protocol-only, Home Server-only, infrastructure-only, or marketing-site-only work.
---

# Develop the ChatCommons Client

Keep the downloadable desktop client and browser review surface on one React
source while preserving the Rust trust boundary.

## Establish the Change Boundary

1. Read the repository `AGENTS.md`.
2. Read the current call path, data types, tests, and only the relevant design
   sources:
   - Product scope: `docs/product-brief.md` and `docs/roadmap.md`.
   - Shared review/desktop UI and adapters:
     `docs/adr/0024-single-client-ui-source.md`.
   - Identity, invitation, and first join:
     `docs/adr/0010-user-and-device-key-separation.md`,
     `docs/adr/0015-secure-invitation-bootstrap.md`, and
     `docs/adr/0022-friends-alpha-desktop-and-server-bootstrap.md`.
   - Home Server behavior:
     `docs/adr/0017-replaceable-community-home-server.md` and
     `docs/adr/0018-minimal-community-home-server.md`.
   - Private feedback: `docs/adr/0023-private-friends-alpha-feedback.md`.
   - Voice: `docs/adr/0025-authorized-community-voice.md`.
3. Before editing, state the requested behavior, root cause or main constraint,
   affected modules, smallest consistent solution, and compatibility or
   security risks.
4. If the requested behavior conflicts with an accepted ADR, stop and present
   the conflict. Do not silently redefine the architecture in code.

## Preserve the Layers

Use the established dependency direction:

```text
React components and workflows
        ↓
ClientAdapter product-shaped commands and snapshots
        ↓
Tauri command boundary
        ↓
Rust validation, projection, storage, and transport
```

- Treat React state as presentation and pending user intent, never as proof of
  identity, membership, authority, event validity, or synchronization.
- Let React consume product-shaped validated snapshots. Do not parse protocol
  events, query SQLite, or accept raw network state in components.
- Keep `apps/client-ui/src/adapters/demo.ts` bounded and deterministic. Demo
  behavior must never imply that a feature is implemented by Rust.
- Keep review-only scripts and credentials out of the desktop build.
- Keep secrets and message content out of process arguments and logs. Preserve
  the existing bounded serialized Rust command boundary.
- When adding a Tauri command, plugin, native API, window, or WebView, consult
  the current official Tauri documentation, validate inputs in Rust, and grant
  only the capability required by that surface. Do not treat generated
  capability schemas as the policy source.
- Treat the local validated event store as what this client currently knows.
  Signed events and deterministic profile rules remain the source of protocol
  validity.

## Apply the Current Product Model

- Use the replaceable Community Home Server as the normal text and history
  path.
- Use direct P2P and Relay assistance only where the accepted milestone or ADR
  defines them. Do not restore the abandoned assumption that all normal chat
  traffic must be pure P2P.
- Keep cached validated state readable during network failure. Bound connection
  attempts, preserve safe local work, and show honest degraded states for
  operations that cannot complete.
- Model edits, deletion requests, membership, moderation, and server changes as
  new events when the protocol supports them. Never mutate signed history in
  the UI.
- Do not add Realm federation, MLS, attachments, video, screen sharing,
  recovery, or automatic updates unless the active milestone explicitly asks
  for them.

## Implement the Smallest Coherent Change

1. Place domain values and contracts below workflows and adapters.
2. Give one hook, reducer, adapter, or Rust module one primary lifecycle or
   reason to change.
3. Treat roughly 200 lines as a cohesion review signal, not a file-size rule.
   Split only at a natural independently testable boundary.
4. Preserve room-keyed drafts, IME composition, stale-response protection,
   bounded retries, and single-flight synchronization when touching adjacent
   behavior.
5. Add both Simplified Chinese and English for every user-visible string. Show
   one selected language at a time and extend localization tests.
6. Keep feedback and first-use screens usable in short windows and mobile-like
   review widths.

## Verify by Affected Layer

Run the narrowest sufficient checks first:

```sh
cd apps/client-ui
npm test
npm run typecheck
npm run build
```

For review-surface changes, also run:

```sh
cd apps/client-ui
npm run build:review
cd ../review-prototype
python3 -m unittest -v test_server.py test_frontend.py
```

For Rust or Tauri boundary changes, run format, build, clippy, and tests in an
environment with adequate storage. Do not silently create a multi-gigabyte
local `target` directory on the maintainer's Mac. Use temporary or remote build
storage, or one explicitly authorized GitHub Actions run when platform
validation is necessary.

Before a UI release, verify the primary path at the minimum supported window
size and the default size with a real browser or packaged desktop client. Do
not claim visual or hardware validation that was not performed.

## Finish

Self-review for trust-boundary violations, stale async writes, duplicated
ownership, unbounded input or retry paths, untranslated copy, unrelated
changes, and simpler alternatives. Report changed behavior, files, checks,
compatibility impact, and anything not verified.
