#!/usr/bin/env bash
# Assert that every bundle-shaped directory inside a built app really is a bundle, and, with
# --forbid, that a name is absent from the payload altogether.
#
#     check-nested-bundles.sh <app-dir> [--forbid <name>]...
#
# It exists because of the 0.3.0 iOS upload, which App Store Connect rejected twice over one empty
# directory: ITMS-90207 ("Payload/AllodiaMail.app/Library/Helpers/allodia-mcp.app does not contain
# a bundle executable") and ITMS-90036 ("the Info.plist file for … is missing or could not be
# read"). Nothing had copied a relay into the iOS build, the copy phase exits on its first line
# when PLATFORM_NAME is not macosx. Xcode built the tree by itself, because the phase DECLARED an
# output path inside the product and Xcode creates an output's directories before it runs the
# phase. So the app shipped a nested .app made of four empty folders.
#
# Every gate we had was green over it, and for one reason: they all read *code*. `codesign --verify
# --deep --strict` walks signed nested code, and empty directories are not code; the cert-consistency
# and entitlement gates iterate the same list. A check that only ever looks at what is signed cannot
# see a bundle with nothing in it, so this one looks at the shape of the payload instead.
#
# Kept out of package.sh so it can be exercised against fixture trees with no Xcode, no signing and
# no archive: scripts/dev/tests/test_check_nested_bundles.py. A release gate nobody can run is a
# release gate that rots.
set -euo pipefail

fail() { echo "error: $*" >&2; exit 1; }

# Read CFBundleExecutable from an Info.plist. python3's plistlib rather than
# `/usr/libexec/PlistBuddy`, because PlistBuddy exists on macOS and nowhere else, and this gate's
# whole point is that its unit tests run against fixture trees on any machine. They did not: off
# macOS the read returned empty, so a *valid* nested bundle was reported as "NO CFBundleExecutable"
# and the suite failed on Windows and on CI's ubuntu runner alike (it has never run there, the
# gate landed after CI was switched off). One path on every OS, so the tests exercise what a
# release runs. `package.sh`, the only caller, already requires python3, so the release path gains
# no dependency; plistlib reads binary and XML plists, as PlistBuddy did. A missing key, an
# unreadable file and a corrupt plist all yield the empty string, exactly as before.
read_bundle_executable() {
  python3 - "$1" 2>/dev/null <<'PY' || true
import plistlib, sys

try:
    with open(sys.argv[1], "rb") as handle:
        print(plistlib.load(handle).get("CFBundleExecutable", ""))
except Exception:
    pass
PY
}

APP=""
FORBID=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --forbid) FORBID+=("${2:?--forbid needs a name}"); shift 2 ;;
    -h|--help) sed -n '2,6p' "$0"; exit 0 ;;
    -*) fail "unknown option: $1" ;;
    *) APP="$1"; shift ;;
  esac
done
[[ -n "$APP" ]] || fail "usage: $(basename "$0") <app-dir> [--forbid <name>]..."
[[ -d "$APP" ]] || fail "$APP is not a directory."

# A nested executable bundle in either layout: macOS puts its parts under Contents/, iOS puts them
# at the top. Both are checked the same way, since the invariant, an Info.plist naming an
# executable that is actually there, is the same one App Store delivery reads.
bad=0
while IFS= read -r bundle; do
  rel="${bundle#"$APP"/}"
  if [[ -f "$bundle/Contents/Info.plist" ]]; then
    plist="$bundle/Contents/Info.plist"; exedir="$bundle/Contents/MacOS"
  elif [[ -f "$bundle/Info.plist" ]]; then
    plist="$bundle/Info.plist"; exedir="$bundle"
  else
    echo "    NO Info.plist: $rel" >&2; bad=1; continue
  fi
  exe="$(read_bundle_executable "$plist")"
  if [[ -z "$exe" ]]; then
    echo "    NO CFBundleExecutable: $rel" >&2; bad=1; continue
  fi
  [[ -f "$exedir/$exe" ]] || { echo "    MISSING executable '$exe': $rel" >&2; bad=1; }
done < <(find "$APP" \( -name '*.app' -o -name '*.appex' -o -name '*.xpc' \) -type d ! -path "$APP")

[[ "$bad" -eq 0 ]] || fail "the payload contains a directory shaped like a bundle that is not one.
       App Store delivery rejects this with ITMS-90207 / ITMS-90036. An empty nested .app is
       usually a build phase that declared an output inside the product on a platform where it
       copies nothing, see clients/apple/project.yml → MCP_RELAY_OUTPUT."

for name in ${FORBID[@]+"${FORBID[@]}"}; do
  found="$(find "$APP" -name "*${name}*" | head -5)"
  if [[ -n "$found" ]]; then
    echo "$found" | sed 's|^|    |' >&2
    fail "'$name' must not be in this payload, and the paths above are.
       (The MCP relay is macOS-only, docs/mcp.md, so an iOS build carries no trace of it.)"
  fi
done

echo "    payload shape OK$([[ ${#FORBID[@]} -gt 0 ]] && echo " · no ${FORBID[*]}")"
