#!/usr/bin/env bash
# Production packaging for the macOS app, the release/Store twin of Scripts/build-and-run.sh, and
# the macOS mirror of clients/windows/package.ps1. Native Xcode tooling throughout (xcodebuild
# archive / -exportArchive, notarytool, stapler, hdiutil). Two flows:
#
#   Scripts/package.sh                  # Flow A (default): notarized Developer-ID .dmg, install on
#                                       #   any Mac (including yours). This is the VERIFIED flow.
#   Scripts/package.sh --app-store      # Flow B: Apple-Distribution .pkg for the macOS App Store.
#                                       #   Signs the whole app tree by hand (persistent certs), the
#                                       #   automatic-signing export skipped a nested SPM bundle and
#                                       #   Apple rejected it (ITMS-90284); see README + the gate.
#   Scripts/package.sh --ios-app-store  # Flow C: App Store .ipa for iOS/iPadOS. Archives for a real
#                                       #   device, then -exportArchive re-signs to Apple Distribution
#                                       #   (automatic; iOS re-signs nested code correctly, so no
#                                       #   manual pass). Upload with Transporter/altool.
#   Scripts/package.sh --ios-device     # Flow D: the SAME Release archive, exported for development
#                                       #   signing, an .ipa you can install on your own iPhone or
#                                       #   iPad. Flow C's .ipa cannot be: an App Store profile lists
#                                       #   no devices, so iOS refuses to launch it.
#
# Each Store flow copies its finished artifact to build/release-<VERSION>/ before exiting. Every run
# wipes build/package first, so without that a second flow destroys the first one's artifact, and
# a release builds both back to back.
#
#   Scripts/package.sh --no-notarize    # Flow A without the notary round-trip (fast pipeline check)
#   Scripts/package.sh --no-core        # skip rebuilding the release Rust XCFramework
#   Scripts/package.sh --version 1.0.1  # stamp the marketing version (CFBundleShortVersionString)
#
# Signing config is read from the git-ignored clients/apple/signing.local.sh (copy the .example and
# fill it in), nothing secret is committed. One-time cert/notary setup: clients/apple/README.md.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"       # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"              # repo root (worktree)

# Every registration this build uses must be present, or the compile fails naming the missing ones
# (crates/mailcal-oauth/build.rs). A build without them is legitimate, it simply offers no Google
# or Microsoft sign-in, which is exactly why a *shipped* one has to be refused rather than
# produced: it would look correct everywhere except in front of a user.
export MAILCAL_REQUIRE_INJECTED_CONFIG=1

# The app's name and bundle id, which project.yml takes from the environment (docs/branding.md).
# XcodeGen leaves the placeholder text behind when they are absent, so this has to happen before
# the project is generated, and `brand_assert_expanded` below is what proves it did.
# shellcheck source=scripts/dev/brand.sh
. "$ROOT/scripts/dev/brand.sh"
brand_load

PROJECT="$HERE/AllodiaMail.xcodeproj"
SCHEME="AllodiaMail"
APP_NAME="AllodiaMail.app"
# The nested MCP relay bundle, relative to the .app. Must match McpEndpoint.relayBundlePath (which
# builds the config snippet the user pastes) and the postBuildScripts phase in project.yml that
# creates it, three places, one path, and a drift means the snippet names something that is not
# there.
RELAY_BUNDLE_PATH="Contents/Library/Helpers/allodia-mcp.app"
# The mounted disk image's name, so the Finder window a user drags from says the product's
# own name. Under HFS+'s 27-character volume limit at 23.
VOLNAME="Allodia Mail & Calendar"
BUILD="$HERE/build/package"                    # archive/export/dmg staging (git-ignored)
DMG="$HERE/build/AllodiaMail.dmg"              # the final direct-distribution artifact
CONFIG="$HERE/signing.local.sh"

FLOW=developer-id
NOTARIZE=1
BUILD_CORE=1
VERSION=""

usage() {
  cat <<'USAGE'
Usage: Scripts/package.sh [options]
  (no flags)        Flow A: notarized Developer-ID .dmg (install on any Mac).
  --app-store       Flow B: Apple-Distribution .pkg for the macOS App Store (manual dist signing).
  --ios-app-store   Flow C: App Store .ipa for iOS/iPadOS (automatic distribution signing).
  --ios-device      Flow D: installable Release .ipa for your own iPhone/iPad (development signing).
  --no-notarize     Flow A without notarization (fast pipeline check; not Gatekeeper-valid).
  --no-core         Skip rebuilding the release Rust XCFramework.
  --version <x.y.z> Stamp the marketing version.
  -h, --help        Show this help.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --developer-id) FLOW=developer-id; shift ;;
    --app-store) FLOW=app-store; shift ;;
    --ios-app-store) FLOW=ios-app-store; shift ;;
    --ios-device) FLOW=ios-device; shift ;;
    --no-notarize) NOTARIZE=0; shift ;;
    --no-core) BUILD_CORE=0; shift ;;
    --version) VERSION="${2:?missing value for --version}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "package: unknown option '$1'" >&2; usage >&2; exit 2 ;;
  esac
done

fail() { echo "error: $*" >&2; exit 1; }

# Leaf "Authority=" value for a signed path. Reads ALL of codesign's output (no early `exit` that
# would close the pipe and SIGPIPE codesign, which, under `set -o pipefail`, aborts the script).
first_authority() {
  codesign -dvv "$1" 2>&1 | awk -F'Authority=' '/^Authority=/ && a == "" { a = $2 } END { print a }'
}

# Nested code splits in TWO, and the split is the whole point, the two halves take OPPOSITE
# entitlement rules, so one list cannot serve both.
#
#   * A LIBRARY (framework / dylib / resource bundle) must be signed with NO entitlements.
#   * An EXECUTABLE (a nested .app, or a bare helper binary) must, on the Store, carry
#     com.apple.security.app-sandbox, App Store Connect rejects the upload otherwise
#     (ITMS-90296), which is exactly how this was found: the relay was signed by the same rule as
#     the SPM resource bundle, the ITMS-90284 cert-consistency gate below reported CLEAN because
#     the CERT did match, and the upload failed on the entitlement instead.
#
# So the gate that exists is not enough on its own: it compares who signed each item and never
# what they were signed WITH. `assert_store_entitlements` is the other axis, and both read these
# same two functions so the signer and the gates cannot disagree about what is what.
#
# AGENTS.md: "if a check cannot fail, it is not a check."

# Frameworks, dylibs, resource bundles, deepest first. Never entitled.
nested_library_items() {
  find "$1" -depth \( -name '*.bundle' -o -name '*.framework' -o -name '*.dylib' \)
}

# Things that RUN: a nested helper .app (the MCP relay, docs/mcp.md) and any bare Mach-O under
# Contents/MacOS or Contents/Helpers, minus the app's own main executable (signing the app signs
# that, and re-signing it separately would be undone a moment later anyway). `! -path "$app"`
# matters: the app we are signing is itself a *.app and would otherwise match its own predicate.
nested_executable_items() {
  local app="$1"
  local main_exe
  main_exe="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$app/Contents/Info.plist" 2>/dev/null || true)"
  find "$app" -depth -name '*.app' ! -path "$app"
  for dir in "$app/Contents/MacOS" "$app/Contents/Helpers"; do
    [[ -d "$dir" ]] || continue
    find "$dir" -type f -perm -111 ! -name '*.dSYM' | while IFS= read -r exe; do
      [[ -n "$main_exe" && "$(basename "$exe")" == "$main_exe" ]] && continue
      echo "$exe"
    done
  done
}

# Everything that carries its own signature, libraries first so a nested .app is sealed after
# anything inside it. What the cert-consistency (ITMS-90284) gate walks.
nested_code_items() {
  nested_library_items "$1"
  nested_executable_items "$1"
}

