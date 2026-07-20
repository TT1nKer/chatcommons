# kaiyuan UI review prototype

This is an intentionally isolated browser prototype for validating product
navigation before the Tauri desktop client exists. It is not a kaiyuan node
and does not read protocol identities or community databases.

`kaiyuan` is the current user-facing product name. Existing repository, Rust
crate and deployment identifiers intentionally remain unchanged until a separate
protocol naming decision records compatibility and migration consequences.

The prototype demonstrates:

- an activity-first “Now” screen instead of a server rail;
- community cards and a focused conversation surface;
- top-level room tabs with an on-demand room browser;
- an on-demand member drawer;
- simulated create, join, invite, search, customization and message flows;
- an authenticated click-to-annotate review overlay;
- an independently authenticated owner inbox with replies and workflow states.

## Local review

Generate two independent high-entropy credentials and export the configuration:

```sh
export REVIEW_TOKEN="$(openssl rand -hex 32)"
export OWNER_TOKEN="$(openssl rand -hex 32)"
export REVIEW_ALLOWED_ORIGIN="http://127.0.0.1:8091"
export REVIEW_STATIC_ROOT="$PWD/apps/review-prototype/public"
export REVIEW_DB_PATH="$(mktemp -d)/reviews.sqlite3"
export REVIEW_SCREENSHOT_DIR="${REVIEW_DB_PATH%.sqlite3}-screenshots"
python3 apps/review-prototype/server.py
```

Open the reviewer URL once with `?review=<REVIEW_TOKEN>` and the owner inbox
with `/admin.html?owner=<OWNER_TOKEN>`. Each token is captured in session storage
and removed from the visible URL. Do not publish either credential.

## Security and operations

- The public prototype has no review controls without a valid reviewer token.
- Reviewer and owner credentials are independent and compared in constant time.
- Rotating `REVIEW_TOKEN` revokes all previous reviewer links after restart.
- SQLite and screenshots live outside the static directory.
- Screenshots are optional, validated by MIME prefix and magic bytes, capped at
  1 MB decoded, and available only through the owner API.
- Review submission is rate-limited and capped at 2 MB at both application and
  reverse-proxy layers.
- Reviewer text and target data are stored as inert values and rendered through
  DOM text nodes rather than HTML interpolation.
- Owner replies and status changes are recorded in `audit_log`.

The production service is for invited design review only. It does not implement
accounts, password recovery, multiple review projects, backups, or public access.
The service can be rolled back by repointing its `current` symlink and restarting
the isolated systemd unit; the existing business backend is not involved.
