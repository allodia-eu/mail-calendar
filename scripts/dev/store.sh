#!/usr/bin/env bash
# Read-only inspection of a client's engine store (the `mailcal.sqlite` SQLite database the
# engine persists mail/calendar objects into). The ground truth when the UI and your mental
# model disagree: it answers "what did the engine actually persist?" without a debugger, and
# without trusting the view-model that may itself be the thing under suspicion.
#
#   scripts/dev/store.sh path   [--platform P] [--store S]   # resolve + print the db path
#   scripts/dev/store.sh tables [--platform P] [--store S]   # list tables + row counts
#   scripts/dev/store.sh schema <table> [...]                # columns of one table
#   scripts/dev/store.sh sql "<SELECT ...>" [...]            # run a read-only query
#   scripts/dev/store.sh threads [subject-substring] [...]   # thread grouping report (mail)
#
#   --platform windows | macos | linux | android | iphone | ipad   (default: the host)
#   --store    real | dev | dev-imap                         (default: real)
#
# SAFETY. This never writes: every query runs against a *copy* of the store (main + `-wal` +
# `-shm`, so a running app's uncommitted WAL frames are included), opened `mode=ro`, in a temp
# dir removed on exit. `sql` refuses anything that isn't a SELECT/WITH/PRAGMA/EXPLAIN.
#
# PRIVACY. The `real` store holds the developer's actual mail. `threads` prints only headers
# (subject, Message-ID, thread id); never bodies. Ad-hoc `sql` can print anything you ask it
# to, so don't paste its output into an issue or a PR without reading it first. Prefer
# `--store dev` (the harness mailbox) when the bug reproduces there.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

DB_NAME="mailcal.sqlite"

PLATFORM=""
STORE="real"
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --platform) PLATFORM="${2:?missing value for --platform}"; shift 2 ;;
    --store) STORE="${2:?missing value for --store}"; shift 2 ;;
    *) ARGS+=("$1"); shift ;;
  esac
done
set -- ${ARGS[@]+"${ARGS[@]}"}
cmd="${1:-path}"

case "$STORE" in real|dev|dev-imap) ;; *) die "unknown --store '$STORE' (real|dev|dev-imap)" ;; esac

# Default to the host's own client, so a bare invocation on a dev box just works.
if [[ -z "$PLATFORM" ]]; then
  PLATFORM="$(host_desktop_client)"
  [[ -n "$PLATFORM" ]] ||
    die "no default client on $(host_os): pass --platform windows|macos|linux|android|iphone|ipad"
fi

# The engine store dir per client. Each dev mode gets an isolated dir so harness data never mixes
# with real accounts; the same mapping the clients apply when they choose their data dir.
#   windows  MailboxModel.Accounts.cs  (LocalAppData/Allodia/MailCalendar[/dev|/dev-imap])
#   macos    MailcalModel.connect      (~/.local/share/mailcal[-dev|-dev-imap])
#   linux    boot.rs                   ($XDG_DATA_HOME/mailcal[/dev|/dev-imap])
#   apple    MailcalModel.connect      (App Support/mailcal[-dev|-dev-imap])
#   android  MainActivity.connect      (files/[dev|dev-imap])
#
# Linux is the same path whether the client was built against the distribution's GTK or run
# through the GNOME runtime: `flatpak run` on a *runtime* creates no per-app home, so XDG_DATA_HOME
# stays unset inside and the app writes to the developer's own data directory either way.
#
# The desktop dev-mode halves come from `dev_store_dir` in lib.sh, which harness.sh also clears
# after a reset; one mapping, so a store this can read can never be one that misses the wipe.
# Only the `real` and mobile cases are decided here.
store_dir() {
  case "$PLATFORM" in
    windows|macos|linux)
      if [[ "$STORE" != real ]]; then dev_store_dir "$PLATFORM" "$STORE"; return; fi
      case "$PLATFORM" in
        windows) printf '%s/Allodia/MailCalendar' "${LOCALAPPDATA:-$HOME/AppData/Local}" ;;
        # The Linux client resolves its data directory through XDG_DATA_HOME; the macOS one lands
        # on the same path but does not read the variable.
        linux) printf '%s/mailcal' "${XDG_DATA_HOME:-$HOME/.local/share}" ;;
        macos) printf '%s/.local/share/mailcal' "$HOME" ;;
      esac ;;
    iphone|ipad)
      local container; container="$(sim_app_container)" || die "no booted simulator with the app installed"
      local name; name="$(apple_dir_name)"
      printf '%s/Library/Application Support/%s' "$container" "$name" ;;
    android) printf 'files%s' "$(android_subdir)" ;;
    *) die "unknown --platform '$PLATFORM' (windows|macos|linux|android|iphone|ipad)" ;;
  esac
}

apple_dir_name() {
  case "$STORE" in real) printf 'mailcal' ;; dev) printf 'mailcal-dev' ;; dev-imap) printf 'mailcal-dev-imap' ;; esac
}
android_subdir() {
  case "$STORE" in real) printf '' ;; dev) printf '/dev' ;; dev-imap) printf '/dev-imap' ;; esac
}

