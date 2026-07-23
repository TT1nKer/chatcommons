# ChatCommons desktop friends alpha

This is the first real desktop test client. It intentionally exposes only the
smallest complete workflow: create one local identity, join one community from
a single-use invitation, list signed channels and messages, send a signed text
message, and synchronize with the community's declared Home Server.

Message sending is local-first: a signed message is persisted and displayed
before the client attempts its bounded Home Server synchronization. A temporarily
unreachable server therefore reports degraded synchronization without blocking
the local conversation.

The desktop executable and `chatcommons-node` must be installed beside each
other. `CHATCOMMONS_NODE_PATH` may override the sidecar location for local
development. The UI never parses or trusts remote messages itself; the sidecar
persists and validates the protocol DAG before returning accepted profile data.

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
cargo build -p chatcommons-cli --bin chatcommons-node -p chatcommons-desktop
CHATCOMMONS_NODE_PATH="$PWD/target/debug/chatcommons-node" \
  cargo run -p chatcommons-desktop
```

Tagged builds are packaged by `.github/workflows/friends-alpha.yml` as a macOS
arm64 application zip and a Windows x64 zip. They are intentionally unsigned
friends-alpha artifacts, so operating-system trust warnings are expected.
