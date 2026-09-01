#!/usr/bin/env bash
# Guard how the Linux client hands a URI or a file to the rest of the desktop. Runs in the
# always-run `checks` CI job and locally:
#
#     scripts/ci/check-desktop-handoff.sh
#
# `g_app_info_launch_default_for_uri` and its siblings resolve against the **desktop's application
# database**. A Flatpak has no such database, so GIO falls back through GVFS onto the session bus,
# and the call is **synchronous**, on the GTK main thread. Measured on the shape that ships: the
# whole app froze, with no repaint and no Cancel button left to press, on every attempt to open the
# browser for a sign-in.
#
# It fails in the worst possible way. A developer running `--host` (the distribution's GTK, outside
# the sandbox) sees it work perfectly, because there the application database is right there. Only
# the packaged build wedges, which is the one nobody runs while iterating; and no test can see it,
# because reproducing it needs a portal and a sandbox.
#
# The right APIs are the portal-shaped, asynchronous ones, and this is what the rule enforces:
#
#   opening a URI    -> gtk::UriLauncher     (GTK 4.10+, asks the OpenURI portal)
#   opening a file   -> gtk::FileLauncher    (passes a file descriptor, not a sandbox-local path)
#
# Both take a callback, so a portal that never answers costs the user nothing.
#
# `git grep` is used so this never descends into a submodule, target/ or generated bindings, and
# --untracked so a newly added file is covered before it is staged.
set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../.."

fail=0

# The call names that reach the host application database. `show_uri` is included for the same
# reason: it is GTK's older, pre-portal spelling, kept working but not portal-aware everywhere.
BANNED=(
  'AppInfo::launch_default_for_uri'
  'AppInfo::launch_uris'
  'AppInfo::launch('
  'gtk::show_uri'
  'show_uri_full'
)

for call in "${BANNED[@]}"; do
  # -F: these are literal call names, not patterns. The pathspecs go after a single `--`, or git
  # reads the first one as a rev and dies -- and an error must never read as "no matches", which is
  # how this very check passed while the call it bans sat in the tree.
  set +e
  hits="$(git grep -n --untracked -F "$call" -- 'clients/linux/**/*.rs' 2>&1)"
  status=$?
  set -e
  case "$status" in
    0)
      printf 'ERROR: %s reaches the desktop application database, which a Flatpak does not have.\n' "$call"
      printf '%s\n' "$hits" | sed 's/^/    /'
      fail=1
      ;;
    1) ;;  # no matches, which is the point
    *)
      printf 'ERROR: could not search for %s -- git grep exited %s:\n%s\n' "$call" "$status" "$hits" >&2
      exit 2
      ;;
  esac
done

if (( fail )); then
  cat >&2 <<'WHY'

Use the portal-shaped launchers instead. They are asynchronous, so they cannot freeze the window
they were called from, and they ask the OpenURI portal rather than a database the sandbox lacks:

    a URI   gtk::UriLauncher::new(uri).launch(parent, cancellable, move |result| { … })
    a file  gtk::FileLauncher::new(Some(&file)).launch(parent, cancellable, move |result| { … })

The failure arrives in the callback rather than as a return value, so route it through an AppInput
the way clients/linux/src/ui/operations.rs does for an attachment.
WHY
  exit 1
fi

printf 'OK: the Linux client hands every URI and file to the desktop through the portal launchers.\n'
