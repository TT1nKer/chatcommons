# Changelog

All notable product changes are recorded here. ChatCommons uses Semantic
Versioning for product releases while it is practical to do so. Product version
and protocol compatibility are separate; see `docs/versioning.md`.

## [Unreleased]

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

[Unreleased]: https://github.com/TT1nKer/chatcommons/compare/v0.1.0-alpha.2...HEAD
[0.1.0-alpha.2]: https://github.com/TT1nKer/chatcommons/compare/v0.1.0-alpha.1...v0.1.0-alpha.2
[0.1.0-alpha.1]: https://github.com/TT1nKer/chatcommons/releases/tag/v0.1.0-alpha.1
