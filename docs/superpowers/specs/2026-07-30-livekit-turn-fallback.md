# LiveKit TURN Fallback Design

## Goal

Improve friends-alpha voice reachability on VPN, campus, enterprise, and
restrictive NAT networks without replacing LiveKit or changing the
ChatCommons voice authorization model.

## Current state

The Community Home Server validates membership and issues short-lived LiveKit
room grants. LiveKit currently exposes ICE/UDP on ports `50000-50020` and
ICE/TCP on port `7881`. The deployed configuration has no TURN fallback.

## Approved design

Keep the existing signaling and media architecture. Add the two missing
LiveKit fallback transports:

- TURN/UDP on UDP port `443`;
- TURN/TLS on TCP port `5349`.
- TURN relay allocations on bounded UDP ports `40000-40100`.

Use the dedicated DNS-only hostname `turn.ttinker.net`. TCP port `443` remains
owned by nginx, so this milestone deliberately does not attempt TURN/TLS on
TCP `443`. If testing later proves that strict HTTPS-only networks are common,
that requires a separate public IP or an explicitly designed layer-4 TLS
dispatcher.

The LiveKit service continues to run as the unprivileged `livekit` user.
Certbot's private-key directory remains root-only. A root-operated deployment
hook validates the certificate and key, copies them into
`/etc/livekit/tls` as `root:livekit` mode `0640`, then restarts LiveKit only
when the service is already running. The systemd unit grants only
`CAP_NET_BIND_SERVICE`, which is necessary for the unprivileged process to bind
UDP `443`.

## Security and operational boundaries

- The TURN hostname must be DNS-only; Cloudflare's ordinary HTTP proxy is not
  part of the media path.
- TURN uses LiveKit's integrated authentication. No static TURN credential is
  published to clients.
- LiveKit receives no capability other than `CAP_NET_BIND_SERVICE`.
- Certificate private keys must not enter the repository, logs, command
  output, or world-readable paths.
- Existing ICE/UDP and ICE/TCP paths remain available.
- Rollback restores the previous LiveKit YAML and closes UDP `443` and TCP
  `5349`; text messaging is unaffected.
- Alibaba Cloud firewall and UFW must both allow the new ports.

## Verification

Deployment is accepted only when:

1. `turn.ttinker.net` resolves directly to `47.254.94.170`;
2. Certbot has a valid certificate for that exact hostname;
3. LiveKit is active after restart with no new restart loop;
4. the server listens on UDP `443` and TCP `5349`, and advertises the bounded
   UDP relay range `40000-40100`;
5. TURN/TLS presents the `turn.ttinker.net` certificate;
6. the existing HTTPS site, WSS signaling, ICE/TCP, and ICE/UDP listeners
   remain available;
7. a packaged client can join voice from a normal network and from at least
   one network where the direct UDP media range is unavailable.
