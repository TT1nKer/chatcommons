# ChatCommons UI review prototype

This is an intentionally isolated browser prototype for validating product
navigation before the Tauri desktop client exists. It is not a ChatCommons node
and does not read protocol identities or community databases.

`ChatCommons` is the user-facing product name. The prototype may be hosted
under a parent website namespace without inheriting that website's brand.

The prototype demonstrates:

- an activity-first “Now” screen instead of a server rail;
- community cards and a focused conversation surface;
- top-level room tabs with an on-demand room browser;
- an on-demand member drawer;
- simulated create, join, invite, search, customization and message flows;
- a Chinese/English toggle that persists locally and can later move into Settings;
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
and removed from the visible URL. Reviewers can use “复制审阅链接” to reconstruct
the authorized entry URL before sharing it with another invited reviewer. Do
not publish either credential.

## Security and operations

- The public prototype has no review controls without a valid reviewer token.
- Reviewer and owner credentials are independent and compared in constant time.
- Each submitted item receives a separate edit capability. Only its SHA-256
  digest is stored server-side; the capability stays in the submitting browser.
- A reviewer can edit their own item while it is pending and can withdraw it
  before it reaches a terminal owner state. Withdrawal is an audited soft state,
  not physical database deletion.
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
the isolated systemd unit. The reference deployment nests the application under
the parent ttinker website at `/chatcommons/`; it does not own the site root.

## ttinker deployment

The current review build is served at `https://ttinker.net/chatcommons/`.
Deployment secrets are generated on the host and kept only in
`/etc/ttinker-chatcommons-review.env`; they must never be committed.

- `deploy/nginx-ttinker-site.conf` is the complete initial HTTPS virtual host.
- `deploy/nginx-chatcommons-location.conf` contains only the locations needed
  when the parent ttinker site later replaces the placeholder root.
- `deploy/ttinker-chatcommons-review.service` keeps the API on loopback port
  8091, behind Nginx.
- Let's Encrypt uses the webroot `/var/www/ttinker/acme`, so renewal does not
  require stopping Nginx. Verify changes with `certbot renew --dry-run`.

Only the static prototype and its lightweight feedback API are operated here.
The hosted-service boundary, including the exclusion of video and bulk relay
traffic, is recorded in `docs/governance/control-boundaries.md`.
