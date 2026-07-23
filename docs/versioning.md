# Versioning policy

ChatCommons versions four different things independently. A number shown in the
application must never be used as a substitute for protocol compatibility.

## Product releases

Product releases follow Semantic Versioning:

```text
MAJOR.MINOR.PATCH-PRERELEASE
0.1.0-alpha.1
```

- `0.x`: the product has not promised a stable public API or user-data
  migration path.
- `alpha.n`: friends and contributors can test it, but normal users should not
  rely on availability or compatibility.
- `beta.n`: the primary workflows are usable and migration testing has begun.
- `rc.n`: a candidate for the first stable release.
- `1.0.0`: the first documented stable compatibility and migration promise.

During `0.x`, breaking product changes are allowed but must be recorded in the
changelog. Once real user identities or permanent communities exist, breaking
data changes require a migration path even before `1.0.0`.

## Protocol versions

Core canonical bytes and the reference chat profile have their own versions.
The current implementation uses core protocol `v2` and chat profile
`chatcommons.chat.v2`. A UI release does not change either value. Any change to
signed canonical bytes, verification rules, or wire compatibility requires an
explicit protocol transition, test vectors, and an ADR.

## Storage schema versions

SQLite and portable archive schemas require explicit migrations when their
shape changes. They must not infer compatibility from the Cargo package version.
The migration mechanism will be added before the first change that cannot be
opened by the existing schema.

## Deployment revisions

Server directories use immutable date-and-purpose names such as:

```text
20260722-product-brief-feedback-v1
```

These are operational rollback identifiers, not product releases. A deployment
records the product version and Git commit when release automation is added.

## Release checklist

1. Decide the next product version and update `VERSION`.
2. Keep `[workspace.package].version`, the prototype label, and
   `public/version.json` equal to `VERSION`.
3. Move relevant entries from `Unreleased` into a dated changelog section.
4. Run format, build, clippy, Rust tests, and frontend tests.
5. Review compatibility impact separately for protocol and storage.
6. Commit the release and create the matching signed Git tag, for example
   `v0.1.0-alpha.1`.
7. Publish immutable artifacts and record checksums.

The repository does not create a release tag merely because files contain a new
version. Tags are created only from a reviewed release commit.
