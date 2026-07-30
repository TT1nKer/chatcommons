# LiveKit TURN Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add authenticated TURN/UDP and TURN/TLS fallback to the existing self-hosted LiveKit friends-alpha deployment.

**Architecture:** Preserve the Community Home Server authorization boundary and LiveKit SFU. Extend only the media reachability layer, using `turn.ttinker.net`, UDP `443`, TCP `5349`, bounded UDP relay ports `40000-40100`, and a root-owned certificate deployment hook that gives the unprivileged LiveKit process only the copied key material it needs.

**Tech Stack:** LiveKit Server 1.13.4, systemd, Certbot, nginx ACME webroot, UFW, Alibaba Cloud firewall, POSIX shell.

## Global Constraints

- Do not replace LiveKit or change the signed community membership/token flow.
- Do not move nginx away from TCP port `443`.
- Do not expose API secrets, private keys, participant tokens, or message data.
- Do not trigger GitHub Actions.
- Keep text messaging available throughout the voice deployment.

---

### Task 1: Repository deployment contract

**Files:**
- Create: `deploy/bin/chatcommons-refresh-livekit-turn-cert`
- Modify: `deploy/livekit.example.yaml`
- Modify: `deploy/systemd/chatcommons-livekit.service`
- Modify: `deploy/README.md`
- Modify: `docs/operations/friends-alpha.md`

**Interfaces:**
- Consumes: Certbot certificate directory `/etc/letsencrypt/live/turn.ttinker.net`.
- Produces: `/etc/livekit/tls/fullchain.pem`, `/etc/livekit/tls/privkey.pem`, and a LiveKit TURN configuration using those paths.

- [x] **Step 1: Add the certificate deployment hook**

Create a POSIX shell script that requires root, ignores Certbot deploy events
for other lineages, validates certificate expiration and hostname, verifies
that the certificate and private key public keys match, stages both files as
`root:livekit` mode `0640`, atomically moves them into `/etc/livekit/tls`, and
restarts `chatcommons-livekit.service` only when the copied certificate changed
and the service is already active.

- [x] **Step 2: Add TURN to the example configuration**

Add:

```yaml
turn:
  enabled: true
  domain: turn.ttinker.net
  udp_port: 443
  tls_port: 5349
  relay_range_start: 40000
  relay_range_end: 40100
  cert_file: /etc/livekit/tls/fullchain.pem
  key_file: /etc/livekit/tls/privkey.pem
```

- [x] **Step 3: Document provisioning and rollback**

Document the DNS-only record, Certbot webroot command, deploy hook, UFW and
cloud firewall ports including UDP `40000-40100`, listener checks, TLS
certificate check, and rollback.
Grant the unprivileged LiveKit service only `CAP_NET_BIND_SERVICE` so it can
bind UDP `443`.

- [x] **Step 4: Run focused repository checks**

Run:

```sh
sh -n deploy/bin/chatcommons-refresh-livekit-turn-cert
git diff --check
rg -n "turn.ttinker.net|udp_port: 443|tls_port: 5349" \
  deploy docs/operations
```

Expected: shell parsing succeeds, no whitespace errors exist, and every
operational layer names the same hostname and ports.

### Task 2: DNS and certificate provisioning

**Files:**
- Server certificate source: `/etc/letsencrypt/live/turn.ttinker.net`
- Server certificate target: `/etc/livekit/tls`

**Interfaces:**
- Consumes: DNS-only `A turn.ttinker.net -> 47.254.94.170`.
- Produces: a valid certificate and LiveKit-readable copied certificate pair.

- [x] **Step 1: Verify DNS points directly to the server**

Run:

```sh
dig +short A turn.ttinker.net
```

Expected: exactly `47.254.94.170`.

- [x] **Step 2: Request the dedicated certificate**

Run on the server:

```sh
sudo certbot certonly \
  --webroot \
  --webroot-path /var/www/ttinker/acme \
  --cert-name turn.ttinker.net \
  --domain turn.ttinker.net \
  --non-interactive
```

Expected: `/etc/letsencrypt/live/turn.ttinker.net/fullchain.pem` and
`privkey.pem` exist and Certbot reports success.

