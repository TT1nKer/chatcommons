# ChatCommons project instructions

These instructions apply to every agent and every file in this repository.

## Bilingual product requirement

- Every user-visible string must be available in both Simplified Chinese and
  English. A feature is incomplete if either language is missing.
- Never add or change UI copy, onboarding, buttons, validation, errors, status
  messages, feedback text, download text, metadata, or accessibility labels
  without updating both languages in the same change.
- Critical first-use paths such as downloading, joining by invitation,
  reporting a problem, and recovering from an error must remain understandable
  before a user discovers the language toggle. Prefer explicit bilingual copy
  for these entry points.
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
- Prefer GitHub Actions for distributable macOS and Windows builds so local
  build artifacts do not consume the maintainer's disk.