# Copy the store aside before reading it. A live app keeps recent commits in the `-wal`, so the
# main file alone is stale (often hours behind); bring all three parts.
#
# The snapshot is a full copy of the developer's mailbox, so it must not outlive the command.
# `snapshot_db` therefore sets globals rather than echoing a path: called as `db="$(snapshot_db)"`
# it would run in a subshell, the parent's SNAPSHOT_DIR would stay empty, and the EXIT trap
# (which subshells do not inherit) would leave the copy behind in /tmp.
SNAPSHOT_DIR=""
DB_PATH=""
# An `if`, not `[[ ]] && rm`: an EXIT trap's last command decides the script's exit status, so a
# command that made no snapshot (e.g. `path`) would report failure after succeeding.
cleanup() {
  if [[ -n "$SNAPSHOT_DIR" && -d "$SNAPSHOT_DIR" ]]; then rm -rf "$SNAPSHOT_DIR"; fi
}
trap cleanup EXIT INT TERM

snapshot_db() {
  local dir; dir="$(store_dir)"
  SNAPSHOT_DIR="$(mktemp -d)"
  chmod 700 "$SNAPSHOT_DIR"
  if [[ "$PLATFORM" == "android" ]]; then
    local adb; adb="$(adb_bin)"
    for part in "" "-wal" "-shm"; do
      "$adb" exec-out run-as "$ANDROID_PKG" cat "$dir/$DB_NAME$part" > "$SNAPSHOT_DIR/$DB_NAME$part" 2>/dev/null || true
    done
    [[ -s "$SNAPSHOT_DIR/$DB_NAME" ]] || die "no store at $dir/$DB_NAME (app installed? debuggable build? --store $STORE synced yet?)"
  else
    [[ -f "$dir/$DB_NAME" ]] || die "no store at $dir/$DB_NAME (has the app run with --store $STORE?)"
    for part in "" "-wal" "-shm"; do
      [[ -f "$dir/$DB_NAME$part" ]] && cp "$dir/$DB_NAME$part" "$SNAPSHOT_DIR/" || true
    done
  fi
  DB_PATH="$SNAPSHOT_DIR/$DB_NAME"
}

# Query the snapshot. Prefer the sqlite3 CLI; fall back to Python's bundled sqlite3 module, which
# is always present on a machine that can build this repo and is the only option on a bare Windows
# box (no sqlite3.exe ships with Windows or with Git for Windows).
run_sql() {
  local db="$1" sql="$2"
  if command -v sqlite3 >/dev/null 2>&1; then
    sqlite3 -readonly -header -box "$db" "$sql"
  else
    require_cmd python3
    SQL="$sql" DB="$db" python3 "$(dirname "${BASH_SOURCE[0]}")/store_query.py"
  fi
}

assert_read_only() {
  local head; head="$(printf '%s' "$1" | tr '[:lower:]' '[:upper:]' | sed 's/^[[:space:](]*//')"
  case "$head" in
    SELECT*|WITH*|PRAGMA*|EXPLAIN*) ;;
    *) die "refusing: only SELECT / WITH / PRAGMA / EXPLAIN are allowed (got: ${1:0:40}…)" ;;
  esac
}

case "$cmd" in
  path)
    # Assign first: `die` inside a `$(...)` only exits the subshell, so an unsupported combination
    # (e.g. an unknown --platform) would print a truncated path instead of refusing.
    dir="$(store_dir)"
    printf '%s/%s\n' "$dir" "$DB_NAME"
    # An `if`, not `[[ ]] && info`: a trailing false test is the script's exit status under `set -e`,
    # so the non-Android path would exit 1 despite having succeeded.
    if [[ "$PLATFORM" == "android" ]]; then
      info "inside the app sandbox - read it with: adb exec-out run-as $ANDROID_PKG cat <path>"
    fi
    ;;
  tables)
    snapshot_db
    [[ "$STORE" == "real" ]] && warn "reading the REAL store - it holds your actual mail"
    run_sql "$DB_PATH" "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    ;;
  schema)
    table="${2:?usage: store.sh schema <table>}"
    snapshot_db
    run_sql "$DB_PATH" "PRAGMA table_info($table)"
    ;;
  sql)
    query="${2:?usage: store.sh sql \"<SELECT ...>\"}"
    assert_read_only "$query"
    snapshot_db
    [[ "$STORE" == "real" ]] && warn "reading the REAL store - read the output before sharing it"
    run_sql "$DB_PATH" "$query"
    ;;
  threads)
    needle="${2:-}"
    snapshot_db
    [[ "$STORE" == "real" ]] && warn "reading the REAL store - subjects/Message-IDs below are your own mail"
    require_cmd python3
    DB="$DB_PATH" NEEDLE="$needle" python3 "$(dirname "${BASH_SOURCE[0]}")/store_threads.py"
    ;;
  help|-h|--help)
    sed -n '2,25p' "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/store.sh" | sed 's/^# \{0,1\}//'
    ;;
  *) die "unknown command '$cmd' (path|tables|schema|sql|threads|help)" ;;
esac
