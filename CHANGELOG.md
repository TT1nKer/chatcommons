# Changelog

All notable product changes are recorded here. ChatCommons uses Semantic
Versioning for product releases while it is practical to do so. Product version
and protocol compatibility are separate; see `docs/versioning.md`.

## [Unreleased]

## [0.1.0-alpha.9] - 2026-07-29

### Fixed

- Desktop Home Server synchronization now prefers one bounded physical private
  IPv4 path before the wildcard route. This prevents a system-wide VPN or TUN
  adapter from silently black-holing QUIC when the ordinary Wi-Fi or Ethernet
  path can reach the community server.
- A failed scoped path falls back once to the existing wildcard listener, so
  machines without a suitable private interface keep the previous behavior.

## [0.1.0-alpha.8] - 2026-07-28

### Added

- Community owners and administrators can create, publish and privately copy
  one-person invitations from the shared desktop client.

### Changed

- The shared client now uses conversation-first spacing, larger readable type,
  centered message and composer widths, and a quieter current-community room
  hierarchy.
- macOS and Windows downloads can advance independently in the release
  manifest, avoiding false claims that both platform artifacts are current.

## [0.1.0-alpha.7] - 2026-07-28

### Added

- A local microphone selector beside the voice action, with the system default
  preserved as the zero-configuration choice.
- Remembered per-device microphone preference and automatic refresh when audio
  input devices are connected or removed.

### Changed

- The chosen microphone is now used consistently for both local preflight and
  LiveKit capture. A removed saved device falls back to the system default.
- Device labels that remain hidden before the first permission grant use
  localized numbered placeholders and refresh after a successful preflight.

## [0.1.0-alpha.6] - 2026-07-28

### Added

- A local microphone preflight before voice authorization, with the detected
  input name and a short live input-level meter.
- Separate Chinese and English recovery guidance for denied macOS and Windows
  microphone permissions, missing devices, busy devices and unsupported
  clients.

### Changed

- Voice authorization is requested only after the local microphone check
  succeeds. Temporary preview tracks and Web Audio resources are released
  before the LiveKit session starts or immediately when the user cancels.

## [0.1.0-alpha.5] - 2026-07-27

### Added

- Authorized room voice for up to ten invited members through a replaceable
  self-hosted LiveKit SFU.
- Five-minute microphone-only media grants issued only after the Community Home
  Server authenticates the device and revalidates membership and channel state.
- Desktop join, reconnecting, participant, mute, leave and microphone-error
  states with Chinese and English copy.

### Security

- Media API secrets remain server-only and voice grants are bounded, validated,
  short-lived and redacted from debug output.
- Community snapshots now exclude voice issuer credentials and restore with
  voice disabled until an operator provisions a new key.

### Changed

- The browser and text client load the media SDK only when voice is requested,
  keeping voice code out of the initial application bundle.
- Linux Home Server builds move to the test server rather than consuming local
  desktop storage.

### Limitations

- Voice media is not end-to-end encrypted in this alpha. The community-operated
  SFU can observe media and connection metadata.
- Video, recording, broadcast and browser-review voice remain unavailable.

## [0.1.0-alpha.4] - 2026-07-27

### Added

- A single-source desktop interface shared with the reviewed web client design.
- Automatic two-second Home Server synchronization after joining, with a
  single-flight guard that prevents overlapping network operations.
- Private in-app feedback with long-form reports, optional screenshots and
  receipt-based owner replies.

### Changed

- Joining a community now opens its first text room immediately.
- Message sending remains local-first and triggers a background synchronization
  without blocking the composer.
- Desktop runtime operations use bounded subprocess and feedback request
  timeouts, while failed synchronization preserves the validated local history.

### Fixed

- Prevented Enter from sending while an input method is still composing text.
- Preserved per-room drafts and room selection across delayed synchronization.
- Added scrollable join and feedback dialogs for short desktop windows.

## [0.1.0-alpha.3] - 2026-07-23

### Changed

- Moved the invited-alpha desktop download entry into a prominent bilingual
  banner while keeping access limited to authorized review links.

### Fixed

- Made the desktop invitation screen vertically scrollable so the join button
  remains reachable in short windows and under enlarged display scaling.
- Kept the desktop status line synchronized with Chinese/English language
  changes, including empty-invitation validation.

## [0.1.0-alpha.2] - 2026-07-23

### Added

- Native Chinese/English friends-alpha desktop client for macOS and Windows.
- Signed channel and text-message commands backed by the existing protocol core
  and SQLite event archive.
- Home Server invitation bootstrap, so a new member can join while the
  community owner is offline.
- Reproducible macOS, Windows, and Linux Home Server release artifacts.
- A permanent `chatcommonsTestCommunity` deployment record and operating guide.
- A redesigned native alpha interface with invitation, community chat and
  bilingual views aligned with the web prototype.
- Private in-app feedback with user-reviewed diagnostics, optional explicit
  app-window screenshots, private receipts and owner replies.
- Collapsible web annotation controls and review-session-only download entry
  points.

### Changed

- Signed messages are now saved and rendered locally before bounded Home Server
  synchronization, so an unreachable server no longer blocks immediate local
  feedback.
- The desktop feedback form is scrollable, has no product-level text-length
  limit and does not require a GitHub account.

### Validated

- Friend invitation, membership acceptance, signed message synchronization,
  duplicate handling, offline branch preservation, and SQLite reopen behavior.
- Public Home Server process startup, service sandboxing, same-host backup, and
  host firewall configuration. External QUIC access still requires the cloud
  security-group rule documented in the operating guide.

### Limitations

- Test identities do not yet have recovery or multi-device authorization.
- The native bundles are not notarized or signed by a trusted publisher.
- The desktop client intentionally supports one identity and one community.
- Review-session-only download visibility does not make the otherwise public
  GitHub release artifact private.

## [0.1.0-alpha.1] - 2026-07-22

### Added

- Canonical signed events, deterministic IDs, DAG validation, and SQLite
  persistence.
- Reference chat profile with membership, single-use invitations, permissions,
  revocation, and replaceable Home Server declarations.
- Direct QUIC synchronization, relay-assisted fallback experiments, durable
  Home Server storage, and backup/restore tooling.
- Bilingual interactive UI prototype with a private annotation workflow.
- Product mission, implementation status, shareable project brief, and
  searchable sample community/room navigation.
- Reviewer-facing acknowledgement and opt-in contributor recognition in the
  private annotation toolbar.

### Security

- Bounded untrusted input, authenticated device transport, storage quotas,
  least-privilege service configuration, and private review credentials.

### Limitations

- The webpage is not connected to the protocol core.
- There is no distributable desktop client or permanent public community yet.
- Voice, video, screen sharing, MLS, attachments, and production account
  recovery are not implemented.

[Unreleased]: https://github.com/TT1nKer/chatcommons/compare/v0.1.0-alpha.4...HEAD
[0.1.0-alpha.4]: https://github.com/TT1nKer/chatcommons/compare/v0.1.0-alpha.3...v0.1.0-alpha.4
[0.1.0-alpha.3]: https://github.com/TT1nKer/chatcommons/compare/v0.1.0-alpha.2...v0.1.0-alpha.3
[0.1.0-alpha.2]: https://github.com/TT1nKer/chatcommons/compare/v0.1.0-alpha.1...v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/TT1nKer/chatcommons/releases/tag/v0.1.0-alpha.1
