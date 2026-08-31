#!/usr/bin/env bash
# Fail if the default build can reach into `allodia_license/`.
#
# That directory is the one part of the repository under a licence other than the GPL, and the
# promise attached to it is that the open tree does not need it: the application compiles, tests
# and runs with no reference to anything closed (docs/pledge.md, promise 4). A promise nothing
# checks is a promise that quietly stops being true the first time an import looks convenient.
#
# Run from the repo root:
#
#     scripts/ci/check-license-dir.sh
#
# Two rules. The first is structural, so a crate added to that directory is outside the default
# build whether or not anyone remembers; the second is the one a person can break in an afternoon.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

DIR="allodia_license"
fail=0

# --- 1. The default build does not include it --------------------------------------------------
# The crate is a workspace *member* on purpose: that is what lets it share one lockfile, one set of
# dependency versions and one copy of the engine. What keeps it out of a shipped build is its
# absence from `default-members`, which is an explicit list rather than a subtraction -- so adding a
# member never adds it to the default build, and putting this one there would have to be typed out.
default_members="$(awk '/^default-members[[:space:]]*=/,/\]/' Cargo.toml)"
if [ -z "$default_members" ]; then
  printf 'ERROR: Cargo.toml has no default-members list, so every member is in the default build.\n' >&2
  fail=1
elif printf '%s' "$default_members" | grep -q "$DIR"; then
  printf 'ERROR: %s/ is in default-members, so a bare `cargo build` ships it.\n' "$DIR" >&2
  fail=1
fi

# --- 2. Nothing the default build reaches into it ------------------------------------------------
# The **path**, not the word. `allodia_license` is also the Rust identifier of the crate that lives
# there, and `use allodia_license::…` is not a reach into a directory; whether that crate is
# linked at all is decided by rule 3 below. What a reach looks like is a path with a component
# after it: a `path =` in a manifest, a `srcDir(…)`, an `#[path]`.
#
# Only build inputs are searched. Prose *should* name the directory; the README explains it, the
# pledge promises it, this script is about it; and a rule that forbade the word would be a rule
# nobody could document the seam under.
hits="$(git grep --untracked -In "$DIR/" -- crates clients Cargo.toml \
  ':!clients/*/README.md' ':!clients/*/*/README.md' || true)"
# The `members` line is the one reference a manifest must carry, and the comment above it says why,
# which is worth more to the next reader than a clean grep. Both are manifest lines starting with
# `members` or `#`, and nothing else in one does. `default-members` is checked above, not here.
hits="$(printf '%s' "$hits" | { grep -vE "^Cargo\.toml:[0-9]+:[[:space:]]*(members[[:space:]]*=|#)" || true; })"
# The optional dependency and the feature that gates it. This is the seam itself: the line has to
# exist for an Allodia build to turn the directory on, and it is `optional = true` that keeps it out
# of everyone else's. Anything else naming the directory in a manifest is not this, so the pattern
# is the whole line rather than the word.
#
# Which crate carries it is not pinned, and deliberately. The seam has moved once already -- to the
# crate that may open sockets -- and a rule about *shape* that fails over a move changing nothing
# about that shape is a rule people learn to edit rather than to read. What is pinned is the shape:
# the relative path from a crate manifest, `optional = true`, and the `dep:` feature gating it.
hits="$(printf '%s' "$hits" | { grep -vE "^crates/[a-z0-9-]+/Cargo\.toml:[0-9]+:[[:space:]]*(#|allodia-license = \{ path = \"\.\./\.\./$DIR/crates/allodia-license\", optional = true \}|allodia-license = \[\"dep:allodia-license\"\])" || true; })"
if [ -n "$hits" ]; then
  printf 'ERROR: the default build references %s/ (it must build without it):\n%s\n' "$DIR" "$hits" >&2
  fail=1
fi

# --- 3. The feature that turns it on is off ------------------------------------------------------
# The optional dependency is only half the guarantee. A feature listed in `default = [...]` is on
# for everyone, and the manifest would still read as "optional" to anyone skimming it.
defaulted="$(git grep --untracked -In '^default = .*allodia-license' -- crates || true)"
if [ -n "$defaulted" ]; then
  printf 'ERROR: the %s feature is on by default, so every build links it:\n%s\n' \
    "$DIR" "$defaulted" >&2
  fail=1
fi

# --- 4. The two copies of the licence text agree -------------------------------------------------
# The text lives twice for the same reason the GPL does: `LICENSES/` is where `reuse lint` reads a
# licence, and the directory itself is where a person looks. A copy nobody compares is a copy that
# disagrees, and `reuse lint` checks that the text exists, never that it says the same thing.
MIRROR="LICENSES/LicenseRef-Allodia-1.0.txt"
if [ -f "$DIR/LICENSE.md" ] && [ -f "$MIRROR" ]; then
  if ! cmp -s "$DIR/LICENSE.md" "$MIRROR"; then
    printf 'ERROR: %s and %s have drifted. Copy one over the other.\n' "$DIR/LICENSE.md" "$MIRROR" >&2
    fail=1
  fi
elif [ -f "$DIR/LICENSE.md" ] || [ -f "$MIRROR" ]; then
  printf 'ERROR: %s and %s must both exist: one is missing.\n' "$DIR/LICENSE.md" "$MIRROR" >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "OK: the default build does not reach into $DIR/."