# An entitlements file about to be handed to codesign holds no build-setting token. codesign does
# not expand one, it signs `$(PRODUCT_BUNDLE_IDENTIFIER)` in as literal text, and the result is an
# app whose keychain group and app group name nothing, which fails at runtime rather than here.
assert_entitlements_resolved() {
  local file="$1" unresolved
  # `grep -o` exits 1 when it matches nothing, and here that is the case which must PASS. Under
  # `set -o pipefail` the pipeline carries that status out to the assignment, and `set -e` then
  # ends the run, with no message, because a grep that matched nothing has nothing to say.
  unresolved="$(grep -o '\$([A-Za-z_][A-Za-z0-9_]*)' "$file" | sort -u | tr '\n' ' ')" || true
  [[ -z "$unresolved" ]] || fail "$(basename "$file") still holds unexpanded build settings: $unresolved
       codesign signs these in literally. Resolve them where the file is materialized."
}

# The ITMS-90296 gate: on the Store, every nested EXECUTABLE carries the sandbox entitlement, and
# no library carries entitlements at all. Runs on the signed tree, so it reads what was actually
# applied rather than what we meant to apply.
assert_store_entitlements() {
  local app="$1" bad=0 ents
  while IFS= read -r item; do
    ents="$(codesign -d --entitlements - --xml "$item" 2>/dev/null | plutil -convert json -o - - 2>/dev/null || true)"
    if [[ "$ents" != *'"com.apple.security.app-sandbox":true'* ]]; then
      echo "    NOT SANDBOXED: ${item#"$app"/}" >&2
      bad=1
    elif [[ "$ents" != *'"com.apple.security.application-groups"'* ]]; then
      # Not fatal to the upload, but fatal to the feature: the relay launches and then cannot
      # reach the socket, which a user reads as "the app is not running".
      echo "    warning: ${item#"$app"/} has no application-groups, it will not reach the app" >&2
    fi
  done < <(nested_executable_items "$app")
  [[ "$bad" -eq 0 ]] || fail "a nested executable is missing com.apple.security.app-sandbox, App Store Connect rejects this with ITMS-90296.
       Every executable in a Store submission must be sandboxed; see App/AllodiaMailMcpHelper.appstore.entitlements."
  while IFS= read -r item; do
    if codesign -d --entitlements - --xml "$item" 2>/dev/null | grep -q 'app-sandbox'; then
      echo "    ENTITLED LIBRARY: ${item#"$app"/}" >&2
      bad=1
    fi
  done < <(nested_library_items "$app")
  [[ "$bad" -eq 0 ]] || fail "a nested framework/bundle carries entitlements, libraries must be signed without them."
}

# The MCP relay must be IN the packaged app, and this asserts it on the built artifact rather than
# trusting the build to have put it there.
#
# It is a gate and not a build-phase check because the failure it exists for is the build phase
# being *absent*: the xcodegen `postBuildScripts` entry was dropped whole in a merge conflict
# resolution, every build stayed green (the copy is a script phase, so nothing references it), and
# a notarized Developer-ID build shipped with no relay at all. The app launched, Settings →
# Advanced rendered, and the config snippet it offered fell back to a bare `allodia-mcp` on PATH:
# which an MCP client spawns without a shell and cannot resolve, so the only symptom was "Failed
# to spawn process: No such file or directory" inside the assistant, pointing at nobody.
#
# A check inside the phase cannot catch the phase not existing. This one reads the app.
assert_relay_present() {
  local app="$1"
  local bundle="$app/$RELAY_BUNDLE_PATH"
  local relay="$bundle/Contents/MacOS/allodia-mcp"
  [[ -f "$relay" ]] \
    || fail "the packaged app has no $RELAY_BUNDLE_PATH, the relay copy phase did not run (see clients/apple/project.yml → postBuildScripts). MCP would be dead in this build."
  [[ -x "$relay" ]] || fail "$relay is present but not executable."
  # It is the relay, not something else that happens to sit there under that name.
  /usr/bin/file -b "$relay" | grep -q 'Mach-O' \
    || fail "$relay is not a Mach-O executable."
  # The bundle half is not cosmetic: without a CFBundleIdentifier the sandbox has no container to
  # attach and a Store build's relay dies in libsecinit before main(). A bundle that lost its
  # Info.plist would still pass every check above.
  [[ -f "$bundle/Contents/Info.plist" ]] \
    || fail "$bundle has no Info.plist, a sandboxed relay with no bundle id cannot launch at all."
  /usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist" >/dev/null 2>&1 \
    || fail "$bundle/Contents/Info.plist declares no CFBundleIdentifier."
}

# The shipped binary still carries the symbol table its crash handler needs.
#
# `STRIP_STYLE: debugging` in project.yml is what keeps it, and nothing else notices if that goes:
# the app builds, runs and signs identically, and only a crash months later reads
# `AllodiaMail + 1852` instead of a function name. Xcode's default for an application is `all`, so
# this is one build setting away from silently reverting on any edit to that file.
#
# The threshold is deliberately loose. A stripped binary keeps a couple of thousand symbols for
# dynamic linking, an unstripped one keeps six figures, and nothing sits between, so any number
# in the tens of thousands distinguishes them without pinning a count that moves with every commit.
assert_symbols_kept() {
  local app="$1" binary count
  # An app keeps its executable in `Contents/MacOS` on macOS and at the top level on iOS, so one of
  # these two paths is always absent and `find` ends non-zero for it, which `set -o pipefail` would
  # carry out to the assignment and `set -e` would turn into a silent end to the run. The path that
  # does exist is still searched and still prints, and the emptiness check below reports a real miss.
  binary="$(/usr/bin/find "$app/Contents/MacOS" "$app" -maxdepth 1 -type f -perm -u+x 2>/dev/null | head -1)" || true
  [[ -n "$binary" ]] || fail "no executable found inside $app to check for symbols."
  # Say so rather than die of it. `nm` is macOS's own and is always here on the machine that
  # packages, but the count below is a command substitution under `set -e` with `pipefail`, so a
  # `nm` that cannot run ends the entire packaging run at this line with **no diagnostic at all**,
  # exit 127 and nothing on either stream. That is the one outcome this guard exists to rule out,
  # and it is what a bash that is not macOS's does with it.
  [[ -x /usr/bin/nm ]] \
    || fail "/usr/bin/nm is not on this machine, so the packaged binary's symbols cannot be counted."
  count="$(/usr/bin/nm "$binary" 2>/dev/null | /usr/bin/wc -l | /usr/bin/tr -d ' ')"
  [[ "$count" -ge 20000 ]] \
    || fail "the packaged binary carries only $count symbols, STRIP_STYLE is back to Apple's 'all' default, and a crash stack in the user's log will name no function (clients/apple/project.yml, docs/logging.md)."
  echo "    symbols kept: $count (a crash stack can name functions)"
}

# ---- Signing config (git-ignored; never committed) --------------------------------------------
[[ -f "$CONFIG" ]] || fail "$CONFIG not found.
       Copy the template and fill it in:  (cd clients/apple && cp signing.local.sh.example signing.local.sh)"
# shellcheck source=/dev/null
source "$CONFIG"
: "${DEVELOPMENT_TEAM:=}"
: "${DEVELOPER_ID_IDENTITY:=}"
: "${NOTARY_KEYCHAIN_PROFILE:=}"
: "${APPLE_DISTRIBUTION_IDENTITY:=}"
: "${MAC_INSTALLER_IDENTITY:=}"
: "${MAS_PROVISIONING_PROFILE:=}"
: "${ASC_API_KEY_ID:=}"
: "${ASC_API_ISSUER_ID:=}"

# True when $CONFIG carries a usable App Store Connect API key id + issuer id. BOTH must be present,
# and neither may still be the template's REPLACE_* text: a command line that looks ready to paste
# but carries a placeholder is worse than one that admits it has nothing, because the first thing it
# does is fail against Apple with an error about credentials rather than about the copy-paste.
asc_ids_ready() {
  [[ -n "$ASC_API_KEY_ID" && -n "$ASC_API_ISSUER_ID" ]] || return 1
  case "$ASC_API_KEY_ID$ASC_API_ISSUER_ID" in *REPLACE*) return 1 ;; esac
  return 0
}

