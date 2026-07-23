# ADR 0024: One client UI source for review and desktop

Status: accepted; migration in progress

## Context

The friends-alpha protocol path was first exposed through a small native eframe
application. In parallel, the project built a browser prototype for product and
UI review. The browser experience received detailed feedback, but the shipped
desktop shell did not use that interface. Review work therefore improved a
different product surface from the one testers downloaded.

The split also duplicated localization, responsive layout, interaction state and
feedback behavior. A visually refined prototype plus a separate functional
desktop shell is not one product.

## Decision

ChatCommons has one user-facing client UI source under `apps/client-ui`.

The same React and TypeScript application is used in two modes:

- review mode uses a bounded demo adapter and may load the authorized Annotate
  overlay;
- desktop mode uses a Tauri adapter that invokes Rust commands and never loads
  review-only scripts.

The public website remains a separate wrapper for the product explanation,
friends-alpha access and download entry. Its “adjust interface” action opens the
shared client UI. Website marketing sections are not rendered inside the
desktop application.

The current CSS and interaction hierarchy are migration inputs, not a prompt to
redesign. Migration proceeds from static visual parity to real data:

1. Now/home, community, room and composer;
2. identity and invite onboarding;
3. validated channel and message snapshots;
4. local-first send and synchronization status;
5. feedback, settings and remaining product actions.

Adapters return product-shaped data. React components do not parse protocol
events, read SQLite or trust network input. Rust continues to validate and
project protocol state before it crosses the desktop bridge.

The eframe application is retained temporarily as an internal protocol
diagnostic harness. It is not the source of product design and must not be
presented as the successor to the reviewed client UI.

## Consequences

- A UI change reviewed in the browser reaches the next desktop build without a
  second implementation.
- Localization, responsive behavior and accessibility have one implementation.
- Review/demo data and real Rust state remain visibly and structurally
  separated.
- The frontend toolchain adds Node, TypeScript, React and Vite to verification.
- Tauri integration and trusted desktop packaging remain a following step; this
  ADR does not claim that the current shared UI already exposes every Rust
  operation.
- The existing eframe artifacts remain useful only for protocol testing during
  migration and will be removed from friend-facing releases once the Tauri path
  reaches feature parity.
