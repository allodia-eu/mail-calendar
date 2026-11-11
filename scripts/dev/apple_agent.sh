#!/usr/bin/env bash
# Where the macOS and iOS Simulator app's test agent listens (docs/test-agent.md). Sourced by
# control.sh and clients/apple/Scripts/build-and-run.sh, after lib.sh; not run directly.

# Bash 5 or newer, like every script in this tree (AGENTS.md, "Building & verifying").
if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${BASH_SOURCE[0]##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  return 1 2>/dev/null || exit 1
fi

# The socket this checkout's app answers its agent on, one per checkout and platform, so two
# worktrees, or a Mac and a simulator build of one, never answer for each other. It lives in the
# user's own temporary directory, which nobody else can enter and the simulator reaches as well,
# since a simulated app runs on the Mac's filesystem; the path stays well inside a socket's 104
# bytes.
apple_agent_socket() { # <macos|iphone|ipad>
  local checkout
  checkout="$(printf '%s' "${REPO_ROOT:-$PWD}" | cksum | cut -d' ' -f1)"
  printf '%s\n' "$(getconf DARWIN_USER_TEMP_DIR)mailcal-agent/$checkout-$1.sock"
}
