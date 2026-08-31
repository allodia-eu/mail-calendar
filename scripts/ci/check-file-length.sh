#!/usr/bin/env bash
# Fail if any tracked source file exceeds the line ceiling.
#
# AGENTS.md product hard rule: "Files stay under 500 lines; split by responsibility." The rule
# is not language-qualified, so neither is this check; it is the machine enforcement for every
# language whose formatter/linter has no per-file length lint (rustfmt and clippy have none;
# neither does `dotnet format`). Wired into CI and runnable locally from the repo root:
#
#     scripts/ci/check-file-length.sh
#
# To bring another language under the rule, add its extension to PATTERNS and split whatever
# the run then flags. Not yet covered: *.swift and *.kt; see the follow-up issue; four client
# files there are still over the cap, so gating them is a refactor, not a one-line change.
#
# A file may be exempted only for being a committed build output (see EXCLUDED). "It is hard to
# split" is not a reason; that is the rule working.
#
# `git ls-files` lists only tracked files, so generated code is out of scope by construction
# (the UniFFI C# bindings under clients/windows/Generated are built, never committed), and it
# never descends into an untracked submodule or the gitignored `target/` dir.
set -euo pipefail

MAX=500

# The extensions under the rule. Every tracked file matching one of these is checked.
PATTERNS=('*.rs' '*.cs' '*.ts' '*.js' '*.html' '*.swift' '*.kt')

# The one exception, and the only kind there can be: a build output that is committed rather than
# generated per build. `clients/composer/dist/editor.html` is the whole rich editor inlined into a single
# self-contained file, because that is what its four WebView hosts can load (see the file's own
# header). Its SOURCES; clients/composer/src/*.ts and that directory's index.html; are tracked,
# checked here like everything else, and are where the rule actually bites.
EXCLUDED=('clients/composer/dist/editor.html')

excluded() {
  local candidate="$1" path
  for path in "${EXCLUDED[@]}"; do
    [ "$candidate" = "$path" ] && return 0
  done
  return 1
}

fail=0

while IFS= read -r file; do
  # In a dirty worktree, `git ls-files` still lists a tracked file that was deleted but not
  # staged yet. CI never sees that state, but local agents do while preparing a deletion.
  [ -f "$file" ] || continue
  excluded "$file" && continue
  lines=$(wc -l <"$file")
  if [ "$lines" -gt "$MAX" ]; then
    printf '  %s: %d lines\n' "$file" "$lines"
    fail=1
  fi
done < <(git ls-files "${PATTERNS[@]}")

if [ "$fail" -ne 0 ]; then
  echo "ERROR: the file(s) above exceed the ${MAX}-line limit: split them by responsibility." >&2
  exit 1
fi

echo "OK: every tracked ${PATTERNS[*]} file is within the ${MAX}-line limit."
