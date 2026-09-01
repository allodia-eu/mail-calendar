# App versioning: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client derives its version. One app has **one
version**, and every client shows it, so a bug report, a store listing, a release note
([`changelog.md`](changelog.md)), and the analytics `app_version`
([`analytics.md`](analytics.md)) all name the same release. This is the **single bar** all clients
meet: Apple (macOS, iOS, iPadOS), Windows, Android, Linux, and any future platform (web, …).

**Principle.** There is **one source of truth**: the top-level [`/VERSION`](../VERSION) file. Everything
else is either **derived from it at build time** or a **committed mirror pinned to it by
CI**. No client carries a hand-maintained version literal. Before these were unified the four
numbers had drifted completely apart (Apple `1.0`, Windows `1.0.0.0`, Android `0.2.0`, Cargo
`0.1.0`), which also made the analytics `app_version` meaningless.

## The source of truth · [`/VERSION`](../VERSION)

A single line, `MAJOR.MINOR.PATCH` (semver, no pre-release/build suffix): e.g. `0.2.2`. It is the
**marketing version** everywhere: the number a user sees and quotes. It is trivially readable from
bash, PowerShell, Gradle, and MSBuild, the same "one blessed file" pattern as
[`rust-toolchain.toml`](../rust-toolchain.toml).

**It holds the *last released* version: the number users currently have, not the one being built.**
That is a deliberate reversal of how it started. Binding the bump to each user-facing change made
every PR edit `/VERSION`, `Cargo.toml` and `clients/apple/project.yml`, so two in flight always
conflicted in three files, and it produced twenty version numbers that never reached a store, which
made the file's own meaning unreadable: `main` said `0.14.0` while every user had `0.2.2`. Now a
feature PR touches none of the three, and a version conflict is **structurally impossible**.

## Cutting a release

One command does the whole of it: assemble the pending release notes, then move the version:

```sh
scripts/dev/release.py --dry-run     # see what would be assembled
scripts/dev/release.py               # X.Y.Z computed from the fragments' `Bump:` lines
```

It calls [`bump-version.sh`](../scripts/dev/bump-version.sh), which is still the only thing that
writes `/VERSION` and its two committed mirrors, and still runs the drift guard:

```sh
scripts/dev/bump-version.sh 0.3.0     # by hand, if you are fixing drift rather than releasing
```

It also **regenerates the user documentation's screenshots**, because those are the one user-facing
artefact that rots without saying so. The set is recaptured
*before* anything is written: it builds and photographs a client per platform, so it is the step
most likely to fail for reasons about the host, and everything after it deletes fragments, and it is
published *after* the bump, since a page's `updated_for` may never exceed `/VERSION`. The run then
refuses to end green over pages that still describe the previous release. `--skip-docs` is for a
host that cannot photograph the clients; it prints what is owed, and still fails.

The **next version is derived, never typed**: each pending fragment in
`docs/changelog/unreleased/` declares `Bump: minor` (a capability a user gains) or `Bump: patch`
(a fix), and the release is **minor if any of them says minor, else patch**. The semver judgement is
therefore made while the change is in the author's head, not reconstructed weeks later at release.

Committing and tagging are left to you: a `vX.Y.Z` tag is the input the store packaging builds
take, so you tag when you mean to release. Those builds are not in this repository; what they need
from it is this tag and the tree under it.

**Dev builds carry no suffix.** A build off `main` reports the last released marketing version, and
what distinguishes it is its **build number**, which is derived fresh and uncommitted on every
platform (see the matrix below). There is no `0.2.2-dev`, and that is not an oversight: Windows
reads `Assembly.GetName().Version.ToString(3)`, a numeric `System.Version` that silently **strips**
a pre-release suffix, so `-dev` would make one client disagree with the other three about what
version it is, which is the exact failure this contract exists to prevent.

## Marketing version vs. build number

Two different things, kept distinct on purpose:

- **Marketing version**: the `/VERSION` semver. Human-facing, identical on every platform, and the
  value analytics reports as `app_version`.
- **Build number**: a per-upload identifier some stores require to be **unique and monotonic**
  *within* a marketing version. It is **derived per store** (the stores encode it incompatibly) and
  is **not** committed; the packaging script stamps a fresh one at release. It is not a version a
  user reads.

## Per-platform derivation matrix

| Platform | Marketing version | Build number | Where |
|---|---|---|---|
| **Rust core** | `/VERSION` mirror in `[workspace.package]` | n/a | `Cargo.toml` (committed mirror; CI-pinned) |
| **Apple** (macOS/iOS/iPadOS) | `CFBundleShortVersionString` = `MARKETING_VERSION`, re-stamped from `/VERSION` at release | `CFBundleVersion` = `CURRENT_PROJECT_VERSION` = **dotted UTC timestamp** `date -u +%Y.%m%d.%H%M` | `Scripts/package.sh`; `project.yml` holds a drift-checked `MARKETING_VERSION` mirror for the dev loop |
| **Windows** | assembly `<Version>` read from `/VERSION`; MSIX package version = `<semver>.0` | MSIX **revision fixed at `.0`** (the Store reserves it); the Store treats `MAJOR.MINOR.PATCH` as the sortable version | `Mailcal.csproj` (`<Version>`), `package.ps1` (`-Version` default + manifest stamp) |
| **Android** | `versionName` = `/VERSION` | `versionCode` = `major·10⁷ + minor·10⁵ + patch·10³` (+ a 0–999 build slot, unused) | `app/build.gradle.kts` (fully derived) |
| **Linux** | AppStream `<release version>` = `/VERSION`, with the **date** taken from that release's own assembled note | n/a (Flatpak sorts by the AppStream release list; there is no second number) | `scripts/dev/flatpak_metadata.py`, from `clients/linux/flatpak/*.metainfo.xml.in` (fully derived, generated at build time, never committed) |

