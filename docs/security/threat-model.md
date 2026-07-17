# ChatCommons security and abuse threat model

Status: baseline for pre-network design

## Assets to protect

- identity keys and device authorization;
- private message content and relationship metadata;
- community membership, roles and moderation state;
- availability of community history and official infrastructure;
- users' ability to leave a provider without losing identity or data;
- trustworthy evidence when a user voluntarily reports abuse.

## Trust boundaries

Everything received from another device, node, community administrator or report
submitter is untrusted. A valid signature establishes which key produced an
event; it does not establish that the person is honest, that the content is legal,
or that a report contains complete context.

ChatCommons distinguishes five actors:

1. protocol maintainers define interoperable rules but do not control the network;
2. official client maintainers control defaults and releases;
3. official service operators control only their relays, mailboxes, directories,
   push gateways and media nodes;
4. third-party node operators decide what their own resources store or forward;
5. community administrators govern membership and visibility in their community.

## Priority abuse cases

| Threat | Likely mechanism | Maturity-zero mitigation |
|---|---|---|
| spam and bulk recruitment | cheap keys, invite automation, unsolicited messages | invite-only groups and single-use bearer invites; expiry, default DM blocking and service rate limits remain launch gates |
| fraud and illegal trade | private groups, disposable identities, payment links | no public directory, recommendation feed, embedded payment or marketplace in early releases |
| malicious attachments | executable or deceptive files | defer attachments; later add strict quotas, type warnings and client-side scanning hooks |
| harassment and network violence | repeated contact, brigading, identity rotation | no public discovery initially; personal block, community ban, invite revocation and report workflows remain launch gates |
| child-safety harm | grooming, sexual material, disclosure of minors' information | no stranger discovery, conservative defaults, dedicated escalation procedure before public launch |
| denial of service | oversized events, parent bombs, signature workload, storage exhaustion | hard size/count/depth budgets before networking, staged validation, per-peer quotas |
| malicious administrators | selective history, abusive bans, false reports | signed moderation events, exportable history, report context, appeal records |
| compromised keys | stolen device authorizes messages or moderation | M3 root/device key separation, device revocation and recovery |
| report forgery | screenshots or edited plaintext | signed report bundle tied to original signed event; human review and appeal |
| metadata surveillance | relay observes social graph and timing | minimize retained logs, separate services, short retention, avoid central relationship index |

## Product constraints for the first release

The first public product is private, invitation-only small-group chat. It will not
include public group discovery, trending lists, broadcast channels, stranger bulk
DMs, integrated payments, anonymous permanent file hosting or large public groups.

Official infrastructure uses explicit quotas and may refuse service independently
of protocol validity. Refusal by an official service must not revoke a protocol
identity or prevent migration to another provider.

## Security invariants

- no global moderation backdoor or universal operator key;
- no claim that replicated plaintext can be physically deleted everywhere;
- no silent acceptance of unsigned moderation evidence;
- no use of timestamps or arrival order as sole authorization conflict resolution;
- no network feature before bounded parsing and deterministic state convergence;
- no public launch before an incident owner, report path and escalation runbook exist.
