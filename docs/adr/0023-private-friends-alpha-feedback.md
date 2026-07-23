# ADR 0023: Private friends-alpha feedback

Status: accepted; implemented for the invited friends alpha

## Context

The first desktop testers are people known directly by the project owner. Asking
them to create GitHub accounts or translate an application problem into a public
issue would add friction and may expose screenshots or diagnostics. The existing
web annotation service already has a private owner inbox, screenshot storage,
status workflow and owner replies.

The alpha also needs to accept long, multiline reports. A visible character
limit caused real feedback to become impossible to submit. At the same time,
untrusted network input still needs finite resource bounds.

## Decision

The desktop alpha submits feedback to a dedicated `POST /api/app-feedback`
endpoint backed by the existing private review database and owner inbox. This
endpoint does not require the web reviewer token. That is an explicit,
replaceable assumption for the direct friends-only distribution phase; it is
not an anonymous public feedback policy for a production release.

The form has no product-level character counter or user-facing length limit.
Text and screenshot data share a 2 MB HTTP request limit, screenshots are
decoded and validated as images, and the decoded image is capped at 1 MB. The
service also applies a bounded per-address submission rate. These are safety
limits, not editorial constraints.

Before sending, the app renders the complete report that will be submitted.
Automatically generated diagnostics exclude chat messages, invitation codes,
identity keys, full user and community identifiers, and local paths. Capturing
the current app window is a separate user action. The feedback dialog is hidden
during capture, the resulting preview is shown locally, and submission requires
an explicit confirmation covering both the text and optional screenshot.

Successful submission returns a public receipt identifier and a separate
high-entropy edit capability. Only the capability digest is stored server-side.
The client stores the capability in its private application-data file and uses
it to retrieve status and owner replies. The capability is never placed in a
shareable URL or diagnostic report.

The website download link is hidden until the current browser session has
successfully authenticated with the reviewer token. This makes the intended
friends-alpha path clear without pretending to make the public GitHub release a
private binary distribution system.

## Consequences

Friends can send long reports and screenshots without a GitHub account, while
the project owner receives them in one private workflow. A compromised or
widely distributed desktop build could still submit spam to this endpoint. Rate
limits and request bounds contain resource use but do not authenticate people.

Before broad public distribution, the project must choose a replacement such as
short-lived per-build submission capabilities, abuse-resistant public feedback,
or an account-backed support channel. That decision is intentionally deferred;
the alpha does not create a general feedback identity system prematurely.