# How to upload a finished Store artifact: `upload_hint <artifact> <macos|ios>`.
#
# The two ids are IDENTIFIERS, not secrets, what authenticates is the private key file
# `AuthKey_<KEY_ID>.p8`, which altool finds BY KEY ID in ~/.appstoreconnect/private_keys (or
# ./private_keys, ~/private_keys, ~/.private_keys) without being told where it is. That is why they
# can live in $CONFIG under the same rule as the Team ID and the cert names: the file stays free of
# secrets, and the .p8 stays where Apple's tooling already looks for it.
#
# Without them this prints exactly what it printed before, placeholders, so a fresh clone, and CI,
# read no differently.
upload_hint() {
  local artifact="$1" platform="$2"
  echo "    Upload with Transporter (Apple's app), or:"
  if asc_ids_ready; then
    echo "      xcrun altool --upload-app -f \"$artifact\" -t $platform --apiKey $ASC_API_KEY_ID --apiIssuer $ASC_API_ISSUER_ID"
  else
    echo "      xcrun altool --upload-app -f \"$artifact\" -t $platform --apiKey <KEY_ID> --apiIssuer <ISSUER_UUID>"
    echo "      (set ASC_API_KEY_ID + ASC_API_ISSUER_ID in $CONFIG and this prints ready to paste)"
  fi
}

# ---- Preflight (fail early + helpfully, mirroring package.ps1's discipline) --------------------
case "$DEVELOPMENT_TEAM" in
  "" | *REPLACE* | TEAMID*) fail "DEVELOPMENT_TEAM is unset or still a placeholder in $CONFIG." ;;
esac
command -v xcodegen >/dev/null 2>&1 || fail "xcodegen not on PATH (brew install xcodegen)."

case "$FLOW" in
developer-id)
  [[ -n "$DEVELOPER_ID_IDENTITY" ]] || fail "DEVELOPER_ID_IDENTITY is unset in $CONFIG."
  # Require an actual 'Developer ID Application' cert in the keychain, not just any codesigning
  # identity, otherwise the archive succeeds and the EXPORT fails late with "No signing certificate
  # 'Developer ID Application' found", wasting the whole archive.
  if ! security find-identity -v -p codesigning 2>/dev/null | grep -F -- "$DEVELOPER_ID_IDENTITY" | grep -q "Developer ID Application"; then
    fail "No 'Developer ID Application' certificate matching '$DEVELOPER_ID_IDENTITY' is in your keychain.
       Create one (once): Xcode ▸ Settings ▸ Accounts ▸ your team ▸ Manage Certificates… ▸ + ▸
       'Developer ID Application'. Then set DEVELOPER_ID_IDENTITY in $CONFIG to its EXACT name from:
         security find-identity -v -p codesigning
       (Full steps: clients/apple/README.md.)"
  fi
  if [[ "$NOTARIZE" -eq 1 && -z "$NOTARY_KEYCHAIN_PROFILE" ]]; then
    fail "NOTARY_KEYCHAIN_PROFILE is unset (or re-run with --no-notarize).
       One-time:  xcrun notarytool store-credentials <name> \\
                    --apple-id <you@example.com> --team-id $DEVELOPMENT_TEAM \\
                    --password <app-specific-password>"
  fi
  ;;
app-store)
  # macOS App Store needs TWO persistent certs (mirrors Flow A's Developer-ID check). We sign the
  # whole app ourselves rather than let Xcode's automatic (cloud-managed) signing do it, because that
  # path re-signs the app but SKIPS nested SPM resource bundles, App Store Connect rejected the
  # first upload with ITMS-90284, and the transient cloud cert can't be reused to fix the bundle.
  [[ -n "$APPLE_DISTRIBUTION_IDENTITY" ]] || fail "APPLE_DISTRIBUTION_IDENTITY is unset in $CONFIG (see clients/apple/README.md → 'Packaging for the Mac App Store')."
  [[ -n "$MAC_INSTALLER_IDENTITY" ]] || fail "MAC_INSTALLER_IDENTITY is unset in $CONFIG."
  if ! security find-identity -v -p codesigning 2>/dev/null | grep -F -- "$APPLE_DISTRIBUTION_IDENTITY" | grep -q "Apple Distribution"; then
    fail "No 'Apple Distribution' certificate matching '$APPLE_DISTRIBUTION_IDENTITY' is in your keychain.
       Create one (once): Xcode ▸ Settings ▸ Accounts ▸ your team ▸ Manage Certificates… ▸ + ▸
       'Apple Distribution'. Then set APPLE_DISTRIBUTION_IDENTITY in $CONFIG to its EXACT name from:
         security find-identity -v -p codesigning
       (Full steps: clients/apple/README.md.)"
  fi
  # Installer identities don't appear under the codesigning policy, check the identity list and, as
  # a fallback, the raw certificate (productbuild is the real gate and fails loudly if the key is
  # absent).
  if ! security find-identity -v 2>/dev/null | grep -qF -- "$MAC_INSTALLER_IDENTITY" \
     && ! security find-certificate -c "$MAC_INSTALLER_IDENTITY" >/dev/null 2>&1; then
    fail "No installer certificate matching '$MAC_INSTALLER_IDENTITY' is in your keychain.
       Create one (once): developer.apple.com ▸ Certificates ▸ + ▸ 'Mac Installer Distribution',
       download and double-click to install. Then set MAC_INSTALLER_IDENTITY in $CONFIG to its EXACT
       name (e.g. '3rd Party Mac Developer Installer: <Name> (<TEAMID>)') from:
         security find-identity -v
       (Full steps: clients/apple/README.md.)"
  fi
  ;;
ios-device | ios-app-store)
  # iOS/iPadOS App Store uses AUTOMATIC signing, `xcodebuild -exportArchive` fetches/creates the
  # Apple Distribution cert and the App Store provisioning profile at export time
  # (-allowProvisioningUpdates), so there is no persistent cert to pre-check here beyond the team
  # (asserted above). An Apple account must be signed into Xcode, and the team needs at least one
  # registered device for the DEVELOPMENT-signed archive (same as Flow B; see clients/apple/README.md).
  # xcodebuild surfaces both with its own clear errors, so we don't second-guess them here.
  :
  ;;
esac

echo "==> Flow: $FLOW$([[ "$FLOW" == developer-id && "$NOTARIZE" -eq 0 ]] && echo ' (no notarization)')  ·  team: $DEVELOPMENT_TEAM"

# ---- Front half: release core + fresh project --------------------------------------------------
if [[ "$BUILD_CORE" -eq 1 ]]; then
  # The macOS flows never link the iOS device slice, so they skip it (--no-device). Flows C and D
  # both archive for a real device (generic/platform=iOS), so they MUST build the aarch64-apple-ios
  # slice.
  CORE_ARGS=(--release)
  [[ "$FLOW" == ios-app-store || "$FLOW" == ios-device ]] || CORE_ARGS+=(--no-device)
  echo "==> Building the release Rust XCFramework (build-core.sh ${CORE_ARGS[*]})"
  "$HERE/Scripts/build-core.sh" "${CORE_ARGS[@]}"
fi
echo "==> Regenerating the Xcode project (xcodegen)"
(cd "$HERE" && xcodegen generate >/dev/null)
# Before anything is built, let alone uploaded: a project generated without the brand loaded names
# the app `${MAILCAL_APP_NAME}` and ships under a bundle id the Store has never heard of.
brand_assert_expanded "$HERE/AllodiaMail.xcodeproj/project.pbxproj"
brand_assert_expanded "$HERE/App/Info.plist"

