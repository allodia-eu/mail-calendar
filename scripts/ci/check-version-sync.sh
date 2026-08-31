#!/usr/bin/env bash
# Fail if the one app version has drifted between its source of truth and any of its mirrors.
#
# The top-level /VERSION file is the single marketing version (docs/versioning.md). Two build systems
# cannot read an external file, so they carry a COMMITTED mirror that this check pins to /VERSION:
#
#   - Cargo.toml   [workspace.package] version   (Cargo can't read /VERSION)
#   - project.yml  MARKETING_VERSION             (the dev-loop honesty mirror; package.sh re-stamps it)
#
# The other three clients DERIVE their version from /VERSION at build time, so there is nothing to
# drift; but a hardcoded literal creeping back is exactly the regression that would silently
# un-sync them, so this check also asserts each still reads the file (and the Apple build number uses
# the dotted, non-overflowing timestamp). Machine enforcement for a rule no formatter has a lint for;
# wired into CI (its own always-run job, like file-length) and runnable locally from the repo root:
#
#     scripts/ci/check-version-sync.sh
#
# Bump everything at once with scripts/dev/bump-version.sh X.Y.Z (it edits the two mirrors and runs
# this check).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
note() { printf '  %s\n' "$1" >&2; fail=1; }

# --- The source of truth ------------------------------------------------------------------------
[ -f VERSION ] || { echo "ERROR: /VERSION is missing: it is the single source of truth." >&2; exit 1; }
VERSION="$(tr -d '[:space:]' <VERSION)"
if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "ERROR: /VERSION must be a single MAJOR.MINOR.PATCH line (got '$VERSION')." >&2
  exit 1
fi

