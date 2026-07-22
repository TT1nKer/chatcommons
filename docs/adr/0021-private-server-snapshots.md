# ADR 0021: Private server snapshots

Status: accepted; diagnostic implementation complete

## Context

Community Archive v1 preserves signed community events but intentionally omits
the Home Server device seed. Restoring only that archive creates a different
server identity, which clients reject until the community owner signs a new
Home Server declaration. Operational recovery of the same declared server needs
both the event history and the server identity.

A cloud-specific backup API, a new encrypted container or a background scheduler
would add policy and dependencies before the private deployment has monitoring,
retention requirements or an agreed key-management system.

## Decision

The Linux deployment supplies two root-only operator scripts. A snapshot is a
versioned directory containing:

```text
manifest
SHA256SUMS
identity.json
community.ccarchive
service.env
```

`chatcommons-backup` validates the instance and its Community ID. If its systemd
unit is active, the script stops it so the event export, identity and runtime
configuration describe one operational point. It always attempts to restart a
service that it stopped, including after a failed backup. Files are staged in a
private partial directory and renamed only after export and checksums complete.

`chatcommons-restore` accepts only bounded regular files, an exact three-entry
checksum list, snapshot version 1 and matching manifest/environment Community
IDs. It validates the identity with the existing parser and the community archive
with the existing parse-then-validate import path. Restoration occurs in a
private partial state directory and refuses to overwrite any existing instance
state or environment. Starting the restored service is explicit unless the
operator supplies `--start`.

Checksums detect accidental corruption but are not signatures: an attacker able
to replace a snapshot can also replace its checksum list. Protocol event
signatures remain the authority for community history. The server identity is
needed to retain the declared Peer ID and is therefore secret backup material.

The snapshot format has no cloud provider, upload transport, encryption,
retention, deletion or schedule. Operators must place it on an encrypted and
access-controlled target. The original and restored servers must never run at
the same time because they share one device identity.

## Consequences

A private operator can now recover the same declared Home Server after machine
loss without asking the community owner to change Community ID or sign a server
migration. Recovery also revalidates every archived event before rebuilding
SQLite rather than copying database pages blindly.

Backup requires a short service interruption. Snapshot confidentiality and
off-host durability remain operator responsibilities. There is no incremental
backup, attachment coverage, automatic rotation, remote durability check,
recovery-point objective, recovery-time objective, KMS integration or scheduled
restore drill. Those require concrete operating and retention decisions rather
than protocol defaults.
