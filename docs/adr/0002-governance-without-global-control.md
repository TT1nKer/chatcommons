# ADR 0002: Governance without global protocol control

Status: accepted as a pre-network baseline

## Context

An open, self-hostable chat protocol can be used for ordinary private communities
and for harmful activity. Eliminating every central service is neither practical
nor sufficient to remove operator obligations. Adding a global moderation key
would instead create a single point of control and compromise.

## Decision

Moderation and resource control are layered:

- users control personal blocks;
- community administrators control signed community membership and bans;
- node operators control their own storage and bandwidth;
- official operators control access to official services, directories and paid
  resources under published policy and appeal procedures;
- protocol maintainers provide interoperable primitives but no global account or
  content revocation authority.

The initial product is invitation-only and omits public discovery and monetized
user-to-user trade. A verifiable report bundle will be designed before public
community discovery, but routine content escrow or a universal decryption key is
not accepted.

## Consequences

Harm cannot be removed from every independently operated node, and official
service suspension is not equivalent to protocol-wide erasure. Official services
still need legal review, identity/risk controls, incident response and evidence
handling. The client must explain these limits honestly.
