# Private Home Server deployment

The supplied systemd unit runs one ChatCommons community per isolated state
directory. It is a private diagnostic deployment baseline, not approval to
operate a public service. It does not configure DNS, nginx, a firewall, a cloud
security group, a relay, monitoring, off-host backup storage or log retention.
The unit permits `AF_NETLINK` only because libp2p uses `NETLINK_ROUTE` to
enumerate local interfaces before publishing its QUIC listen addresses; the
service still receives no Linux capabilities.

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