# Flow C: assert the iPad-multitasking Info.plist invariant BEFORE the (multi-minute) archive, not at
# the App Store delivery gate. Unless the app opts out of multitasking (UIRequiresFullScreen = true),
# App Store validation rejects the build (error 90474) unless UISupportedInterfaceOrientations~ipad
# lists ALL FOUR orientations. xcodegen has just written the generated Info.plist, so check it here:
# a rejection at delivery burns a build number and a slow round-trip.
if [[ "$FLOW" == ios-app-store ]]; then
  INFO_PLIST="$HERE/App/Info.plist"
  if [[ "$(/usr/libexec/PlistBuddy -c 'Print :UIRequiresFullScreen' "$INFO_PLIST" 2>/dev/null || echo false)" != true ]]; then
    IPAD_ORIENTS="$(/usr/libexec/PlistBuddy -c 'Print :UISupportedInterfaceOrientations~ipad' "$INFO_PLIST" 2>/dev/null || true)"
    for o in UIInterfaceOrientationPortrait UIInterfaceOrientationPortraitUpsideDown \
             UIInterfaceOrientationLandscapeLeft UIInterfaceOrientationLandscapeRight; do
      grep -q "$o" <<<"$IPAD_ORIENTS" || fail "iPad multitasking (App Store error 90474): UISupportedInterfaceOrientations~ipad is missing $o.
       A multitasking iPad app must list all four orientations, add it under the app target's
       info.properties in clients/apple/project.yml, then re-run."
    done
  fi
fi

rm -rf "$BUILD"; mkdir -p "$BUILD"
ARCHIVE="$BUILD/AllodiaMail.xcarchive"
EXPORT="$BUILD/export"
# Marketing version: the top-level /VERSION file is the single source of truth (docs/versioning.md);
# --version overrides it for a one-off build. The build number is always a fresh, monotonically
# increasing timestamp (unique per App Store upload; cosmetic for Developer ID), DOTTED, not a single
# integer: a bare YYYYMMDDHHMM (e.g. 202607191430) overflows CFBundleVersion's per-component 2^32-1
# limit, which App Store Connect rejects, so each dot-separated field stays small (2026.0719.1430).
MARKETING_VERSION_VALUE="${VERSION:-$(cat "$ROOT/VERSION")}"

# Where a finished Store artifact is kept. Deliberately OUTSIDE $BUILD, which is wiped at the start
# of every run: the two Store flows are separate invocations, so running --app-store and then
# --ios-app-store destroys the .pkg the first one just produced, silently, because the second flow
# succeeds. Both artifacts are needed together at a release, which is exactly when they are built
# back to back.
# Artifacts are named for their FLOW, not just the app: `-exportArchive` names every iOS export
# after the app, so an App Store .ipa and a device .ipa are both "Allodia Mail & Calendar.ipa" and
# the second flow to run would silently replace the first one's artifact in here.
KEEP="$HERE/build/release-$MARKETING_VERSION_VALUE"
VERSION_ARGS=(
  "MARKETING_VERSION=$MARKETING_VERSION_VALUE"
  "CURRENT_PROJECT_VERSION=$(date -u +%Y.%m%d.%H%M)"
)

# ---- Archive -----------------------------------------------------------------------------------
# Signing is overridden here (not baked into project.yml). Hardened Runtime + the macOS entitlements
# come from the Release config in project.yml; the macOS app-store flow swaps in the sandbox
# entitlements, and the iOS flow archives for a device destination.
SIGN_ARGS=("DEVELOPMENT_TEAM=$DEVELOPMENT_TEAM")
EXPORT_EXTRA=()
case "$FLOW" in
developer-id)
  SIGN_ARGS+=("CODE_SIGN_STYLE=Manual" "CODE_SIGN_IDENTITY=$DEVELOPER_ID_IDENTITY")
  EXPORT_TEMPLATE="$HERE/Scripts/ExportOptions-DeveloperID.plist"
  DESTINATION='generic/platform=macOS'
  ;;
app-store)
  # macOS App Store: the ARCHIVE is DEVELOPMENT-signed with automatic provisioning (Xcode resolves or
  # creates an Apple Development cert + a development profile for eu.allodia.mailcal via
  # -allowProvisioningUpdates). Forcing "Apple Distribution" here fails ("can't distribution-sign an
  # archive"), and the committed "-" can't ad-hoc-sign the sandbox entitlements' keychain-access-
  # group (a restricted entitlement), so Apple Development is the identity. DISTRIBUTION signing is
  # NOT done by `xcodebuild -exportArchive`, that re-signs the app but skips nested SPM resource
  # bundles (ITMS-90284), but by the explicit manual pass further down, against a Mac App Store
  # profile that lists our Apple Distribution cert (a dev-signed archive does NOT refresh that
  # profile, create it explicitly; see the resolver below + README). MACOS_ENTITLEMENTS swaps the
  # app target onto the sandbox set (see project.yml) without leaking a global CODE_SIGN_ENTITLEMENTS
  # onto the package targets.
  SIGN_ARGS+=("CODE_SIGN_STYLE=Automatic" "CODE_SIGN_IDENTITY=Apple Development" \
              "MACOS_ENTITLEMENTS=App/AllodiaMail.appstore.entitlements")
  EXPORT_EXTRA=(-allowProvisioningUpdates)   # resolve/create the dev profile for the archive
  DESTINATION='generic/platform=macOS'
  ;;
ios-device | ios-app-store)
  # iOS/iPadOS App Store: like the macOS Store archive, DEVELOPMENT-signed with automatic
  # provisioning (an Apple Development cert + a development profile resolved via
  # -allowProvisioningUpdates); `xcodebuild -exportArchive` re-signs to Apple Distribution against an
  # auto-managed App Store profile. No MACOS_ENTITLEMENTS / sandbox swap and no Hardened Runtime, iOS
  # ignores both, and the iOS keychain entitlements come from CODE_SIGN_ENTITLEMENTS[sdk=iphoneos*]
  # (App/AllodiaMail.entitlements) in project.yml. The device destination pulls the ios-arm64 slice.
  SIGN_ARGS+=("CODE_SIGN_STYLE=Automatic" "CODE_SIGN_IDENTITY=Apple Development")
  EXPORT_EXTRA=(-allowProvisioningUpdates)   # resolve/create the dev profile for the archive
  DESTINATION='generic/platform=iOS'
  ;;
esac

echo "==> Archiving ($SCHEME, Release, arm64, $DESTINATION)"
# ARCHS=arm64 is a command-line override, so it applies to EVERY target in the build, including the
# MailcalKit SPM package targets, which don't inherit the app target's EXCLUDED_ARCHS and would
# otherwise compile x86_64 too during a Release archive (ONLY_ACTIVE_ARCH is off for Release). Both
# platforms are arm64 only (Apple-silicon Mac / arm64 iPhone+iPad), matching the XCFramework's slices.
xcodebuild \
  -project "$PROJECT" \
  -scheme "$SCHEME" \
  -configuration Release \
  -destination "$DESTINATION" \
  -archivePath "$ARCHIVE" \
  ARCHS=arm64 \
  "${SIGN_ARGS[@]}" \
  "${VERSION_ARGS[@]}" \
  ${EXPORT_EXTRA[@]+"${EXPORT_EXTRA[@]}"} \
  archive

