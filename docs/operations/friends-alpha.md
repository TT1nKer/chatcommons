# Friends-alpha operating record

## Test community

- Display name: `chatcommonsTestCommunity`
- Community ID: `37473c23cd5c005c688ed872428049e308627b6d48d98547f309cf6d8c61080e`
- Declared endpoint: `/ip4/47.254.94.170/udp/4001/quic-v1`
- Server instance: `chatcommons-home-server@chatcommonsTestCommunity.service`
- Server state: `/var/lib/chatcommons/chatcommonsTestCommunity`
- Owner state: `$HOME/.local/share/chatcommons/alpha/chatcommonsTestCommunity-owner`
- Logical event quota: 512 MiB

The owner state contains the community governance identity and must never be
copied to the Home Server or committed. The server state contains a separate
infrastructure device identity. Initial server snapshots are stored under the
root-only `/var/backups/chatcommons` directory; this is same-host recovery, not
an off-host durability guarantee.

## Network

The process listens on UDP 4001 and the host UFW rule allows that port. The
Alibaba Cloud security-group inbound rule must also allow UDP destination port
4001 before an external QUIC handshake can complete. Keep TCP ports closed; the
current endpoint is QUIC only.

This is a capability-gated friends test, not a public service launch. Stop the
instance and remove the firewall/security-group rule when the test is not being
operated:

```sh
ssh aliyun systemctl stop chatcommons-home-server@chatcommonsTestCommunity.service
```

## Validation

After the cloud rule is active, run from a separate network:

```sh
chatcommons-node sync-home-server \
  --state "$HOME/.local/share/chatcommons/alpha/chatcommonsTestCommunity-owner" \
  --community 37473c23cd5c005c688ed872428049e308627b6d48d98547f309cf6d8c61080e \
  --listen /ip4/0.0.0.0/udp/0/quic-v1 \
  --idle-timeout-ms 1500
```

Success prints `AUTHENTICATED`, one or more sync progress lines and
`SYNC_COMPLETE`. A handshake timeout while the service and UFW are healthy
indicates the cloud security group is still blocking UDP 4001.

The alpha.9 desktop client also handles a common local failure mode: a VPN or
TUN adapter may capture the default route while silently dropping QUIC. The
client first tries one active, non-virtual RFC1918 IPv4 interface and then
falls back once to the wildcard route. When diagnosing an older client, compare
the default route with a physical-interface-scoped route before changing the
server firewall. A successful scoped sync proves that the server is reachable
and the local virtual route is the failing boundary.

Alpha.10 applies the same bounded path selection to voice authorization. It
also keeps the Home Server listener alive when one incoming QUIC handshake is
rejected. A rising systemd restart count therefore indicates a regression or a
different process-level failure, not an expected response to untrusted traffic.

## Voice

The invited alpha uses a separate LiveKit process for audio media. The Home
Server only validates membership and issues five-minute room grants; it does
not forward microphone traffic. The deployed limits are:

- 10 participants per room;
- TCP 7881 as WebRTC fallback;
- UDP 50000–50020 for WebRTC media;
- UDP 443 as authenticated TURN/UDP fallback;
- TCP 5349 as authenticated TURN/TLS fallback;
- UDP 40000–40100 for bounded TURN relay allocations;
- HTTPS/WSS signaling through `wss://ttinker.net` and nginx `/rtc`;
- no video, recording or media E2EE.

The cloud security group and UFW must allow TCP 7881, TCP 5349, UDP 443, UDP
40000–40100, and UDP 50000–50020. TCP 7880 remains host-local behind nginx.
The DNS-only `turn.ttinker.net` record points directly to the LiveKit host;
Cloudflare's ordinary HTTP proxy must not sit in the TURN path. The voice API
key and secret exist in both `/etc/livekit/livekit.yaml` and the root-only Home
Server environment. They must never be copied to clients, logs, archives or
this repository.

The unprivileged LiveKit service reads its copied TURN certificate from
`/etc/livekit/tls`, not from Certbot's root-only directory. Install
`deploy/bin/chatcommons-refresh-livekit-turn-cert` as a Certbot deploy hook so
renewals validate and atomically replace that copy. TCP 443 remains assigned to
nginx; therefore this alpha does not claim compatibility with networks that
permit only TURN/TLS on TCP 443. The service has only
`CAP_NET_BIND_SERVICE`, allowing it to bind TURN/UDP on UDP 443 without running
as root.
The friends-alpha signaling endpoint temporarily shares the existing
`ttinker.net` TLS origin. Nginx proxies only `/rtc` to LiveKit using
`deploy/nginx/chatcommons-livekit-rtc.conf`; TURN uses its separate DNS-only
hostname. A later move to a dedicated signaling hostname changes only the Home
Server URL and reverse-proxy endpoint.
The snapshot script removes all three voice variables; after a restore, issue a
new media-service key before re-enabling voice.

## Latency diagnosis

The `938574d` desktop candidate established the current baseline against the
Silicon Valley Home Server. ICMP round-trip time was 163 ms from macOS and 136
ms from Windows with no measured loss. The latest 100 macOS synchronization
attempts had a 2746 ms median and 3793 ms p95; 21 Windows attempts had a 3060
ms median and 3679 ms p95. Operation-lock acquisition was normally zero. The
per-attempt sidecar and QUIC setup plus the fixed 1.5-second idle-completion
wait are therefore the primary known text-latency cost. See
[ADR 0027](../adr/0027-persistent-client-session.md); do not compensate by
increasing polling frequency.

Latency tracing is off by default. Start each test client with
`CHATCOMMONS_LATENCY_TRACE=1`, send one uniquely identifiable test message in
each direction, then join the same voice room. The client writes a bounded
`latency-trace.jsonl` file inside its local `ChatCommonsAlpha/node` data
directory. The file rotates at 1 MiB and contains only phase names, timestamps,
elapsed durations and a 16-character event marker. It does not contain message
bodies, identity keys, invite codes, participant tokens or full event IDs.

Example launch commands:

```sh
# macOS: run the executable inside the installed application bundle.
CHATCOMMONS_LATENCY_TRACE=1 \
  "/Applications/ChatCommons Alpha.app/Contents/MacOS/chatcommons-desktop"
```

```powershell
# Windows PowerShell: adjust the installed path when necessary.
$env:CHATCOMMONS_LATENCY_TRACE = "1"
& "$env:LOCALAPPDATA\ChatCommons Alpha\chatcommons-desktop.exe"
```

Copy both trace files to one trusted machine and compare them locally:

```sh
python3 tools/latency_report.py \
  diagnostics/mac/latency-trace.jsonl \
  diagnostics/windows/latency-trace.jsonl
```

The report matches text delivery by the event marker and breaks sync and voice
join attempts into stages. Synchronize both operating-system clocks before the
test: cross-device delivery is calculated from wall-clock timestamps, while
the stage durations within one device use its monotonic clock. Do not publish
raw diagnostic files even though their contents are deliberately bounded.

If voice must be disabled without affecting text, stop
`chatcommons-livekit.service`, remove the three voice issuer variables from the
Home Server environment and restart the Home Server. Existing text state is
unchanged.
