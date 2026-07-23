# Manual UI and positioning feedback — 2026-07-22

Status: Triaged

Source: feedback relayed through chat screenshots and an annotated review sheet.
The remote review inbox contained no submitted records at the time of triage.
Reviewer names and unrelated conversation content are intentionally omitted.

## 1. The project is not explained before the prototype

- Current: the shared URL opens directly into a simulated chat home. Reviewers
  infer that it is only a Discord clone hosted on a personal computer.
- Requested: state the mission, the actual server model, current implementation
  status, and the difference from a platform-owned community.
- Acceptance: the first viewport answers what ChatCommons is, why it exists,
  how a Home Server fits, what is real today, and what remains a prototype.
  Link previews also carry a useful English description.
- Ownership: product copy maintained in the shared localization resources.

## 2. Visual hierarchy and legibility

- Current: the brand mark feels vertically misaligned; mentions resemble
  ordinary activity; the connection badge is both faint and misleading; the
  featured community card contains a large decorative empty region.
- Requested: align the brand, make direct mentions visibly higher priority,
  identify community names more clearly, remove the ambiguous connection badge,
  and use space more evenly.
- Acceptance: mentions have a distinct accent and community name; all community
  cards use comparable density; prototype/network status is explicit and does
  not claim a real connection.
- Ownership: fixed interaction and presentation rules.

## 3. Community and room navigation needs a growth path

- Current: three room tabs and three community cards work only at the sample
  scale. “All rooms” does not explain what opens, and activity versus community
  navigation is ambiguous.
- Requested: search/filter entry points and clearer separation between attention
  items and places the user can enter.
- Acceptance: the home labels attention items separately from communities;
  “Find and filter” and “Browse rooms” open a searchable panel whose results can
  navigate to the selected sample community or room.
- Ownership: interaction capability; eventual categories remain user or
  community-managed rather than hard-coded protocol concepts.

## 4. Invitation lifetime is a product decision

- Current: every invite is a single-person, single-use bearer capability.
- Requested: optionally create a persistent or multi-use invite similar to
  established community chat products.
- Acceptance: not implemented in this batch. A future decision must define
  maximum uses, expiry, revocation, role/channel scope, abuse limits, and whether
  the community can disable reusable invites.
- Ownership: community policy exposed through owner controls and enforced by the
  chat profile; not a visual-only toggle.

## 5. Feedback must work outside the annotation overlay

- Current: some reviewers will not open an unfamiliar link, and others may use
  an embedded browser where the annotation toolbar loads slowly.
- Requested: make the project understandable from a shared preview or pasted
  explanation, while continuing to accept screenshots and relayed messages.
- Acceptance: the page exposes a bilingual “copy project brief” action; Open
  Graph copy explains the mission without requiring a visit; manual feedback is
  recorded in this triage format instead of being discarded.
- Ownership: product communication plus the existing review workflow.

## Recognition and close-out rule

Every implemented annotation receives a specific thank-you and implementation
reply before it enters `client_review`. The owner inbox provides a bilingual
“thank and move to review” action, and the API rejects terminal/review states
with an empty reply. Reviewers are invited to join the public contributors list,
but their name or account is published only after they provide the preferred
credit and consent.
