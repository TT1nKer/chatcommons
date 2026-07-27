# ChatCommons project instructions

These instructions apply to every agent and every file in this repository.

## Bilingual product requirement

- Every user-visible string must be available in both Simplified Chinese and
  English. A feature is incomplete if either language is missing.
- Never add or change UI copy, onboarding, buttons, validation, errors, status
  messages, feedback text, download text, metadata, or accessibility labels
  without updating both languages in the same change.
- Show exactly one selected language at a time unless the user explicitly asks
  for simultaneous bilingual copy. Every visible string on the page, including
  first-use guidance and download controls, must follow the same language
  control.
- Critical first-use paths such as downloading, joining by invitation,
  reporting a problem, and recovering from an error must remain understandable
  before a user discovers the language toggle. Keep the language control obvious
  and accessible instead of showing both languages at once.
- Changing the language must update already-visible and persisted status text;
  it is not sufficient to translate only newly rendered content.
- Every UI change must extend localization tests to prove that both language
  variants exist and are reachable.

## Review workflow

- An authorized review link must show the Annotate toolbar expanded on every
  page load. Collapsing it is a current-page convenience and must not hide it
  from a later reviewer or a later review session.
- Feedback controls must remain reachable in short windows and mobile
  viewports. Any form or first-use screen taller than its viewport must scroll.
- Static CSS and JavaScript entry points must carry one shared deployment
  revision in their URLs. Change that revision whenever any referenced asset
  changes so a browser can never combine new HTML with week-old cached assets.

## Release checks

- Before publishing a UI release, verify the primary first-use flow at the
  minimum supported window size as well as the default size.
- Use GitHub Actions sparingly. Run ordinary tests and checks locally; do not
  trigger hosted CI for small edits or repeated experimentation. Use it only
  for an intentional distributable macOS/Windows candidate, when the maintainer
  explicitly requests a build, or when a platform check cannot reasonably be
  performed locally.

## Engineering quality

- Before editing, read the relevant call path, data structures and tests, then
  briefly state the understood requirement, root cause, affected modules,
  smallest architecture-consistent approach, and compatibility or security
  risks.
- Prefer correct, direct, intention-revealing code over speculative
  extensibility. Do not add unrequested features, framework abstractions,
  patterns, dependencies, unrelated refactors or formatting.
- Treat files, network data, environment variables and client input as
  untrusted. Preserve trusted authorization boundaries and never expose secrets
  or personal data in code, logs or errors.
- Add behavior-focused tests for defects and important boundaries. Run relevant
  local tests, type checks, builds and static checks; state anything that could
  not be verified.
- Before finishing, self-review for root-cause correctness, simpler possible
  solutions, dead or duplicate code, races and failure paths, compatibility
  with APIs/config/data/callers, test coverage, and unrelated changes.