# ---- Flow D: iOS/iPadOS device export (.ipa you can install) -----------------------------------
# The SAME Release archive as Flow C, only the export signing differs. A development export embeds
# a profile carrying the team's registered devices, which is precisely what makes the result
# installable; Flow C's App Store profile lists none, so iOS refuses to launch it.
if [[ "$FLOW" == ios-device ]]; then
  EXPORT_PLIST="$BUILD/ExportOptions.plist"
  sed "s/__TEAM_ID__/$DEVELOPMENT_TEAM/g" "$HERE/Scripts/ExportOptions-Device-iOS.plist" >"$EXPORT_PLIST"

  echo "==> iOS device: exporting the archive (development signing)"
  rm -rf "$EXPORT"; mkdir -p "$EXPORT"
  xcodebuild -exportArchive \
    -archivePath "$ARCHIVE" \
    -exportOptionsPlist "$EXPORT_PLIST" \
    -exportPath "$EXPORT" \
    -allowProvisioningUpdates

  IPA="$(/usr/bin/find "$EXPORT" -maxdepth 1 -name '*.ipa' | head -1)"
  [[ -n "$IPA" && -f "$IPA" ]] || fail "export produced no .ipa, check the log above."

  # The gate that decides whether this build was worth making. Counting the profile's devices is
  # NOT it: a team whose only registered device is a Mac produces an iOS profile carrying that one
  # UDID, so "1 device" reads as success and the install then fails with 0xe8008012. Measured here.
  # So compare UDIDs against what is actually plugged in.
  echo "==> Verifying the exported app is installable (profile covers a connected device)"
  rm -rf "$BUILD/ipa-verify"; mkdir -p "$BUILD/ipa-verify"
  (cd "$BUILD/ipa-verify" && unzip -qo "$IPA")
  APP="$BUILD/ipa-verify/Payload/$(basename "$(/usr/bin/find "$BUILD/ipa-verify/Payload" -maxdepth 1 -name '*.app' | head -1)")"
  [[ -d "$APP" ]] || fail "the .ipa has no Payload/*.app"
  PROFILE="$APP/embedded.mobileprovision"
  [[ -f "$PROFILE" ]] || fail "the exported app embeds no provisioning profile."
  security cms -D -i "$PROFILE" 2>/dev/null >"$BUILD/profile.plist"
  PROFILE_UDIDS="$(/usr/libexec/PlistBuddy -c 'Print :ProvisionedDevices' "$BUILD/profile.plist" 2>/dev/null |
    /usr/bin/sed -n 's/^ *\([0-9A-Fa-f][0-9A-Fa-f-]*\)$/\1/p' || true)"
  [[ -n "$PROFILE_UDIDS" ]] ||
    fail "the embedded profile lists no ProvisionedDevices, this is a distribution profile, so the
       .ipa will not install."

  # The connected iOS devices, if any. `devicectl` reports the hardware UDID, which is the same
  # identifier a profile lists, its own `Identifier` column is a CoreDevice UUID and is NOT.
  CONNECTED="$(xcrun devicectl list devices --json-output "$BUILD/devices.json" >/dev/null 2>&1 &&
    /usr/bin/python3 -c "
import json, sys
devices = json.load(open(sys.argv[1]))['result']['devices']
for d in devices:
    if d.get('connectionProperties', {}).get('tunnelState') != 'unavailable':
        print(d['hardwareProperties']['udid'])
