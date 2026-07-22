# Private Home Server deployment

The supplied systemd unit runs one ChatCommons community per isolated state
directory. It is a private diagnostic deployment baseline, not approval to
operate a public service. It does not configure DNS, nginx, a firewall, a cloud
security group, a relay, monitoring, backups or log retention.

## Install the process

Build `chatcommons-node` for the target Linux host, then install it and the unit:

```sh
install -o root -g root -m 0755 chatcommons-node /opt/chatcommons/bin/chatcommons-node
install -o root -g root -m 0644 \
  deploy/systemd/chatcommons-home-server@.service \
  /etc/systemd/system/chatcommons-home-server@.service
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

`CHATCOMMONS_MAX_STORE_BYTES` limits the sum of JSON event bodies that
the sync layer will retain, including unresolved events. It is not a filesystem
quota: SQLite pages, WAL files, identity state and logs add overhead. Use an OS or
volume quota as an additional hard boundary when public operation is authorized.

The example listens on loopback. Changing it to `0.0.0.0` makes the UDP socket
reachable only if both the host firewall and cloud security group allow that
port. Do not expose this milestone publicly: per-peer persistent rate limits,
monitoring, backup automation and the operated-service governance gate are still
missing.
