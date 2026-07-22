# ADR 0020: Private Home Server runtime boundaries

Status: accepted; diagnostic implementation complete

## Context

M3c can provision and start a Community Home Server, but two processes could use
one state directory and authenticated peers could grow its SQLite database until
the host ran out of space. There was also no reproducible least-privilege service
definition for a Linux test host.

The current Home Server is still below the repository's public operated-service
gate. This milestone must make private testing safer without implying that a
single quota or systemd unit constitutes production readiness.

## Decision

Every CLI operation that mutates, exports or continuously serves a node state
holds one advisory exclusive lock at `<state>/state.lock`. The file is private on
Unix. A conflicting ChatCommons process fails closed; `info` remains a read-only
diagnostic and does not take the lock. The state directory is already private to
one OS user, so symlink rejection plus an advisory lock is sufficient for this
diagnostic boundary. It is not a hostile same-user sandbox.

`SyncPeer` may enforce a maximum number of stored event-body bytes. The projected
usage includes SQLite event bodies already stored, unresolved pending events and
new unique events in the current message. An over-quota batch is rejected before
it enters the pending set or database. `serve-community` enables this limit and
defaults to 512 MiB; operators may set a positive value with
`--max-store-bytes`.

The accounting unit is the JSON byte representation persisted in the `events`
table. It deliberately excludes SQLite page/WAL overhead, the identity file,
logs, archives and future attachments. Therefore it is an application resource
limit, not a filesystem quota.

A systemd template runs each community as an unprivileged `chatcommons` user,
grants write access only to that instance's state directory, removes Linux
capabilities, applies a memory/task/file-descriptor budget and restarts on
failure. Its example endpoint is loopback-only. Firewall, cloud security group,
DNS and reverse-proxy changes are outside the template.

## Consequences

An accidental second server, export or import cannot concurrently operate on the
same state through the reference CLI. A peer cannot exceed the configured logical
event storage budget by accumulating either immediately ingestible or unresolved
events. Existing state above a configured limit prevents the server from
starting until the operator raises the limit or restores a smaller valid state.

The lock is advisory, not a database encryption or authorization mechanism.
Disk exhaustion remains possible through SQLite overhead, logs, manual files or
other system users unless the operator also configures volume-level limits.
There are still no per-user request rates, retention jobs, metrics, alerting,
automated backups, attachment accounting or public-service incident controls.
The Home Server remains suitable only for isolated private testing.