" "$BUILD/devices.json" 2>/dev/null || true)"

  if [[ -z "$CONNECTED" ]]; then
    warn "no iOS device is connected, so this cannot verify the profile covers the one you will
         install to. It lists: $(tr '\n' ' ' <<<"$PROFILE_UDIDS")"
  else
    COVERED=""
    while read -r udid; do
      [[ -n "$udid" ]] || continue
      grep -qix "$udid" <<<"$PROFILE_UDIDS" && COVERED="$COVERED $udid"
    done <<<"$CONNECTED"
    [[ -n "$COVERED" ]] || fail "the embedded profile covers none of the connected devices, so the
       install fails with 0xe8008012 ('This provisioning profile cannot be installed on this
       device').
       connected: $(tr '\n' ' ' <<<"$CONNECTED")
       profile:   $(tr '\n' ' ' <<<"$PROFILE_UDIDS")
       Register it, then re-run this flow so the export picks up a profile that includes it:
         asc devices register --name '<name>' --udid '<udid>' --platform IOS"
    echo "    covers connected device(s):$COVERED"
  fi
  DEVICE_COUNT="$(grep -c . <<<"$PROFILE_UDIDS" || true)"
  codesign --verify --deep --strict "$APP" || fail "codesign --verify failed on the exported app."

  mkdir -p "$KEEP"
  KEPT="$KEEP/AllodiaMail-$MARKETING_VERSION_VALUE-Device.ipa"
  cp "$IPA" "$KEPT"

  echo ""
  echo "==> Installable iOS Release build ready: $KEPT"
  echo "    Release configuration, signed for $DEVICE_COUNT registered device(s)."
  echo "    Install it by dragging the .ipa onto your device in Xcode > Window > Devices and"
  echo "    Simulators, or:"
  echo "      xcrun devicectl device install app --device <udid> \"$KEPT\""
  echo "    Push uses the APNs SANDBOX here (development entitlements), for real notification"
  echo "    delivery, install the TestFlight build instead. See clients/apple/README.md."
  exit 0
fi

# ---- Flow C: iOS/iPadOS App Store export (.ipa) ------------------------------------------------
# Unlike macOS Flow B, iOS `xcodebuild -exportArchive` re-signs nested code correctly, so the export
# itself does the Apple-Distribution signing (automatic, against an auto-managed App Store profile):
# there is no manual re-sign pass. We still ASSERT the invariants App Store delivery checks, locally,
# BEFORE upload: the app is Apple-Distribution-signed, its embedded profile is a distribution (App
# Store) profile, and the full signature verifies deep. A rejection at delivery costs a build number
# and a slow round-trip, so the gate must be able to fail here first.
if [[ "$FLOW" == ios-app-store ]]; then
  EXPORT_PLIST="$BUILD/ExportOptions.plist"
  sed "s/__TEAM_ID__/$DEVELOPMENT_TEAM/g" "$HERE/Scripts/ExportOptions-AppStore-iOS.plist" >"$EXPORT_PLIST"

  echo "==> iOS App Store: exporting the archive (app-store-connect, automatic distribution signing)"
  rm -rf "$EXPORT"; mkdir -p "$EXPORT"
  xcodebuild -exportArchive \
    -archivePath "$ARCHIVE" \
    -exportOptionsPlist "$EXPORT_PLIST" \
    -exportPath "$EXPORT" \
    -allowProvisioningUpdates

  IPA="$(/usr/bin/find "$EXPORT" -maxdepth 1 -name '*.ipa' | head -1)"
  [[ -n "$IPA" && -f "$IPA" ]] || fail "export produced no .ipa, check the log above."

  echo "==> Verifying the exported app before upload (Apple Distribution · App Store profile · deep signature)"
  VERIFY_DIR="$BUILD/ipa-verify"
  rm -rf "$VERIFY_DIR"; mkdir -p "$VERIFY_DIR"
  /usr/bin/unzip -q "$IPA" -d "$VERIFY_DIR"
  APP="$(/usr/bin/find "$VERIFY_DIR/Payload" -maxdepth 1 -name '*.app' | head -1)"
  [[ -n "$APP" ]] || fail "no .app inside the exported .ipa."

  # Before the signing checks, because it is cheaper and because the signing checks cannot see this:
  # they walk signed code, and 0.3.0's rejection was an EMPTY nested .app, four directories, no
  # code at all, which `codesign --verify --deep --strict` passed straight over. The mirror of
  # Flow B's assert_relay_present: on iOS the relay must be ABSENT, since only macOS hosts a server.
  "$HERE/Scripts/check-nested-bundles.sh" "$APP" --forbid allodia-mcp
  assert_symbols_kept "$APP"

  APP_AUTH="$(first_authority "$APP")"
  echo "    signing authority: $APP_AUTH"
  [[ "$APP_AUTH" == *"Apple Distribution"* ]] \
    || fail "the exported app is not signed with an Apple Distribution cert (got '$APP_AUTH'), the export did not distribution-sign."

  # A distribution (App Store) profile carries NO ProvisionedDevices; a development/ad-hoc one does.
  # This catches accidentally exporting a non-Store build, which App Store Connect would reject.
  PROFILE="$APP/embedded.mobileprovision"
  [[ -f "$PROFILE" ]] || fail "the exported app has no embedded provisioning profile."
  if security cms -D -i "$PROFILE" 2>/dev/null | grep -q ProvisionedDevices; then
    fail "the embedded profile lists ProvisionedDevices, it's a development/ad-hoc profile, not an App Store one."
  fi

  codesign --verify --deep --strict --verbose=2 "$APP" || fail "codesign --verify failed on the exported app."

  mkdir -p "$KEEP"
  KEPT="$KEEP/AllodiaMail-$MARKETING_VERSION_VALUE-AppStore.ipa"
  cp "$IPA" "$KEPT"

  echo ""
  echo "==> iOS App Store package ready: $KEPT"
  upload_hint "$KEPT" ios
  echo "    See clients/apple/README.md before submitting."
  exit 0
fi

# ---- Flow B: manual distribution signing + Store .pkg ------------------------------------------
# Done by hand, NOT `xcodebuild -exportArchive`: the app-store re-sign pass re-signs the app and
# frameworks with the distribution cert but SKIPS nested SPM resource bundles under
# Contents/Resources/, MailcalKit_MailcalUI.bundle kept its Apple Development signature and App
# Store Connect rejected the upload (ITMS-90284). We also can't post-fix it, because Xcode's
# automatic (cloud-managed) signing fetches the distribution cert transiently and leaves nothing in
# the keychain to re-sign with. So Flow B signs the whole bundle tree itself with an explicit,
# persistent Apple Distribution cert, then builds the installer with a persistent Mac Installer
# Distribution cert. It is deterministic and self-verifying (the consistency gate below).
if [[ "$FLOW" == app-store ]]; then
  echo "==> Mac App Store: signing manually ($APPLE_DISTRIBUTION_IDENTITY)"
  APP="$EXPORT/$APP_NAME"
  rm -rf "$EXPORT"; mkdir -p "$EXPORT"
  /bin/cp -R "$ARCHIVE/Products/Applications/$APP_NAME" "$APP"
  # Before any signing: a build with no relay is not shippable, and finding that out here costs
  # nothing while finding it out from a user's assistant costs a release.
  assert_relay_present "$APP"

  BUNDLE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")"

  # Resolve a Mac App Store provisioning profile that names our app id, authorises our distribution
  # cert (App Store validates the app's signing cert is listed in its embedded profile), AND grants
  # the App Group the sandboxed relay needs. MAS_PROVISIONING_PROFILE (a path) wins; otherwise scan
  # the two standard profile locations, a portal-downloaded profile lands in MobileDevice/, an
  # Xcode-managed one in UserData/.
  #
  # The group is read out of the helper's entitlements file rather than written again here, one
  # literal, so what we demand of the profile cannot drift from what we actually sign.
  APP_GROUP="$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.application-groups:0' \
    "$HERE/App/AllodiaMailMcpHelper.appstore.entitlements" 2>/dev/null || true)"
  APP_GROUP="${APP_GROUP//\$(PRODUCT_BUNDLE_IDENTIFIER)/$BUNDLE_ID}"
  [[ -n "$APP_GROUP" ]] || fail "could not read the app group from App/AllodiaMailMcpHelper.appstore.entitlements."
  # EVERY valid codesigning identity with this name, not the first one. A renewed Apple
  # Distribution cert does not replace its predecessor, both sit in the keychain under the SAME
  # common name until one expires, and then `codesign --sign "<name>"` refuses outright:
  #   "Apple Distribution: … : ambiguous (matches … and … in login.keychain-db)".
  # Picking the first match would be worse than the error, because the two are not
  # interchangeable: a provisioning profile authorises ONE of them by fingerprint, and App Store
  # validation checks the app's signing cert against the profile's list. So the profile decides
  # which cert we sign with, and we sign by SHA-1, a name is not a unique key here.
  DIST_CANDIDATES="$(security find-identity -v -p codesigning \
    | awk -F'"' -v name="$APPLE_DISTRIBUTION_IDENTITY" '$2 == name { split($1, a, " "); print a[2] }')"
  [[ -n "$DIST_CANDIDATES" ]] || fail "no valid codesigning identity named '$APPLE_DISTRIBUTION_IDENTITY' is in the keychain.
       Check the name in $CONFIG against: security find-identity -v -p codesigning"
  PROFILE_MATCH="$(python3 - "$DEVELOPMENT_TEAM.$BUNDLE_ID" "$DIST_CANDIDATES" "$MAS_PROVISIONING_PROFILE" "$APP_GROUP" <<'PY'
import glob, hashlib, os, plistlib, subprocess, sys
appid, explicit, group = sys.argv[1], sys.argv[3], sys.argv[4]
candidates = {s.strip().upper() for s in sys.argv[2].split() if s.strip()}

def decode(p):
    try:
        return plistlib.loads(subprocess.run(['security', 'cms', '-D', '-i', p],
                                              capture_output=True).stdout)
    except Exception:
        return None

# Which of our certs this profile authorises, if any. Returned so the caller signs with THAT one
# rather than by name, the fingerprint is the only unique key when two certs share a name.
def signing_cert(d):
    listed = {hashlib.sha1(c).hexdigest().upper() for c in d.get('DeveloperCertificates', [])}
    match = candidates & listed
    return sorted(match)[0] if match else None

# Every capability the signed app claims has to be in here, not just the app id and the cert.
# Regenerating a profile mints a NEW file beside the old one, both match the app id, both list
# the same cert, so a predicate that stops there is choosing between an outdated profile and a
# current one on a coin toss it does not know it is flipping. (It was worse than a coin toss: the
# scan was alphabetical, so `6186a9b6…` beat `a91000f9…` and the STALE profile won every time,
# deterministically, no matter how many times you regenerated.)
def authorizes(d):
    if not d or d.get('Entitlements', {}).get('com.apple.application-identifier') != appid:
        return False
    if group not in d.get('Entitlements', {}).get('com.apple.security.application-groups', []):
        return False
    return signing_cert(d) is not None

if explicit:
    ex = os.path.expanduser(explicit)
    d = decode(ex)
    if authorizes(d):
        print(f"{ex}\t{signing_cert(d)}")
    sys.exit(0)

# Newest first among the qualifying ones: after a regeneration the freshest profile is the one
# that reflects the App ID as it stands today, and the older ones are debris nobody prunes.
found = []
for base in ('~/Library/MobileDevice/Provisioning Profiles',
             '~/Library/Developer/Xcode/UserData/Provisioning Profiles'):
    for ext in ('*.provisionprofile', '*.mobileprovision'):
        for p in glob.glob(os.path.join(os.path.expanduser(base), ext)):
            d = decode(p)
            if authorizes(d):
                found.append((d.get('CreationDate'), p, signing_cert(d)))
if found:
    found.sort(key=lambda c: (c[0] is not None, c[0]), reverse=True)
    print(f"{found[0][1]}\t{found[0][2]}")
PY
)"
  PROFILE_FILE="${PROFILE_MATCH%%$'\t'*}"
  # Sign with the FINGERPRINT, never the name: two valid certs can share a common name, and
  # `codesign --sign "<name>"` then fails "ambiguous", and if it did not, it would be a coin toss
  # between a cert this profile authorises and one it does not.
  SIGN_ID="${PROFILE_MATCH##*$'\t'}"
  [[ -n "$PROFILE_FILE" ]] || fail "no Mac App Store provisioning profile for $DEVELOPMENT_TEAM.$BUNDLE_ID
       authorizes a valid '$APPLE_DISTRIBUTION_IDENTITY' cert AND grants the app group
       '$APP_GROUP'.
       Candidate certs in the keychain:
