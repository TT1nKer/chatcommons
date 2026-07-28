---
name: release-chatcommons-alpha
description: Prepare, package, deploy, validate, or roll back an intentional ChatCommons friends-alpha desktop release. Use when the maintainer explicitly requests a macOS or Windows distributable, version bump, friends-alpha GitHub Actions run, ttinker.net review-site deployment, artifact verification, or release rollback; never trigger for ordinary code edits or local development builds.
---

# Release ChatCommons Friends Alpha

Produce one traceable macOS/Windows candidate with one hosted build, verify its
contents, and deploy the review/download surface atomically.

## Confirm Release Authority and Scope

1. Require an explicit release, packaging, deployment, or rollback request.
2. Confirm the product version, source branch or commit, requested platforms,
   and destination before changing external state.
3. Read `AGENTS.md`, `docs/versioning.md`,
   `docs/operations/friends-alpha.md`, `.github/workflows/friends-alpha.yml`,
   and the active release notes in `CHANGELOG.md`.
4. Inspect the worktree and preserve unrelated user changes.
5. Treat a signed Git tag and public GitHub prerelease as separate,
   higher-visibility actions. Do not create or push a tag unless the maintainer
   explicitly requests that publication.

## Prepare the Candidate

1. Select the next prerelease version according to `docs/versioning.md`.
2. Use `rg` to find active copies of the previous product version. Update
   `VERSION`, workspace/package metadata and locks, Tauri metadata, review-site
   labels and download links, version JSON, tests, and current documentation
   that intentionally display the active version.
3. Move user-relevant changes from `Unreleased` into one dated changelog
   section. Do not imply unimplemented features.
4. Check protocol and SQLite compatibility independently of the product version.
   A UI version bump never changes signed canonical bytes or storage schema by
   itself.
5. Run frontend tests, type checks, review build, review-service tests, JSON
   validation, and `git diff --check` locally.
6. Do not create a large local Rust `target` directory on the maintainer's Mac.
   Reserve Rust and platform packaging for adequate remote storage or the one
   authorized hosted run.

## Commit and Run Hosted Validation Once

1. Review the complete diff before committing.
2. Commit and push the exact candidate revision intentionally.
3. Dispatch `.github/workflows/friends-alpha.yml` once with the narrowest
   target that produces the requested artifacts. Use `desktop` for macOS and
   Windows together; do not request `all` unless the Linux Home Server artifact
   is also required.
4. Record the run ID and source commit. Wait for the existing run instead of
   starting duplicates.
5. If verification fails, inspect the failing step and logs before deciding
   whether another paid run is justified. Never rerun merely to see whether a
   deterministic failure disappears.

## Verify Artifacts Before Deployment

1. Download artifacts into a new temporary directory outside the repository.
2. Confirm expected filenames and archive structure.
3. Read the `VERSION` file from inside each archive and require an exact match
   with the candidate.
4. Calculate and record SHA-256 for every artifact.
5. Reject artifacts built from another commit, missing binaries, or carrying a
   mismatched version.
6. Keep unsigned/ad-hoc-signed status explicit. Do not call the current packages
   notarized or trusted-publisher signed.

## Deploy the Review and Download Surface

1. Inspect the current remote symlink, relevant systemd units, disk space,
   review database location, and current health before copying files.
2. Create a new immutable date-and-purpose release directory. Never modify the
   active release directory in place.
3. Copy the built review surface, review server entry point, and both verified
   desktop archives into the new directory.
4. Preserve root-only environment files, review credentials, review SQLite
   data, Home Server state, LiveKit configuration, and backups. Never print
   tokens or secrets.
5. Set only the permissions required for the static/review service.
6. Switch the `current` symlink atomically, then restart only the review service.
   Do not restart or replace the Community Home Server or LiveKit for a
   frontend-only release.

## Validate and Roll Back

After switching, verify:

- base page, health endpoint, and shared client return success;
- the page displays the exact product version;
- direct static artifact access remains unavailable;
- unauthenticated download API requests are rejected;
- authorized downloads match the local SHA-256 values;
- review records still exist and their count did not unexpectedly change;
- review service, nginx, Community Home Server, and LiveKit remain active;
- recent review-service logs contain no new warning or error.

If a required check fails:

1. Atomically restore the previous `current` symlink.
2. Restart only the review service when needed.
3. Re-run the same health and data-preservation checks.
4. Keep the failed immutable directory for diagnosis unless the maintainer asks
   to remove it.

Deploying a new Home Server binary, changing firewall rules, rotating media
credentials, publishing a GitHub release, or migrating community state is a
separate operation and requires explicit scope.

## Report

Report the version, commit, workflow run, deployed directory, artifact hashes,
service and endpoint checks, preserved data, rollback state, signing
limitations, hardware checks not performed, and any remaining risk.
