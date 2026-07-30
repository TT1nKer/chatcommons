# Private Home Server deployment

The supplied systemd unit runs one ChatCommons community per isolated state
directory. It is a private diagnostic deployment baseline, not approval to
operate a public service. It does not configure DNS, nginx, a firewall, a cloud
security group, a relay, monitoring, off-host backup storage or log retention.
The unit permits `AF_NETLINK` only because libp2p uses `NETLINK_ROUTE` to
enumerate local interfaces before publishing its QUIC listen addresses; the
service still receives no Linux capabilities.

The optional `nginx/chatcommons-livekit-rtc.conf` fragment terminates WSS for
the LiveKit signaling endpoint. Include it inside the TLS virtual host named by
`CHATCOMMONS_VOICE_SERVER_URL`; WebRTC media ports remain directly exposed as
described in `docs/operations/friends-alpha.md`.

## LiveKit TURN fallback

The example LiveKit configuration keeps direct ICE/UDP and ICE/TCP enabled and
adds authenticated TURN/UDP on UDP `443` plus TURN/TLS on TCP `5349`. Relayed
media uses the bounded UDP range `40000-40100`; the default unbounded range is
unnecessary for the ten-participant alpha. Create a DNS-only `A` record from
`turn.ttinker.net` to the LiveKit host. Do not enable Cloudflare's HTTP proxy
for this record because TURN is not HTTP traffic.

The existing nginx TLS virtual host keeps TCP `443`. TCP `5349` is an
intentional friends-alpha compromise: strict networks that allow only
HTTPS/TCP `443` may still require a second public IP or a separately reviewed
layer-4 TLS dispatcher.

Request a dedicated certificate through the existing ACME webroot:

```sh
certbot certonly \
  --webroot \
  --webroot-path /var/www/ttinker/acme \
  --cert-name turn.ttinker.net \
  --domain turn.ttinker.net \
  --non-interactive \
  --agree-tos
```

Do not grant the `livekit` user access to Certbot's private-key directory.
Install the certificate deployment hook instead:

```sh
install -o root -g root -m 0755 \
  deploy/bin/chatcommons-refresh-livekit-turn-cert \
  /opt/chatcommons/bin/chatcommons-refresh-livekit-turn-cert
/opt/chatcommons/bin/chatcommons-refresh-livekit-turn-cert
ln -sfn \
  /opt/chatcommons/bin/chatcommons-refresh-livekit-turn-cert \
  /etc/letsencrypt/renewal-hooks/deploy/chatcommons-livekit-turn-cert
```

The hook accepts only a valid certificate for `turn.ttinker.net`, verifies that
its private key matches, and copies the pair to `/etc/livekit/tls` as
`root:livekit` mode `0640`. It ignores renewal events for other certificates
and restarts LiveKit only when the copied pair changed.

Both the host firewall and cloud firewall must allow UDP `443`, TCP `5349`, and
UDP `40000-40100`.
The supplied systemd unit grants the unprivileged LiveKit process only
`CAP_NET_BIND_SERVICE`, which is required to bind UDP `443`; it does not grant
network-administration or file-access capabilities. After updating
`/etc/livekit/livekit.yaml`, restart and verify the service:

```sh
ufw allow 443/udp comment 'ChatCommons LiveKit TURN UDP'
ufw allow 5349/tcp comment 'ChatCommons LiveKit TURN TLS'
ufw allow 40000:40100/udp comment 'ChatCommons LiveKit TURN relay'
systemctl restart chatcommons-livekit.service
systemctl show chatcommons-livekit.service \
  -p ActiveState -p SubState -p NRestarts
ss -lntup | grep -E ':(443|5349|7880|7881)([[:space:]]|$)'
openssl s_client \
  -connect turn.ttinker.net:5349 \
  -servername turn.ttinker.net \
  -verify_return_error </dev/null
```

To roll back TURN without affecting text, restore the previous root-only copy
of `/etc/livekit/livekit.yaml`, restart `chatcommons-livekit.service`, and
remove only the two new firewall rules. The existing WSS, ICE/UDP, and ICE/TCP
paths remain unchanged.

## Install the process

Build `chatcommons-node` for the target Linux host, then install it and the unit:

```sh
install -o root -g root -m 0755 chatcommons-node /opt/chatcommons/bin/chatcommons-node
install -o root -g root -m 0644 \
  deploy/systemd/chatcommons-home-server@.service \
  /etc/systemd/system/chatcommons-home-server@.service
install -o root -g root -m 0755 \
  deploy/bin/chatcommons-backup /opt/chatcommons/bin/chatcommons-backup
install -o root -g root -m 0755 \
  deploy/bin/chatcommons-restore /opt/chatcommons/bin/chatcommons-restore
useradd --system --home-dir /var/lib/chatcommons --shell /usr/sbin/nologin chatcommons
install -o root -g chatcommons -m 0750 -d /etc/chatcommons /var/lib/chatcommons
systemctl daemon-reload
```

Create and provision a named instance while it is stopped. The example uses
`friends`; replace it consistently:

```sh
install -o chatcommons -g chatcommons -m 0700 -d /var/lib/chatcommons/friends
sudo -u chatcommons /opt/chatcommons/bin/chatcommons-node init \
  --state /var/lib/chatcommons/friends
sudo -u chatcommons /opt/chatcommons/bin/chatcommons-node import-community \
  --state /var/lib/chatcommons/friends \
  --input /path/to/community.ccarchive
install -o root -g root -m 0600 \
  deploy/systemd/chatcommons-home-server.env.example \
  /etc/chatcommons/friends.env
```

Edit the environment file with the imported Community ID and the endpoint
already declared by the community owner. Start and inspect the instance:

```sh
systemctl enable --now chatcommons-home-server@friends
systemctl status chatcommons-home-server@friends
journalctl -u chatcommons-home-server@friends
```

The state lock makes a second ChatCommons process fail instead of sharing the
same directory. Stop the unit before export, import or manual maintenance:

```sh
systemctl stop chatcommons-home-server@friends
```

## Consistent private snapshots

Create a root-only snapshot directory on an encrypted or otherwise protected
volume:

```sh
install -o root -g root -m 0700 -d /var/backups/chatcommons
/opt/chatcommons/bin/chatcommons-backup friends /var/backups/chatcommons
```

If the instance is running, the script stops it, exports a parent-closed event
archive, copies its server identity and service configuration, then restarts it.
The completed snapshot appears atomically and contains:

```text
manifest
SHA256SUMS
identity.json
community.ccarchive
service.env
```

Voice endpoint and LiveKit issuer credentials are intentionally excluded from
`service.env`. A restored Home Server starts with voice disabled until the
operator provisions a new media-service key and all three voice variables.

The checksums detect accidental corruption; they are stored beside the data and
do not authenticate a snapshot supplied by an attacker. Event signatures and
archive validation still protect the community history, while identity parsing
protects the server key structure.

Restore refuses to overwrite an existing state or environment. Restore into a
new instance name, or use the original name only after the old state has been
lost or moved out of the way:

```sh
/opt/chatcommons/bin/chatcommons-restore \
  /var/backups/chatcommons/friends-<timestamp>-<id> \
  friends-restored

# Start only after confirming the old instance is stopped.
systemctl start chatcommons-home-server@friends-restored
```

`--start` may be added to the restore command when the endpoint is known to be
free. A snapshot preserves the server identity because clients authenticate the
device declared by the community. Never run the original and restored instances
simultaneously: they would share one cryptographic device identity and normally
the same declared endpoint.

Snapshots contain the plaintext server identity seed and plaintext signed event
history. Directory and file modes are `0700` and `0600`, but permissions are not
encryption. Copy snapshots only to an encrypted, access-controlled target and
apply an operator-defined retention policy. The scripts do not upload, delete or
rotate snapshots automatically.

`CHATCOMMONS_MAX_STORE_BYTES` limits the sum of JSON event bodies that
the sync layer will retain, including unresolved events. It is not a filesystem
quota: SQLite pages, WAL files, identity state and logs add overhead. Use an OS or
volume quota as an additional hard boundary when public operation is authorized.

The example listens on loopback. Changing it to `0.0.0.0` makes the UDP socket
reachable only if both the host firewall and cloud security group allow that
port. Do not expose this milestone publicly: per-peer persistent rate limits,
monitoring, scheduled off-host backups and the operated-service governance gate
are still missing.
