#!/usr/bin/env bash
# Fail if something the public repository must not carry has come back.
#
# The tree is being made publishable while work continues in it, so the strip is not a one-off: a
# private-tracker reference written today is one that has to be stripped again tomorrow, and the
# pipeline that copies the tree cannot tell a fresh one from a missed one. This is the machine
# half; the patterns a grep can decide, held at zero from here on.
#
# Run from the repo root:
#
#     scripts/ci/check-public-hygiene.sh
#
# What it deliberately does NOT decide: prose. Whether a paragraph is competitor commentary,
# whether a design note may cite another mail client, whether a document belongs in the public
# tree at all; those need a reading, and they are settled per file, not per pattern.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

# Paths the public copy does not receive at all, so what is in them is not this check's business.
# Kept here rather than only in the copying script because this is the list a reader wants in
# front of them when a match looks like a false positive.
EXCLUDED=(
  ':!docs/*-plan.md'              # working plans, excluded entirely
  ':!docs/changelog/released'     # shipped release notes: history, not maintained prose
  ':!clients/android/gradlew'     # vendored upstream
  ':!clients/composer/dist'       # the generated editor bundle
  ':!.agents/skills/gh-stack'     # a third-party CLI's own worked examples
  ':!scripts/ci/check-public-hygiene.sh'      # the patterns themselves,
  ':!scripts/ci/tests/test_public_hygiene.py' # and the fixtures written to trip them
)

fail=0

# `--untracked` is not optional here. Without it `git grep` reads the index, so a new file is
# invisible until it is staged; and the check passes on the very working tree that introduces the
# thing it exists to forbid, then fails on main once the file is committed. Ignored paths (target/,
# .env) stay ignored either way, so this widens the search without widening the noise.
deny() { # <what> <extended regex> [extra pathspec...]
  local what="$1" pattern="$2" hits
  shift 2
  hits="$(git grep --untracked -InE "$pattern" -- . "${EXCLUDED[@]}" "$@" || true)"
  if [ -n "$hits" ]; then
    printf 'ERROR: %s\n%s\n\n' "$what" "$hits" >&2
    fail=1
  fi
}

# --- The private tracker ---------------------------------------------------------------------
# `#NNN` addresses an issue in a repository the public tree has no link to, so the reference is a
# dead end for every reader it reaches. The repo's own writing rule covers the same ground from
# the other side: a comment says what the code does now, not which change landed it.
#
# Two exemptions, both narrow:
#   * the ENGINE's tracker is public, so a line naming that repository may carry a number;
#   * #1234 and #1281 are the designated stand-ins, for the few places that have to show an
#     issue-shaped token in order to forbid one (AGENTS.md's ✗ example, the log-hygiene rule).
ISSUE_SHAPE='(\(#[0-9]{2,4}\)|issue #[0-9]+|[Pp][Rr] #[0-9]+|\[#[0-9]{2,4}\]|→ #[0-9]+)'
tracker="$(git grep --untracked -InE "$ISSUE_SHAPE" -- . "${EXCLUDED[@]}" || true)"
tracker="$(printf '%s' "$tracker" | { grep -v 'email-calendar-sync-engine' || true; })"
tracker="$(printf '%s' "$tracker" | { grep -vE '#(1234|1281)([^0-9]|$)' || true; })"
if [ -n "$tracker" ]; then
  printf 'ERROR: a reference to the private tracker (state the rule, not the ticket):\n%s\n\n' \
    "$tracker" >&2
  fail=1
fi

# --- A plan's phases -------------------------------------------------------------------------
# "Phase A", "the field-parity wave", "Pre-Phase-4" name a step in a roadmap. Two things are
# wrong with one in the code: it points at a document the reader may not have, and it stops being
# true the moment the plan moves; a phase that has shipped, been renamed or been abandoned leaves
# a comment describing a future that never arrived. Say the fact instead. *The editor does not
# offer this yet* is what the reader needed, and it stays true however the plan ends.
#
# The roadmap that defines the phases is exempt, because that is where a phase name means
# something; `docs/*-plan.md` is excluded from the public tree outright, above.
# Capitalised and a single token: a plan phase is a proper noun ("Phase A", "Phase-0"). The
# domain's own uses; a gesture's propagation phase, an Xcode build phase, a sync's phases; are
# lowercase and stay untouched, which is why the pattern is not case-insensitive.
# The boundaries are spelled out rather than written `\b`: this is a POSIX ERE, where `\b` is
# not a word boundary and the pattern silently matches nothing; a check that cannot fail.
#
# Nothing that ships is exempt: a finished plan is deleted, not carried and excused.
PHASE_SHAPE='(Phase[ -][A-Z0-9]([^A-Za-z0-9]|$)|the [a-z-]+ wave([^a-z]|$))'
phases="$(git grep --untracked -InE "$PHASE_SHAPE" -- . "${EXCLUDED[@]}" || true)"
if [ -n "$phases" ]; then
  printf 'ERROR: a plan phase named outside the roadmap (state the fact, not the phase):\n%s\n\n' \
    "$phases" >&2
  fail=1
fi

# --- People and reservations -----------------------------------------------------------------
# Fixtures and docs name no individual: a signature body, an attendee CN and a primary address
# all read better as `alice`, and the rest of the suite already does. The Apple team id and the
# certificate display names are Allodia's reservations, which the release repository owns; a
# public repository carrying them can put something in a store under this product's name.
# The four documents that need a party name one on purpose. An eenmanszaak has no legal
# personality of its own, so "Allodia" alone would name nobody a licence could bind; which is the
# whole reason the CLA's placeholder was a placeholder.
# The three rules below name the values they forbid, and this file ships. That is unavoidable --
# a check cannot forbid a value without holding it -- and it is acceptable because none of these is
# a secret: a licence and a CLA name the licensor deliberately, an Apple team id is in every shipped
# bundle, and the seller name is on the store listing. The one that is merely unpleasant to publish
# is the test account, and if that ever matters the fix is to rotate the account, not to hide the
# pattern from a file anyone can read.
#
# What does not ship is this script's *test*, which would otherwise concentrate every one of these
# values in a single file that is exempt from the check by construction. See public-exclude.txt.
deny "a personal identifier in a fixture, doc or script:" '(Dennis|Ameling|dennisameling)' \
  ':!CLA.md' ':!REUSE.toml' ':!allodia_license/LICENSE.md' ':!LICENSES/LicenseRef-*'
deny "an Apple team reservation:" '(X98DRMUM3J|947BB2P68Y|Fits4all)'
deny "an internal test account:" 'allodia\.e2e'

if [ "$fail" -ne 0 ]; then
  echo "ERROR: the tree carries something the public repository must not." >&2
  exit 1
fi
echo "OK: no private references, plan phases, personal identifiers or store reservations."