### Why each build-number encoding

- **Apple, dotted, never a single integer.** `CFBundleVersion` accepts up to three dot-separated
  integers, each `< 2³²`. A bare `YYYYMMDDHHMM` (e.g. `202607191430`) is one integer that
  **overflows** that limit and App Store Connect rejects it: a live bug before this contract. The
  dotted `2026.0719.1430` keeps every field small and stays monotonic across minutes, days, months,
  and years. `MARKETING_VERSION` in `project.yml` is a **mirror**, not the source: it keeps the dev
  loop (`build-and-run.sh`) honest, and `package.sh` re-stamps it from `/VERSION` at release.
- **Windows, revision `.0`.** The Store rejects any package whose 4th version field is non-zero
  (it reserves that field for its own repackaging), so the Store version is `<semver>.0`. The
  **assembly** version is what `DeviceFacts.cs` reads for `app_version`, so the csproj derives that
  directly from `/VERSION`: the MSIX manifest version and the assembly version are stamped
  separately but from the same source.
- **Android, a formula, so `versionCode` only ever climbs.** Play requires a strictly increasing
  integer `versionCode` and it can **never** decrease. Encoding the semver positionally
  (`0.2.0 → 200000`) guarantees a newer marketing version outranks an older one. It clears the old
  hardcoded `2` and stays far under Play's `2_100_000_000` ceiling (holds for `major ≤ 209`).
- **Linux, no build number at all, and a date that is not "now".** A Flatpak's version *is* its
  AppStream release list, so there is nothing to encode separately. The `date` attribute is the one
  field a generator is tempted to fill with today: don't. It is the date the release was cut, which
  is recorded once, in the heading of `docs/changelog/released/<X.Y.Z>.md`, so rebuilding an old
  tag emits the same metainfo it did the first time, rather than one claiming the release happened
  today. `/VERSION` must have a note there or the build fails, which is the same invariant
  `check-version-sync.sh` enforces from the other side. The diagnostic log separately stamps a
  source fingerprint and build epoch. That identifies a support artifact; it is not a Flatpak
  version and is never published as one.

## Committed mirrors (the only hand-synced values)

Two files cannot read `/VERSION` at build time, so they carry a copy the version-sync check pins:

- `Cargo.toml` `[workspace.package].version`: Cargo has no file-include.
- `clients/apple/project.yml` `MARKETING_VERSION`: consumed by XcodeGen when generating the project
  for the dev loop; `package.sh` overrides it at release anyway, but a stale value would misreport
  the dev build.

Everything else (Android, the Windows assembly version, the Apple release stamp, the Windows Store
package version, the Linux AppStream release) is **derived**, so there is nothing to keep in sync;
the guard instead asserts each still *reads* `/VERSION` and that no literal has crept back.

## Known gaps / follow-ups

- **Same-marketing-version re-uploads need a manual build bump.** Android's `versionCode` is derived
  purely from the semver (the 0–999 build slot is always `0`), so two Play uploads of the *same*
  `MAJOR.MINOR.PATCH` would collide. In practice every store upload rides a version bump; if a
  same-version re-upload is ever needed, bump the patch (or wire the build slot to a CI run number).
  Apple (fresh timestamp per run) and Windows sideload (auto-versioned) don't have this limit.
- **iOS/iPadOS packaging landed (`package.sh --ios-app-store`).** It reuses this same `/VERSION` +
  dotted-`CFBundleVersion` scheme as macOS (the Apple targets share `project.yml`), stamping
  `CFBundleVersion` with `date -u +%Y.%m%d.%H%M` per upload and preserving it through export
  (`manageAppVersionAndBuildNumber = false`). What remains is product, not versioning: creating the
  App Store Connect record and uploading a build (see [`store-listing.md`](store-listing.md)).
- **No git tag is auto-created.** `release.py` prints the `git tag vX.Y.Z` command and
  `bump-version.sh` edits files, but neither commits or tags, on purpose (a tag triggers a release
  build). Tagging discipline is manual.
- **Nothing proves a user-facing change wrote a fragment.** `version-sync` now proves `/VERSION`
  names a release that has a note in `docs/changelog/released/`, and that no note claims a version
  above it, but a PR that ships a visible fix and writes no fragment is invisible to every machine
  here. That half stays a reviewer's duty ([`changelog.md`](changelog.md) → Enforcement).
- **The Windows CI `windows` job builds Debug**, where `<Version>` still resolves from `/VERSION`,
  but the Store package version (`.0` revision, manifest stamp) is only exercised by `package.ps1`
  in a release build, which this repository does not run. A manifest-versioning regression surfaces
  at release, not in per-commit CI.

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change the app version or how a
client derives it:

1. Change [`/VERSION`](../VERSION) only by cutting a release (`scripts/dev/release.py`, which calls
   `scripts/dev/bump-version.sh`), never a per-client literal and never in a feature PR.
2. Keep the derivation identical in spirit across platforms: marketing version = `/VERSION`, build
   number derived per store, no committed value except the two pinned mirrors.
3. The version-sync check ([`scripts/ci/check-version-sync.sh`](../scripts/ci/check-version-sync.sh))
   fails the build if a mirror drifts, a derivation stops reading `/VERSION`, or `/VERSION` names a
   release with no note under `docs/changelog/released/`; it runs on **every** push, not behind
   change-area gating, because `/VERSION` feeds every client.
4. A new platform derives its version from `/VERSION` **before** it ships; any shortfall goes under
   "Known gaps", never left silent.