$(echo "$DIST_CANDIDATES" | sed 's/^/         /')
       If a cert was recently renewed, the profile still authorizes the OLDER one until it is
       re-issued, regenerate the profile and it will pick up the current cert.
       A profile is a SNAPSHOT of the App ID's capabilities taken when it was GENERATED, so a
       profile issued before the group was added will never carry it and re-DOWNLOADING that one
       returns a byte-identical file. In the developer portal:
         Identifiers ▸ $BUNDLE_ID ▸ enable 'App Groups' ▸ assign '$APP_GROUP' ▸ Save
         Profiles ▸ (the Mac App Store profile) ▸ Edit ▸ Save   ← this RE-ISSUES it
       Then Download it. Note a PRODUCTION profile cannot be installed by double-clicking:
       System Settings takes Development profiles only, so either open it with Xcode or just
       copy it into place:
         cp ~/Downloads/<name>.provisionprofile ~/Library/Developer/Xcode/UserData/Provisioning\\ Profiles/
       (or point MAS_PROVISIONING_PROFILE=<path> at the download in $CONFIG). Then re-run.
       'Enabled Capabilities' on the profile page should list App Groups. (More: clients/apple/README.md.)"
  echo "    profile: $(basename "$PROFILE_FILE") (grants $APP_GROUP)"
  echo "    signing cert: $SIGN_ID"
  /bin/cp "$PROFILE_FILE" "$APP/Contents/embedded.provisionprofile"

  # Reconstruct the distribution entitlements that xcodebuild would inject: the sandbox set from
  # App/AllodiaMail.appstore.entitlements, with $(AppIdentifierPrefix) resolved to the team id and
  # the profile-derived application-identifier / team-identifier added. (codesign, unlike the build
  # system, does NOT inject those, they must be present or the app won't match its profile.)
  RESOLVED_ENTS="$BUILD/appstore.resolved.entitlements"
  # Two tokens, not one. The bundle id is injected (docs/branding.md), so the entitlements name it
  # as $(PRODUCT_BUNDLE_IDENTIFIER), which xcodebuild expands on the ordinary build path but
  # codesign does not, and an unexpanded one signs the literal text into the app.
  sed -e "s/\$(AppIdentifierPrefix)/${DEVELOPMENT_TEAM}./g" \
      -e "s/\$(PRODUCT_BUNDLE_IDENTIFIER)/${BUNDLE_ID}/g" \
    "$HERE/App/AllodiaMail.appstore.entitlements" >"$RESOLVED_ENTS"
  /usr/libexec/PlistBuddy -c "Add :com.apple.application-identifier string ${DEVELOPMENT_TEAM}.${BUNDLE_ID}" "$RESOLVED_ENTS"
  /usr/libexec/PlistBuddy -c "Add :com.apple.developer.team-identifier string ${DEVELOPMENT_TEAM}" "$RESOLVED_ENTS"
  # This flow reads its entitlements from the source file, which carries XML comments that
  # codesign's AMFI parser rejects. It survives only because the PlistBuddy calls above rewrite the
  # plist and drop the comments as a side effect, an accident, so make it a check. The
  # Developer-ID flow hit the same rock and reads the entitlements off the app instead.
  plutil -lint "$RESOLVED_ENTS" >/dev/null \
    || fail "the resolved App Store entitlements are not a valid plist ($RESOLVED_ENTS)."

  # The relay bundle's own entitlements, resolved the same way as the app's. It needs its own set
  # the Store requires every executable to be sandboxed (ITMS-90296), and it must NOT get the
  # app's set: the app's keychain group and file grants are the app's, and Apple's guidance for an
  # embedded tool is that surplus entitlements cause launch failures.
  # Deliberately NOT given com.apple.application-identifier / team-identifier, unlike the app.
  # This is the exact set that was MEASURED to work (2026-08-03); adding an application-identifier
  # claims an App ID that would then need its own registration and profile, and Apple's guidance
  # for an embedded tool is that surplus entitlements cause launch failures, under ad-hoc signing
  # an application-identifier here was SIGKILLed outright. If App Store validation turns out to
  # want a profile on the nested helper, that is the point to add both, together, and re-measure.
  # The app group needs no team prefix on macOS: `group.`-style identifiers work (measured), and
  # the shorter string buys 5 bytes of sun_path headroom.
  RELAY_ENTS="$BUILD/appstore.relay.entitlements"
  # Same substitution as the app's. This file is signed ONLY from here, the ordinary build
  # ad-hoc-signs the nested bundle with no entitlements at all, so nothing else would ever expand
  # the token for it.
  sed -e "s/\$(AppIdentifierPrefix)/${DEVELOPMENT_TEAM}./g" \
      -e "s/\$(PRODUCT_BUNDLE_IDENTIFIER)/${BUNDLE_ID}/g" \
    "$HERE/App/AllodiaMailMcpHelper.appstore.entitlements" >"$RELAY_ENTS"
  # Normalise before signing, which drops the XML comments as a side effect. The app's own flows
  # document codesign's AMFI parser rejecting comments outright ("AMFIUnserializeXML: syntax error
  # near line 6") and dodge it, one by reading entitlements back off the app, the other by
  # accident, via PlistBuddy rewriting the file. That rejection did NOT reproduce on the toolchain
  # here (2026-08-03: codesign accepted the commented file and applied both keys correctly), so
  # this is insurance rather than a fix for an observed failure. It is one cheap line, and the
  # alternative is a rationale comment we cannot keep in the file it explains.
  plutil -convert xml1 "$RELAY_ENTS"
  plutil -lint "$RELAY_ENTS" >/dev/null \
    || fail "the resolved relay entitlements are not a valid plist ($RELAY_ENTS)."
  assert_entitlements_resolved "$RELAY_ENTS"
  assert_entitlements_resolved "$RESOLVED_ENTS"

  # Deep re-sign, inner code first (-depth) then the app last. Libraries take NO entitlements;
  # executables MUST be sandboxed; the app carries Hardened Runtime (matching the Release config)
  # + the sandbox set. See nested_library_items / nested_executable_items for why that split is
  # load-bearing rather than tidiness.
  while IFS= read -r item; do
    echo "    signing nested library: ${item#"$APP"/}"
    codesign --force --sign "$SIGN_ID" "$item"
  done < <(nested_library_items "$APP")
  while IFS= read -r item; do
    echo "    signing nested executable: ${item#"$APP"/}"
    codesign --force --options runtime --entitlements "$RELAY_ENTS" \
      --sign "$SIGN_ID" "$item"
  done < <(nested_executable_items "$APP")
  codesign --force --options runtime --entitlements "$RESOLVED_ENTS" \
    --sign "$SIGN_ID" "$APP"

  # Signature-consistency gate, the check that would have caught ITMS-90284 before the upload.
  echo "==> Verifying every nested item is signed with the same cert as the app"
  APP_AUTH="$(first_authority "$APP")"
  echo "    app: $APP_AUTH"
  [[ "$APP_AUTH" == *"Apple Distribution"* ]] || fail "the app is not signed with an Apple Distribution cert (got '$APP_AUTH')."
  MISMATCH=0
  while IFS= read -r item; do
    A="$(first_authority "$item")"
    if [[ "$A" != "$APP_AUTH" ]]; then
      echo "    MISMATCH: ${item#"$APP"/} → '$A'" >&2
      MISMATCH=1
    fi
  done < <(nested_code_items "$APP")
  [[ "$MISMATCH" -eq 0 ]] || fail "nested code signed with a different cert than the app (ITMS-90284)."

  # The OTHER axis. The cert gate above says WHO signed each item; this says what they were signed
  # WITH, which is the half that was missing when App Store Connect returned ITMS-90296.
  echo "==> Verifying every nested executable is sandboxed (ITMS-90296)"
  assert_store_entitlements "$APP"
  echo "    every nested executable carries com.apple.security.app-sandbox"

  # And the axis neither of those two reads: that each nested bundle is a bundle at all. Both gates
  # above iterate signed code, so a directory tree with nothing in it is invisible to them
  # (ITMS-90207 / ITMS-90036, see check-nested-bundles.sh). No --forbid here: on macOS the relay
  # belongs in the payload, and assert_relay_present already insists on it.
  "$HERE/Scripts/check-nested-bundles.sh" "$APP"
  assert_symbols_kept "$APP"

  codesign --verify --deep --strict --verbose=2 "$APP" || fail "codesign --verify failed on the signed app."

  # Build + sign the Store installer.
  PKG="$EXPORT/AllodiaMail.pkg"
  echo "==> Building the installer .pkg (productbuild, signed with $MAC_INSTALLER_IDENTITY)"
  productbuild --component "$APP" /Applications --sign "$MAC_INSTALLER_IDENTITY" "$PKG"
  pkgutil --check-signature "$PKG"

  mkdir -p "$KEEP"
  KEPT="$KEEP/AllodiaMail-$MARKETING_VERSION_VALUE-MacAppStore.pkg"
  cp "$PKG" "$KEPT"

  echo ""
  echo "==> Mac App Store package ready: $KEPT"
  upload_hint "$KEPT" macos
  echo "    See clients/apple/README.md before submitting."
  exit 0
