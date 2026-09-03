#!/usr/bin/env bash
# Guard the single Tokio runtime the Linux client's desktop-portal callers share. Runs in the
# always-run `checks` CI job and locally:
#
#     scripts/ci/check-portal-runtime.sh
#
# `ashpd` caches one `zbus::Connection` for the whole process, and zbus drives that connection from
# whichever runtime opened it. A runtime built per call, or owned by one service, takes the
# connection's reader with it when it is dropped while the connection itself stays cached. Every
# later portal call then awaits a reply that can never arrive: no error, no timeout, a thread parked
# for the life of the process, and whatever state machine that thread was serving parked with it.
#
# It fails in the worst possible way, because **the first portal call of the process succeeds**. A
# manual check passes, a screenshot proves nothing, and only the second call hangs; and the two
# callers are far apart, so the store that seeds the connection and the notification that hangs on
# it look like unrelated code. That is why the shape is caught in the source rather than in a run.
#
# Test code may build its own: it reaches no portal, and the secure store's nesting guard needs two
# distinct runtimes to prove anything at all. A file's first `#[cfg(test)]` marks where that
# begins, and a `*_tests.rs` file is test code throughout.
#
# `git grep` is used so this never descends into target/ or generated bindings, and --untracked so
# a newly added file is covered before it is staged.
set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../.."

# The one module allowed to build a runtime, because it owns the only one and never drops it.
OWNER='clients/linux/src/host_runtime.rs'
BUILDERS='runtime::Builder|Builder::new_multi_thread|Builder::new_current_thread'

# An error must never read as "no matches": a check that reports OK because it could not look is
# worse than no check.
set +e
hits="$(git grep -n --untracked -E "$BUILDERS" -- 'clients/linux/**/*.rs' 2>&1)"
status=$?
set -e
case "$status" in
  0 | 1) ;;
  *)
    printf 'ERROR: could not search for a Tokio runtime, git grep exited %s:\n%s\n' \
      "$status" "$hits" >&2
    exit 2
    ;;
esac

fail=0
while IFS= read -r hit; do
  [[ -n "$hit" ]] || continue
  file="${hit%%:*}"
  rest="${hit#*:}"
  line="${rest%%:*}"
  [[ "$file" == "$OWNER" ]] && continue
  [[ "$file" == *_tests.rs ]] && continue
  cutoff="$(git grep -n --untracked -F -e '#[cfg(test)]' -- "$file" 2>/dev/null |
    head -1 | cut -d: -f2 || true)"
  if [[ -n "$cutoff" ]] && ((line > cutoff)); then
    continue
  fi
  printf 'ERROR: %s builds a Tokio runtime of its own.\n    %s\n' "$file" "$hit"
  fail=1
done <<<"$hits"

if ((fail)); then
  cat >&2 <<'WHY'

Take the shared one instead:

    let Some(runtime) = crate::host_runtime::shared() else { ... };

It outlives every caller, so the portal connection it opened stays driven. A runtime of your own
works exactly once and then hangs the process for good (docs/client-traps.md).
WHY
  exit 1
fi