- [x] **Step 3: Install and execute the renewal hook**

Run:

```sh
sudo install -o root -g root -m 0755 \
  deploy/bin/chatcommons-refresh-livekit-turn-cert \
  /opt/chatcommons/bin/chatcommons-refresh-livekit-turn-cert
sudo /opt/chatcommons/bin/chatcommons-refresh-livekit-turn-cert
sudo ln -sfn \
  /opt/chatcommons/bin/chatcommons-refresh-livekit-turn-cert \
  /etc/letsencrypt/renewal-hooks/deploy/chatcommons-livekit-turn-cert
sudo certbot renew --cert-name turn.ttinker.net --dry-run --run-deploy-hooks
```

Expected: copied files are `root:livekit` mode `0640`; the renewal dry run
completes and invokes the hook without exposing key material.

### Task 3: TURN activation and network verification

**Files:**
- Server configuration: `/etc/livekit/livekit.yaml`
- Server firewall: UFW and Alibaba Cloud firewall

**Interfaces:**
- Consumes: certificate target from Task 2.
- Produces: authenticated TURN/UDP on UDP `443` and TURN/TLS on TCP `5349`.

- [x] **Step 1: Back up and update LiveKit configuration**

Copy `/etc/livekit/livekit.yaml` to a root-only timestamped backup, add the
exact `turn` block from Task 1, and preserve the existing API key unchanged.

- [x] **Step 2: Open only the required host firewall ports**

Run:

```sh
sudo ufw allow 443/udp comment 'ChatCommons LiveKit TURN UDP'
sudo ufw allow 5349/tcp comment 'ChatCommons LiveKit TURN TLS'
sudo ufw allow 40000:40100/udp comment 'ChatCommons LiveKit TURN relay'
```

Add matching inbound rules to the Alibaba Cloud firewall before restarting
LiveKit.

- [x] **Step 3: Restart with bounded rollback**

Restart `chatcommons-livekit.service`, verify `ActiveState=active`, and inspect
the latest journal. If it does not remain active, restore the root-only backup
immediately and restart the previous configuration.

- [x] **Step 4: Verify listeners and certificate**

Run:

```sh
sudo ss -lntup | grep -E ':(443|5349|7880|7881)([[:space:]]|$)'
openssl s_client \
  -connect turn.ttinker.net:5349 \
  -servername turn.ttinker.net \
  -verify_return_error </dev/null
curl --fail --silent --show-error https://ttinker.net/ >/dev/null
```

Expected: LiveKit listens on UDP `443` and TCP `5349`, the TLS certificate
verifies for `turn.ttinker.net`, existing LiveKit TCP listeners remain, and
the website still responds.

- [ ] **Step 5: Verify real client behavior**

Join the same voice room from two packaged clients on a normal network. Repeat
with one client on a network where direct UDP `50000-50020` is blocked. Both
clients must join, hear each other, mute, unmute, leave, and rejoin without
changing community membership.

Automated deployment verification forced WebRTC to use relay-only ICE and
connected successfully through TURN/UDP, selecting relay port `40037` inside
the configured range. The packaged two-client microphone check remains a
manual friends-alpha acceptance item.

### Task 4: Review and commit

**Files:**
- Review every file changed by Tasks 1-3.

**Interfaces:**
- Consumes: repository checks and deployment evidence.
- Produces: one reviewable infrastructure commit.

- [x] **Step 1: Review the diff and deployed state**

Check for secret material, inconsistent ports, unsafe permissions, unbounded
restart behavior, accidental nginx changes, and undocumented rollback steps.

- [x] **Step 2: Commit repository changes**

Run:

```sh
git add \
  deploy/bin/chatcommons-refresh-livekit-turn-cert \
  deploy/livekit.example.yaml \
  deploy/systemd/chatcommons-livekit.service \
  deploy/README.md \
  docs/operations/friends-alpha.md \
  docs/superpowers/specs/2026-07-30-livekit-turn-fallback.md \
  docs/superpowers/plans/2026-07-30-livekit-turn-fallback.md
git commit -m "ops: add LiveKit TURN fallback"
```

Expected: the commit contains deployment code and documentation only, with no
certificate, key, token, generated artifact, or unrelated file.
