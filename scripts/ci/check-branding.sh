#!/usr/bin/env bash
# Fail if a client has stopped taking its identity from the brand.
#
# The app's name and application id are injected at build time (docs/branding.md): `branding/`
# holds them, every client's build config reads them, and the unbranded default is what a build
# carries when nothing overrides it. Nothing about that arrangement fails loudly on its own; a
# literal written back into a manifest builds, installs and runs, and simply cannot be re-branded
# any more. This is the machine half of the rule, in the shape `check-version-sync.sh` uses for
# /VERSION: pin the committed defaults, then assert each client still *derives* rather than states.
#
# Run from the repo root:
#
#     scripts/ci/check-branding.sh
#
# What it deliberately does NOT check: prose. Documentation, comments and store copy still name the
# product, and the split's content pass is where that is decided; a grep for the word "Allodia"
# here would fail on every file that correctly explains what Allodia's build is.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
note() { printf '  %s\n' "$1" >&2; fail=1; }

# --- The source of truth ------------------------------------------------------------------------
# shellcheck source=scripts/dev/brand.sh
. "$ROOT/scripts/dev/brand.sh"

[ -f branding/default.env ] || {
  echo "ERROR: branding/default.env is missing: it is the identity every build falls back to." >&2
  exit 1
}

# Read the neutral values directly, not through brand_value: this checks what is *committed*, and
# a checkout with a brand (or an exported variable) would otherwise answer with that instead.
neutral_id="$(sed -n 's/^MAILCAL_APP_ID="\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' branding/default.env | tail -n1)"
neutral_name="$(sed -n 's/^MAILCAL_APP_NAME="\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' branding/default.env | tail -n1)"
[ -n "$neutral_id" ] || note "branding/default.env names no MAILCAL_APP_ID."
[ -n "$neutral_name" ] || note "branding/default.env names no MAILCAL_APP_NAME."

# The neutral listing is the third slot (docs/branding.md -> "The store copy"), and its absence is
# the one failure nothing else reports: a Flatpak build resolves it, finds nothing, and the app has
# no entry in a software centre at all -- on Linux only, at packaging time, in a checkout with no
# brand file. Which is exactly the checkout a fork has.
[ -f branding/default-listing.md ] || {
  echo "ERROR: branding/default-listing.md is missing: it is the store copy an unbranded build" >&2
  echo "       describes itself with, and the Linux metainfo cannot be generated without it." >&2
  exit 1
}
for heading in "Google Play — Short description" "Shared description — English"; do
  grep -qF "$heading" branding/default-listing.md ||
    note "branding/default-listing.md has no '$heading' section; flatpak_metadata.py needs both."
done

# The id is a URI scheme on Android, where an intent filter's scheme is matched case-sensitively
# and an uppercase one silently never matches. It is also a Flatpak app id, which needs three
# components.
if [[ ! "$neutral_id" =~ ^[a-z0-9_]+(\.[a-z0-9_]+){2,}$ ]]; then
  note "MAILCAL_APP_ID '$neutral_id' must be three or more lowercase reverse-DNS components."
fi

# --- Every client derives, rather than states ----------------------------------------------------
gradle=clients/android/app/build.gradle.kts
grep -q 'applicationId = brandValue("MAILCAL_APP_ID")' "$gradle" \
  || note "$gradle no longer takes applicationId from the brand."
grep -q 'manifestPlaceholders\["appName"\] = brandValue("MAILCAL_APP_NAME")' "$gradle" \
  || note "$gradle no longer takes the launcher label from the brand."

manifest=clients/android/app/src/main/AndroidManifest.xml
grep -q 'android:label="${appName}"' "$manifest" \
  || note "$manifest has a literal android:label: it must come from the appName placeholder."
# The OAuth redirects are the app id; a literal here is a filter that stops matching the moment the
# app is re-branded, which is a sign-in that dies on delivery with nothing logged.
grep -q 'android:host="${applicationId}"' "$manifest" \
  || note "$manifest has a literal Microsoft redirect host."