fi

# ---- Flow A (Developer ID): export --------------------------------------------------------------
EXPORT_PLIST="$BUILD/ExportOptions.plist"
sed "s/__TEAM_ID__/$DEVELOPMENT_TEAM/g" "$EXPORT_TEMPLATE" >"$EXPORT_PLIST"

echo "==> Exporting the archive ($FLOW)"
xcodebuild -exportArchive \
  -archivePath "$ARCHIVE" \
  -exportOptionsPlist "$EXPORT_PLIST" \
  -exportPath "$EXPORT" \
  ${EXPORT_EXTRA[@]+"${EXPORT_EXTRA[@]}"}

# ---- Flow A: notarize (optional) + .dmg --------------------------------------------------------
APP="$EXPORT/$APP_NAME"
[[ -d "$APP" ]] || fail "export produced no $APP_NAME, check the log above."
assert_relay_present "$APP"

# Re-sign nested code with the Developer ID cert, then check it. `-exportArchive` handles the app
# and its frameworks, but a bare Mach-O helper (Contents/MacOS/allodia-mcp) keeps whatever it had
# an ad-hoc signature from the build phase, and the notary service rejects an ad-hoc-signed
# executable inside a Developer ID app. Signing the helper FIRST and the app LAST matters: the
# app's signature seals its own directory, so touching anything inside afterwards invalidates it.
# Read the entitlements back OFF THE APP, not out of App/AllodiaMail.macOS.entitlements.
#
# Two reasons, and the second one is a trap. The dumped copy is what the export ACTUALLY applied:
# already resolved, with no build-setting indirection to drift from. And the source file is not
# something `codesign` can read at all: it carries an XML comment inside <plist> (which is where
# the rationale for these entitlements lives, and rightly so), and codesign's AMFI parser rejects
# comments outright, `Failed to parse entitlements: AMFIUnserializeXML: syntax error near line 6`.
# Xcode strips comments before signing; codesign does not. Flow B reads its own source file and
# gets away with it only by accident: its PlistBuddy calls rewrite the plist, dropping the comments
# as a side effect.
#
# Re-signing the app WITHOUT its entitlements is not an acceptable simplification, even though this
# build's set is currently empty (`<dict/>`): the file exists to hold Hardened-Runtime exceptions
# the moment one is needed, and silently dropping them later would be invisible here and fatal at
# runtime.
APP_ENTS="$BUILD/devid.resolved.entitlements"
ENTS_ARGS=()
if codesign -d --entitlements - --xml "$APP" >"$APP_ENTS" 2>/dev/null && [[ -s "$APP_ENTS" ]]; then
  plutil -lint "$APP_ENTS" >/dev/null \
    || fail "the entitlements read back from the exported app are not a valid plist ($APP_ENTS)."
  ENTS_ARGS=(--entitlements "$APP_ENTS")
  echo "    entitlements: $(plutil -convert json -o - "$APP_ENTS")"
else
  echo "    entitlements: none on the exported app"
fi

echo "==> Signing nested code with $DEVELOPER_ID_IDENTITY"
NESTED_SIGNED=0
while IFS= read -r item; do
  echo "    signing nested: ${item#"$APP"/}"
  codesign --force --options runtime --timestamp --sign "$DEVELOPER_ID_IDENTITY" "$item"
  NESTED_SIGNED=1
done < <(nested_code_items "$APP")
if [[ "$NESTED_SIGNED" -eq 1 ]]; then
  codesign --force --options runtime --timestamp \
    ${ENTS_ARGS[@]+"${ENTS_ARGS[@]}"} \
    --sign "$DEVELOPER_ID_IDENTITY" "$APP"
fi

echo "==> Verifying every nested item is signed with the same cert as the app"
APP_AUTH="$(first_authority "$APP")"
echo "    app: $APP_AUTH"
[[ "$APP_AUTH" == *"Developer ID Application"* ]] \
  || fail "the app is not signed with a Developer ID cert (got '$APP_AUTH'), notarization would reject it."
MISMATCH=0
while IFS= read -r item; do
  A="$(first_authority "$item")"
  if [[ "$A" != "$APP_AUTH" ]]; then
    echo "    MISMATCH: ${item#"$APP"/} → '$A'" >&2
    MISMATCH=1
  fi
done < <(nested_code_items "$APP")
[[ "$MISMATCH" -eq 0 ]] || fail "nested code signed with a different cert than the app, notarization would reject it."

# The re-sign above replaced the app's signature, so prove it carried the entitlements through.
# This build's set is EMPTY, which is exactly why the check is here: a bug that dropped
# entitlements would look identical to success today and only surface the first time one matters.
if [[ "${#ENTS_ARGS[@]}" -gt 0 ]]; then
  AFTER_ENTS="$BUILD/devid.after.entitlements"
  codesign -d --entitlements - --xml "$APP" >"$AFTER_ENTS" 2>/dev/null \
    || fail "the re-signed app reports no entitlements, but it was signed with some."
  diff <(plutil -convert json -o - "$APP_ENTS") <(plutil -convert json -o - "$AFTER_ENTS") >/dev/null \
    || fail "the re-sign changed the app's entitlements, compare $APP_ENTS and $AFTER_ENTS."
  echo "    entitlements survived the re-sign"
fi

codesign --verify --deep --strict --verbose=2 "$APP" || fail "codesign --verify failed on the exported app."

if [[ "$NOTARIZE" -eq 1 ]]; then
  ZIP="$BUILD/AllodiaMail.zip"
  echo "==> Submitting to the notary service (notarytool submit --wait; this can take a few minutes)"
  /usr/bin/ditto -c -k --keepParent "$APP" "$ZIP"
  xcrun notarytool submit "$ZIP" --keychain-profile "$NOTARY_KEYCHAIN_PROFILE" --wait
  echo "==> Stapling the notarization ticket into the app (works fully offline afterwards)"
  xcrun stapler staple "$APP"
else
  echo "==> Skipping notarization (--no-notarize): the app is Developer-ID-signed but NOT notarized."
fi

echo "==> Building the .dmg"
STAGING="$BUILD/dmg"
rm -rf "$STAGING"; mkdir -p "$STAGING"
/bin/cp -R "$APP" "$STAGING/"
ln -s /Applications "$STAGING/Applications"
rm -f "$DMG"
hdiutil create -volname "$VOLNAME" -srcfolder "$STAGING" -ov -format UDZO "$DMG" >/dev/null
# Tear the staging tree down immediately. The `Applications` entry above is a symlink to the real
# /Applications, and anything that walks this repo following symlinks, Android Studio's codebase
# indexer did exactly this, descends into every installed app, including Xcode's SDK. That turned a
# 176k-file repo into a 1.5M-file scan and pinned the machine. The .dmg is already written; the
# staging dir has no further use.
rm -rf "$STAGING"

echo ""
echo "==> Done. Artifact: $DMG"
echo "    Install: open the .dmg, drag Allodia Mail & Calendar to Applications."
if [[ "$NOTARIZE" -eq 1 ]]; then
  echo "    Verify:  spctl -a -vvv -t exec \"$APP\"        # expect: accepted, source=Notarized Developer ID"
  echo "             stapler validate \"$APP\""
else
  echo "    NOTE: not notarized, Gatekeeper will block this on Macs other than the build machine."
fi
