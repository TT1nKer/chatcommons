# ChatCommons friends-alpha desktop client

The friend-facing desktop application is a Tauri host for the shared React
client in `apps/client-ui`. Browser review and packaged desktop builds therefore
render the same product components. The former eframe shell is retained as the
separate `chatcommons-diagnostic` binary for internal protocol checks only; see
ADR 0024.

The current alpha intentionally exposes only the smallest complete workflow:
create one local test identity, join one community from a single-use invitation,
list validated signed channels and messages, send signed text messages, and
synchronize with each community's declared Home Server.

Message sending is local-first: a signed message is persisted and displayed
before the client attempts its bounded Home Server synchronization. A temporarily
unreachable server therefore reports degraded synchronization without blocking
the local conversation.

The desktop executable and `chatcommons-node` must be installed beside each
other. `CHATCOMMONS_NODE_PATH` may override the sidecar location for local
development. The webview does not parse or trust remote messages itself. Tauri
commands serialize operations, call the sidecar, and return only the accepted
projected state after the sidecar has persisted and validated the protocol DAG.

This alpha has no account recovery, multi-device identity, automatic updates,
attachments, voice, notifications, or production key-management guarantee.
Windows state inherits the current user's application-data ACL. Use only a test
identity and test community.

## Private feedback

The friends alpha includes a private feedback form rather than redirecting
testers to GitHub. The visible text fields do not impose a product character
limit and the form body scrolls independently from its fixed confirmation and
submission controls.

The app generates a diagnostic preview before submission. It excludes chat
messages, invitations, identity keys, full identity/community identifiers and
local paths. A tester may separately capture the current app window; the dialog
is hidden during capture and the screenshot is attached only after explicit
confirmation. The server still enforces operational safety bounds: a 2 MB
request cap, a 1 MB decoded screenshot cap and per-address rate limiting.

The returned private edit capability is stored in the user's application data
directory so the app can later retrieve the owner status and reply. It must not
be logged, displayed as a public identifier or embedded in a shared report.

## Local development

Build both executables into the same target directory, then launch the desktop
binary:

```sh
npm ci --prefix apps/client-ui
npm run build --prefix apps/client-ui
cargo build -p chatcommons-cli --bin chatcommons-node -p chatcommons-desktop
CHATCOMMONS_NODE_PATH="$PWD/target/debug/chatcommons-node" \
  cargo run -p chatcommons-desktop --bin chatcommons-desktop
```

Tagged builds are packaged by `.github/workflows/friends-alpha.yml` as a macOS
arm64 application zip and a Windows x64 zip. They are intentionally unsigned
friends-alpha artifacts, so operating-system trust warnings are expected.

Run the retained protocol diagnostic shell only when investigating the Rust
workflow directly:

```sh
cargo run -p chatcommons-desktop --bin chatcommons-diagnostic
```
