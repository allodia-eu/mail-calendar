#!/usr/bin/env bash
# Decide which CI areas a change set can possibly affect, so the expensive per-platform jobs run
# only when something they actually build has moved.
#
# Why a script instead of `on.<event>.paths`: a path-filtered workflow is *skipped*, which reports
# no status at all (and would hang a required check), and the mapping is not a plain path prefix,
# the Rust core and the shared l10n catalog feed every client, so one directory fans out to several
# areas. This runs once in a cheap Linux job; each expensive job gates on its output.
#
# Fail-open by design: a path this script does not recognise turns on EVERY area. A new top-level
# directory therefore over-builds rather than silently skipping its gate.
#
# Usage:
#   changed-areas.sh              # resolve the change set from the GitHub event, write $GITHUB_OUTPUT
#   changed-areas.sh --files -    # classify a newline-separated path list on stdin (local/testing)
#
# Env:
#   FORCE_AREAS   "all", or a comma-separated subset of rust,apple,windows,android,linux. Skips
#                 detection entirely; the manual-dispatch escape hatch for a trigger that missed
#                 a build.
set -euo pipefail

rust=false
apple=false
windows=false
android=false
linux=false

enable_all() {
  rust=true
  apple=true
  windows=true
  android=true
  linux=true
}

enable() {
  case "$1" in
    all) enable_all ;;
    rust) rust=true ;;
    apple) apple=true ;;
    windows) windows=true ;;
    android) android=true ;;
    linux) linux=true ;;
    *)
      echo "changed-areas: unknown area '$1' (want: all, rust, apple, windows, android, linux)" >&2
      exit 2
      ;;
  esac
}

# Map each changed path onto the areas it can break. The first matching pattern wins, so the
# "no CI impact" arms come before the directory arms they overlap; a clients/apple/README.md is
# documentation, not an Apple build input.
classify() {
  local path
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    case "$path" in
      # Prose, agent guidance, submodule pointers: nothing is compiled from these.
      *.md | .gitignore | .gitmodules | docs/* | .agents/* | .claude/*) ;;
      # Renovate's own config decides when dependency PRs are opened; nothing builds from it. The
      # PRs it opens are classified by the manifests they touch, like any other change.
      renovate.json) ;;
      # The dev harness scripts are run by hand; nothing here is a *platform build* input, so no
      # area is turned on. Their own unit tests are not skipped by that: the `dev-scripts` job is
      # ungated and runs on every push, precisely because this arm would otherwise mean a change
      # to the helpers ran no test at all.
      scripts/dev/*) ;;
      # Brand source art. Nothing is derived from it at build time; each client's copy is
      # committed, and `scripts/dev/brand-icons.sh` / `brand-welcome.sh` are run by hand when a
      # source changes. So a change here compiles nothing; it is the *derived* files under clients/
      # that trigger a build. Only the art: `branding/*.env` carries the application id, which every
      # client builds against, and falls through to the catch-all that turns on everything.
      branding/*.png) ;;
      # The Android job runs the client's JVM unit suite (Robolectric + Compose), no emulator.
      clients/android/*) android=true ;;
      clients/apple/*) apple=true ;;
      clients/windows/*) windows=true ;;
      clients/linux/*) linux=true ;;
      # The rich-composer editor is bundled into the existing full clients; Linux included, which
      # compiles `editor.html` into the binary with `include_str!` and so must rebuild with it.
      clients/composer/*)
        apple=true
        windows=true
        android=true
        linux=true
        ;;
      # Every native client generates its localised strings from the shared inlang catalog.
      messages/* | project.inlang/*)
        apple=true
        windows=true
        android=true
        linux=true
        ;;
      # The seeded Stalwart server the gated JMAP live test runs against.
      docker/*) rust=true ;;
      # The nightly rustfmt pin. Its only readers are the `lint` job's fmt step and
      # scripts/dev/gate.sh; nothing is compiled from it, so a bump needs the Rust area rather than
      # a six-job fan-out. Reformatting it causes lands as crates/* edits, which fan out below.
      rust-nightly.toml) rust=true ;;
      # The core, its lockfile and its toolchain pin are compiled into every client, so they fan
      # out to everything. /VERSION is embedded by every client (docs/versioning.md), so it does too.
      crates/* | Cargo.toml | Cargo.lock | rustfmt.toml | rust-toolchain.toml | VERSION) enable_all ;;
      # Anything unrecognised; including .github/ and scripts/ci/ itself: build everything.
      *) enable_all ;;
    esac
  done
}

# Resolve the change set from the event payload rather than from git, so the checkout can stay
# shallow. Prints one path per line; a non-zero exit means "cannot tell", and the caller fails open.
changed_files() {
  local before
  case "${GITHUB_EVENT_NAME:-}" in
    pull_request | pull_request_target)
      gh api --paginate \
        "repos/$GITHUB_REPOSITORY/pulls/$(jq -r '.pull_request.number' "$GITHUB_EVENT_PATH")/files" \
        --jq '.[].filename'
      ;;
    push)
      before=$(jq -r '.before' "$GITHUB_EVENT_PATH")
      # An all-zero `before` means the branch was just created; there is no diff base.
      [[ "$before" =~ ^0+$ ]] && return 1
      gh api "repos/$GITHUB_REPOSITORY/compare/$before...$GITHUB_SHA" --jq '.files[]?.filename'
      ;;
    *) return 1 ;;
  esac
}

files=
reason=

if [[ "${1:-}" == "--files" ]]; then
  files=$(cat -- "${2:--}")
elif [[ -n "${FORCE_AREAS:-}" ]]; then
  for area in ${FORCE_AREAS//,/ }; do enable "$area"; done
  reason="forced via FORCE_AREAS=$FORCE_AREAS"
elif ! files=$(changed_files); then
  enable_all
  reason="could not resolve the change set for '${GITHUB_EVENT_NAME:-?}'"
fi

if [[ -z "$reason" ]]; then
  count=0
  [[ -n "$files" ]] && count=$(printf '%s\n' "$files" | wc -l | tr -d ' ')
  if ((count == 0)); then
    enable_all
    reason="empty change set"
  # The compare API caps at 300 files and the pulls API at 3000, with no truncation flag. At that
  # size the change is sweeping anyway, so stop guessing and build everything.
  elif ((count >= 300)); then
    enable_all
    reason="$count files changed (at the API cap)"
  else
    classify <<<"$files"
    reason="$count file(s) changed"
  fi
fi

echo "changed-areas: $reason"
printf '  %-8s%s\n' rust "$rust" apple "$apple" windows "$windows" android "$android" linux "$linux"

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
  printf 'rust=%s\napple=%s\nwindows=%s\nandroid=%s\nlinux=%s\n' \
    "$rust" "$apple" "$windows" "$android" "$linux" >>"$GITHUB_OUTPUT"
fi
if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  printf '### Changed areas\n\n%s\n\n| area | build |\n|---|---|\n| rust | %s |\n| apple | %s |\n| windows | %s |\n| android | %s |\n| linux | %s |\n' \
    "$reason" "$rust" "$apple" "$windows" "$android" "$linux" >>"$GITHUB_STEP_SUMMARY"
fi
