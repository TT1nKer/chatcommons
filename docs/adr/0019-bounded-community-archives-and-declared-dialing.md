# ADR 0019: Bounded community archives and declared Home Server dialing

Status: accepted; diagnostic implementation complete

## Context

The first Home Server could be provisioned by temporarily synchronizing it with
an online member, but this made backup, migration, and repeatable deployment
awkward. Clients also had to repeat a Peer ID and address that were already
derivable from the signed `HomeServerSet` event.

The missing mechanism is a portable operational archive, not a new source of
protocol authority. Individual events already carry their authorship and Event
IDs, so signing an outer backup container would add another key and ambiguous
ownership semantics.

## Decision

`chatcommons-storage` defines Community Archive version 1 as a deterministic JSON
container with:

```text
version
community_id
events[] sorted by Event ID
```

The container is limited to 64 MiB and 65,536 events. Validation requires a
strictly sorted unique event list, exactly one genesis matching Community ID,
valid Core signatures and Event IDs, correct community membership for every
event, and every referenced parent in the same archive. Parsing is separate from
validation. Imports do not write any event until the complete container passes
these checks.

“Complete” here means parent-closed: every included event has all of its ancestry.
Without a separately witnessed checkpoint, the format cannot prove that an
exporter did not omit an independent leaf or branch it had never learned.

Export order does not depend on SQLite row or network arrival order. Import is
idempotent for a database containing the same community and refuses a database
containing another community. Large valid archives are ingested in bounded
topological batches rather than raising the existing single-ingest limit.

The archive contains community events only. It never contains the exporting
user's identity seed, a server device seed, an invite bearer secret outside an
event, attachments, or runtime configuration. The outer file is not encrypted
or signed: authenticity comes from each event, while confidentiality remains the
operator's responsibility. The Unix diagnostic CLI creates new export files with
mode `0600` and refuses to overwrite an existing path.

`sync-home-server` reads the current accepted `HomeServerSet`, validates its
Ed25519 device key, derives the libp2p Peer ID, selects the first endpoint that is
a supported Multiaddr, and starts the existing authenticated QUIC sync. It does
not consult an official directory or silently substitute an official server.
Endpoint URLs that are not Multiaddrs remain signed hints for future clients and
are skipped by this diagnostic command.

## Consequences

An operator can now export a community on one machine, transfer the archive,
import it idempotently into a separately initialized Home Server identity, and
start `serve-community`. Members no longer type or trust an independent Peer ID:
the connection target and accepted device originate from the same owner-signed
declaration.

This is not a complete backup system. The archive size is intentionally bounded,
exports are full snapshots rather than incremental streams, attachments are
absent, files are plaintext at rest, and there is no schedule, retention policy,
compression, encryption, remote object store, checkpoint signature, or restore
monitoring. Those remain production milestones.
