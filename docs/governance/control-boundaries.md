# Governance and control boundaries

This document is a product and engineering boundary, not legal advice.

| Capability | Protocol | Community admin | Node operator | Official service operator |
|---|---:|---:|---:|---:|
| verify signatures and authority | defines rules | uses rules | enforces before storage | enforces before service |
| remove a member from one community | provides signed event | yes | observes valid event | observes valid event |
| block another identity personally | provides local primitive | no | no | no |
| refuse storage, relay or media resources | no | for owned node | yes, locally | yes, on official services |
| erase all replicated copies | impossible to guarantee | no | only its own copy | only its own copy |
| revoke an identity across the network | deliberately absent | no | no | no |
| suspend access to an official account/service | no | no | no | yes, with policy and appeal |
| publish a community in an official directory | no | requests listing | no | yes, curated and removable |
| inspect end-to-end encrypted content by default | no | only content available as member | no | no |
| review voluntarily submitted report evidence | defines verifiable format | may submit | may submit | yes, under retention policy |

## Service tiers

Official services should be separable so that using one does not silently enroll a
user in all of them:

- bootstrap/discovery: returns candidate nodes, owns no community state;
- relay: forwards bounded ciphertext for short periods;
- offline mailbox: stores bounded ciphertext until delivery or expiry;
- push gateway: carries opaque wake-up tokens and minimal metadata;
- directory: optional curated public metadata, never required for private invites;
- media/SFU: an optional provider governed by a separate resource policy.

Each service needs its own data inventory, retention period, abuse controls,
operator identity and shutdown/migration behavior before deployment.

## Report bundle requirements

A future report bundle should be a separate, explicit user action and contain only:

- the original signed event and Event ID;
- the minimum plaintext or decryption material required for review;
- relevant signed membership/moderation context;
- reporter identity or service account reference where legally and operationally
  appropriate;
- a reporter statement, category and submission time;
- an integrity signature over the complete bundle.

A signature proves event provenance, not the truth or completeness of an
allegation. Decisions require context, proportional action, retention limits,
human escalation for serious cases and an appeal path. Report evidence must not
be republished as a global public blocklist by default.
