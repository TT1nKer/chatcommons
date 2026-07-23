# Pre-network and pre-launch engineering gates

These gates convert the governance baseline into testable work. They do not
replace legal review.

## Gate A — before accepting network events

- maximum encoded event size, string length, parent count and nesting are fixed;
- parsing rejects oversized input before proportional allocation where practical;
- signature verification and database work have per-peer budgets;
- the same event set converges under randomized arrival order;
- concurrent authorization conflicts have published deterministic rules;
- malformed, replayed, cyclic and missing-parent event corpora are tested;
- canonical encoding vectors can be consumed outside Rust.

## Gate B — before operating a relay or offline mailbox

The repository's ephemeral loopback diagnostic relay does not satisfy this gate
and must not be exposed as a public service.

- service data-flow and metadata inventory are reviewed;
- ciphertext size, recipients, retention and rate quotas are enforced;
- expiry and deletion jobs have observable success/failure metrics;
- abuse suspension affects only that service credential, not protocol identity;
- incident logs are minimized, access-controlled and covered by retention policy;
- operator contact, reporting, escalation and appeal paths exist;
- qualified counsel has classified the concrete deployed service.

## Gate C — before public discovery or public communities

- community listing is curated and removable from the official directory;
- stranger contact and bulk invitations are restricted by default;
- signed community bans, personal blocks and invite revocation are implemented;
- report bundles preserve provenance and minimum necessary context;
- serious-harm escalation, evidence access and appeal procedures are rehearsed;
- minor-safety and public-content obligations receive separate legal review;
- transparency metrics do not expose victims or private social graphs.

## Gate D — before attachments, payments or voice

Each capability receives a separate threat model, resource budget, acceptable-use
policy and legal review. None is treated as a harmless extension of text chat.