# --- Committed mirrors, which must equal /VERSION exactly ----------------------------------------
# Cargo.toml: the `version` line inside [workspace.package] (not a dependency's inline version).
cargo_version="$(awk '
  /^\[workspace\.package\]/ { inpkg = 1; next }
  /^\[/                     { inpkg = 0 }
  inpkg && /^version[[:space:]]*=/ { gsub(/[^0-9.]/, ""); print; exit }
' Cargo.toml)"
[ "$cargo_version" = "$VERSION" ] || note "Cargo.toml [workspace.package] version is '$cargo_version', expected '$VERSION'."

# Cargo.lock: a mirror in all but name, and the one that used to be missed. It carries a version per
# workspace member, and a release that moved Cargo.toml without it produced a tag whose every
# `--locked` build failed with "cannot update the lock file". Checked against a member that is
# always present rather than all of them, because one drifting means the bump skipped the lock
# entirely.
lock_version="$(awk '
  /^name = "mailcal-app"$/ { found = 1; next }
  found && /^version = "/ { gsub(/[^0-9.]/, ""); print; exit }
' Cargo.lock)"
[ "$lock_version" = "$VERSION" ] ||
  note "Cargo.lock has mailcal-app at '$lock_version', expected '$VERSION'. Run: cargo update --workspace"

# project.yml: MARKETING_VERSION under settings.base (the `$(MARKETING_VERSION)` Info.plist reference
# has no trailing colon, so this matches only the real setting).
apple_marketing="$(grep -Eo 'MARKETING_VERSION:[[:space:]]*"[0-9.]+"' clients/apple/project.yml | head -n1 | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+')"
[ "$apple_marketing" = "$VERSION" ] || note "clients/apple/project.yml MARKETING_VERSION is '$apple_marketing', expected '$VERSION'."

# --- Derived mechanisms, which must still read /VERSION (no literal crept back) ------------------
gradle=clients/android/app/build.gradle.kts
grep -q 'rootProject.file("../../VERSION")' "$gradle" \
  || note "$gradle no longer reads ../../VERSION: the version must stay derived."
# A re-introduced hardcoded marketing version is the drift this guards against.
if grep -Eq 'versionName[[:space:]]*=[[:space:]]*"[0-9]' "$gradle"; then
  note "$gradle has a hardcoded versionName literal: derive it from /VERSION instead."
fi

csproj=clients/windows/Mailcal/Mailcal.csproj
grep -q 'ReadAllText(.*VERSION' "$csproj" \
  || note "$csproj no longer reads /VERSION for <Version>: DeviceFacts.cs would report a stale app_version."

pkgsh=clients/apple/Scripts/package.sh
grep -q 'cat "$ROOT/VERSION"' "$pkgsh" \
  || note "$pkgsh no longer defaults MARKETING_VERSION from /VERSION."
# The build number must be the dotted timestamp; the bare single-integer form overflows CFBundleVersion.
grep -q 'date -u +%Y.%m%d.%H%M' "$pkgsh" \
  || note "$pkgsh build number is not the dotted timestamp (date -u +%Y.%m%d.%H%M): a single integer overflows CFBundleVersion."
if grep -q 'date +%Y%m%d%H%M' "$pkgsh"; then
  note "$pkgsh still has the overflowing single-integer build number (date +%Y%m%d%H%M)."
fi

# The Linux metainfo is generated at build time from /VERSION plus the release note's own date.
# The regression this guards is a committed literal: the template is a tracked file, and a
# `<release version="0.4.0" date="…"/>` typed straight into it would build, install and validate,
# and then advertise whatever version the last person to edit it happened to have.
metainfo_template=clients/linux/flatpak/metainfo.xml.in
generator=scripts/dev/flatpak_metadata.py
if grep -Eq '<release[[:space:]]+version="[0-9]' "$metainfo_template"; then
  note "$metainfo_template has a hardcoded <release version=…> literal: the release list is generated by $generator from /VERSION."
fi
grep -q 'VERSION' "$generator" \
  || note "$generator no longer reads /VERSION: the Linux client's advertised version must stay derived."

pkgps=clients/windows/package.ps1
grep -q "Get-Content (Join-Path \$root 'VERSION')" "$pkgps" \
  || note "$pkgps no longer reads /VERSION for the package version."
# Reading /VERSION is not enough; the -Sign sideload path used to *ignore* it and hardcode a
# "1.0.$build.$rev" package version (major.minor stuck at 1.0 regardless of /VERSION). Guard the
# regression class directly: any string literal assigned to $Version must not begin with a digit
# (the derived forms start from $semver/$mm, e.g. "$($mm[0])...").
if grep -Eq '\$Version[[:space:]]*=[[:space:]]*"[0-9]' "$pkgps"; then
  note "$pkgps assigns a hardcoded numeric \$Version literal: derive MAJOR.MINOR from /VERSION (\$semver) instead."
fi

# --- The release note for that version, which must exist ----------------------------------------
# /VERSION means "the version users currently have" (docs/changelog.md), so it can only name a
# release that was actually assembled. This closes the gap AGENTS.md used to admit in writing: the
# checks proved the mirrors AGREED, never that the number had MOVED for a reason. A version with no
# note is a release nobody can describe; a note above /VERSION is a release that has not happened.
released_dir=docs/changelog/released
[ -f "$released_dir/$VERSION.md" ] \
  || note "$released_dir/$VERSION.md is missing: /VERSION names a release with no note. Cut the release with scripts/dev/release.py rather than editing /VERSION by hand."

# `sort -V` is not portable enough to bet a gate on (BSD and GNU disagree on prerelease handling),
# and the versions here are always three plain integers; so compare them as three integers.
newer() { # newer A B -> true when A > B
  awk -F. -v a="$1" -v b="$2" 'BEGIN {
    split(a, x, "."); split(b, y, ".")
    for (i = 1; i <= 3; i++) { if (x[i] + 0 > y[i] + 0) exit 0; if (x[i] + 0 < y[i] + 0) exit 1 }
    exit 1
  }'
}
shopt -s nullglob
for note_file in "$released_dir"/*.md; do
  candidate="$(basename "$note_file" .md)"
  if [[ ! "$candidate" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    note "$note_file is not named X.Y.Z.md: the filename is how /VERSION finds its note."
    continue
  fi
  if newer "$candidate" "$VERSION"; then
    note "$note_file names $candidate, which is above /VERSION ($VERSION): a released note may not describe a release that has not happened."
  fi
done
shopt -u nullglob

if [ "$fail" -ne 0 ]; then
  echo "ERROR: app version is out of sync: see above. Bump with scripts/dev/bump-version.sh, or fix the drift." >&2
  exit 1
fi

echo "OK: every version mirror and derivation is in sync with /VERSION ($VERSION)."
