#!/usr/bin/env bash
# Copies symbolic icons from GNOME's icon-development-kit into the Linux client, verbatim.
#
#   scripts/dev/vendor-idk-icons.sh                  # re-fetch every icon already vendored
#   scripts/dev/vendor-idk-icons.sh bell signature   # add or refresh these, by upstream name
#
# The kit is CC0-1.0 and is the set the GNOME HIG tells an application to copy from when Adwaita
# lacks a glyph (docs/icons.md). Files land in clients/linux/icons/idk/ under their upstream names,
# untouched, so a re-run at the same commit is a no-op and a diff after moving the pin is upstream's
# own change. Naming one in clients/linux/icons/mailcal.gresource.xml is what makes it drawable.

# Bash 5 or newer, like every script in this tree (AGENTS.md, "Building & verifying").
if [[ ${BASH_VERSION%%.*} -lt 5 ]]; then
  echo "error: ${0##*/} needs bash 5 or newer, and got ${BASH_VERSION:-no bash at all}" >&2
  echo "       macOS ships bash 3.2 as /bin/bash: \`brew install bash\` puts 5 ahead of it" >&2
  exit 1
fi

set -euo pipefail

# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

# The kit has no releases; a commit is the only fixed point. Move it deliberately, then read the diff.
IDK_COMMIT="e5857f6d796a96571a61030db40f35bfb8815a94"
IDK_RAW="https://gitlab.gnome.org/Teams/Design/icon-development-kit/-/raw/$IDK_COMMIT/icons"
DEST="$REPO_ROOT/clients/linux/icons/idk"

require_cmd curl

names=("$@")
if ((${#names[@]} == 0)); then
  for file in "$DEST"/*.svg; do
    [[ -e "$file" ]] || die "nothing vendored yet: name the icons to fetch"
    name="${file##*/}"
    names+=("${name%.svg}")
  done
fi

mkdir -p "$DEST"
for name in "${names[@]}"; do
  [[ "$name" =~ ^[a-z0-9-]+$ ]] || die "not an icon-development-kit name: $name"
  curl --fail --silent --show-error --location "$IDK_RAW/$name.svg" --output "$DEST/$name.svg" ||
    die "icon-development-kit has no $name.svg at $IDK_COMMIT"
  info "$name.svg"
done
