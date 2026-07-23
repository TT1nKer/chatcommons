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