grep -q 'android:scheme="${applicationId}"' "$manifest" \
  || note "$manifest has a literal JMAP redirect scheme."

project=clients/apple/project.yml
grep -q 'PRODUCT_BUNDLE_IDENTIFIER: ${MAILCAL_APP_ID}' "$project" \
  || note "$project no longer takes the bundle id from the environment."
grep -q 'CFBundleDisplayName: ${MAILCAL_APP_NAME}' "$project" \
  || note "$project no longer takes the display name from the environment."
for ents in clients/apple/App/AllodiaMail.entitlements clients/apple/App/AllodiaMail.appstore.entitlements; do
  grep -q '<string>$(AppIdentifierPrefix)$(PRODUCT_BUNDLE_IDENTIFIER)</string>' "$ents" \
    || note "$ents names a keychain group that does not follow the bundle id."
done

# The MSIX and Flatpak manifests are committed carrying the neutral identity and rewritten by their
# packaging scripts, so what is checked is that the committed copy is still the neutral one.
appx=clients/windows/Mailcal/Package.appxmanifest
grep -q "<uap:Protocol Name=\"$neutral_id\">" "$appx" \
  || note "$appx does not declare the neutral protocol '$neutral_id': package.ps1 rewrites that one."
grep -q "<DisplayName>$neutral_name</DisplayName>" "$appx" \
  || note "$appx does not carry the neutral display name '$neutral_name'."

flatpak="clients/linux/flatpak/$neutral_id.yml"
[ -f "$flatpak" ] || note "the Flatpak manifest is not named for the neutral id ($flatpak is missing)."
if [ -f "$flatpak" ]; then
  grep -q "^app-id: $neutral_id\$" "$flatpak" \
    || note "$flatpak does not carry the neutral app id: package.sh rewrites that one."
fi

# --- The art ------------------------------------------------------------------------------------
# Every launcher icon is cut from one source, resolved by the same rule as the values above. The
# failure this catches is a generator quietly pinned back to a path: it keeps working, on Allodia's
# art, and only an unbranded build notices; by shipping somebody else's icon.
[ -f branding/default-icon.png ] || note "branding/default-icon.png is missing: the neutral icon."
[ -f branding/default-welcome.png ] || note "branding/default-welcome.png is missing: the neutral illustration."
grep -q 'brand_welcome_source' scripts/dev/brand-welcome.sh \
  || note "scripts/dev/brand-welcome.sh no longer resolves its source through the brand."
# The art is a brand slot, so the label has to describe the slot. "The Allodia robot, waving" was
# true of one build and announced to every other one.
for catalog in messages/*.json; do
  grep -q '"a11y_welcome_art"' "$catalog" \
    || note "$catalog is missing a11y_welcome_art."
done
for generator in \
  clients/apple/Scripts/generate-appicon.sh \
  clients/android/generate-icons.sh \
  clients/linux/flatpak/generate-icons.sh
do
  grep -q 'brand_icon_source' "$generator" \
    || note "$generator no longer resolves its source through the brand."
done
grep -q 'brand.py.*--icon-source' clients/windows/Mailcal/Images/generate-assets.ps1 \
  || note "clients/windows/Mailcal/Images/generate-assets.ps1 no longer resolves its source through the brand."

# --- The name the app says aloud -----------------------------------------------------------------
# Substituted into the catalog at codegen time, so the copy is unbranded in the tree and every
# locale moves together. A locale that hardcoded the product would be one language showing a
# different app's name.
for catalog in messages/*.json; do
  grep -q '"app_title": "{app_name}"' "$catalog" \
    || note "$catalog does not leave app_title as the {app_name} placeholder."
done

if [ "$fail" -ne 0 ]; then
  echo "ERROR: the app's identity is no longer injected everywhere (docs/branding.md)." >&2
  exit 1
fi
echo "OK: every client takes its name and application id from branding/ ($neutral_id)."
