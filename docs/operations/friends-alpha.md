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

## Voice

The invited alpha uses a separate LiveKit process for audio media. The Home
Server only validates membership and issues five-minute room grants; it does
not forward microphone traffic. The deployed limits are:

- 10 participants per room;
- TCP 7881 as WebRTC fallback;
- UDP 50000–50020 for WebRTC media;
- HTTPS/WSS signaling through `wss://ttinker.net` and nginx `/rtc`;
- no video, recording or media E2EE.

The cloud security group and UFW must allow TCP 7881 and UDP 50000–50020.
TCP 7880 remains host-local behind nginx. The voice API key and secret exist in
both `/etc/livekit/livekit.yaml` and the root-only Home Server environment.
They must never be copied to clients, logs, archives or this repository.
The friends-alpha deployment temporarily shares the existing `ttinker.net` TLS
origin to avoid a second DNS dependency. Nginx proxies only `/rtc` to LiveKit,
using `deploy/nginx/chatcommons-livekit-rtc.conf`. A later move to a dedicated
voice hostname changes only the Home Server URL and reverse-proxy endpoint.
The snapshot script removes all three voice variables; after a restore, issue a
new media-service key before re-enabling voice.

If voice must be disabled without affecting text, stop
`chatcommons-livekit.service`, remove the three voice issuer variables from the
Home Server environment and restart the Home Server. Existing text state is
unchanged.
