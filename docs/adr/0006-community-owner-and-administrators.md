# ADR 0006: Community owner and administrators

Status: superseded by ADR 0007; retained as protocol-v1 history

## Context

The genesis-only administrator model cannot safely delegate routine community
management or transfer long-lived community control. Adding roles without clear
authority would expand the concurrent-resolution surface prematurely.

## Decision

Every community has exactly one projected owner. Genesis establishes the first
owner and administrator.

- only the owner may grant or revoke administrator status;
- only active members may become administrators or receive ownership;
- an owner cannot remove the current owner from the administrator set;
- an owner cannot revoke their own membership and must transfer ownership first;
- administrators manage channels, invitations and member removals;
- administrators cannot grant administrators or transfer ownership;
- ownership transfer makes the recipient owner and administrator;
- the former owner remains an administrator until the new owner revokes it.

The new canonical payload tags are appended after the existing v1 tags, so the
published genesis test vector and all existing payload encodings remain stable:

- `6`: `AdministratorGrant`
- `7`: `AdministratorRevoke`
- `8`: `OwnershipTransfer`

Only removals accepted by an initial resolution pass can suppress concurrent
authorization by their target. Resolution repeats until that effective-removal set
reaches a fixed point. Work is bounded by the number of unique candidates plus one;
failure to stabilize rejects resolution rather than looping indefinitely.

## Consequences

Ownership and administrator state are part of deterministic snapshots and survive
SQLite recovery. Concurrent ownership transfers occupying the same author sequence
slot use Event ID ordering. An unauthorized signed removal has no suppressive
effect.

M2 still has no arbitrary roles, permission bitsets or multisignature ownership.
Deleting an owner's local database only removes that node's replica and does not
change community authority retained by other replicas.
