# ChatCommons desktop friends alpha

This is the first real desktop test client. It intentionally exposes only the
smallest complete workflow: create one local identity, join one community from
a single-use invitation, list signed channels and messages, send a signed text
message, and synchronize with the community's declared Home Server.

The desktop executable and `chatcommons-node` must be installed beside each
other. `CHATCOMMONS_NODE_PATH` may override the sidecar location for local
development. The UI never parses or trusts remote messages itself; the sidecar
persists and validates the protocol DAG before returning accepted profile data.

This alpha has no account recovery, multi-device identity, automatic updates,
attachments, voice, notifications, or production key-management guarantee.
Windows state inherits the current user's application-data ACL. Use only a test
identity and test community.

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
