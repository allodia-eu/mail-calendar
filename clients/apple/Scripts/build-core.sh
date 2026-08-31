#!/usr/bin/env bash
# Builds the shared Rust core for every Apple slice and packages it for MailcalKit:
#   1. cargo-build mailcal-bindings for iOS device, iOS simulator, and Apple-silicon macOS
#   2. regenerate the UniFFI Swift bindings + localised L10n into Sources/MailcalBindings
#   3. assemble Mailcal.xcframework (static slices + the C-module headers) into artifacts/
#   4. build the `allodia-mcp` stdio relay for macOS into artifacts/ (docs/mcp.md)
# Run this once after cloning, and again whenever the Rust FFI changes. The xcframework and
# the generated bindings are git-ignored (rebuilt from the Rust source of truth).
#
# Usage: build-core.sh [--no-device] [--release]
#   --no-device   Skip the aarch64-apple-ios (physical device) slice. CI passes this: it only
#                 ever links `platform=macOS` and `generic/platform=iOS Simulator`, so the device
#                 slice is compiled purely to be packaged. Never pass it when building something
#                 that will run on a real iPhone/iPad (see .agents/skills/ios-device-bgsync).
#   --release     Build the optimised `release` profile instead of `debug`. The packaging path
#                 (Scripts/package.sh) uses this so the shipped app carries an optimised core;
#                 the dev loop (build-and-run.sh) stays on debug. `dev-harness` is off by default
#                 and never passed here, so release adds no feature juggling, only the profile.
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"        # clients/apple
ROOT="$(cd "$HERE/../.." && pwd)"               # repo root (worktree)
PKG="$HERE/Packages/MailcalKit"
BIND="$PKG/Sources/MailcalBindings"
ARTIFACTS="$PKG/artifacts"
IOS_DEPLOYMENT_TARGET=18.0
MACOS_DEPLOYMENT_TARGET=15.0

# Apple silicon only; add x86_64-apple-darwin if the Mac support policy ever widens.
# The simulator slice must stay in the list unconditionally: step [2/3] generates the Swift
# bindings from its dylib.
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin)

# Cargo profile: `debug` (dev loop, the default) or `release` (packaging). The profile name is
# also the target/ sub-directory each slice's artifacts land in.
PROFILE=debug
CARGO_PROFILE_ARGS=()

for arg in "$@"; do
  case "$arg" in
    --no-device) TARGETS=(aarch64-apple-ios-sim aarch64-apple-darwin) ;;
    --release) PROFILE=release; CARGO_PROFILE_ARGS=(--release) ;;
    -h|--help) sed -n '9,17p' "$0"; exit 0 ;;
    *) echo "build-core: unknown option '$arg' (want: --no-device, --release)" >&2; exit 2 ;;
  esac
done

# `cargo rustc --crate-type`, not `cargo build`, and this script is the only caller that does it.
# Both crate types are needed here and nowhere else: step [3/3] links `libmailcal_bindings.a` into
# every xcframework slice (an iOS app bundle can't ship a loose dylib), and step [2/3] reads the
# ios-sim `.dylib` to generate the Swift bindings. Cargo has no per-target crate-type, so the
# manifest lists only what every host needs (`cdylib`, `lib`) and the Apple-only `staticlib` is
# asked for right here, see crates/mailcal-bindings/Cargo.toml. Leaving it in the manifest cost
# Windows, Android and Linux 1.9 GB of archive apiece that nothing on those platforms opens.
#
# `--lib` is required, not decoration: the package also has a `uniffi-bindgen` bin target, and
# `cargo rustc` refuses to apply `--crate-type` when the selection is ambiguous.
BINDINGS_CRATE_TYPES=(--lib --crate-type staticlib --crate-type cdylib)

# The Allodia sign-in, when this build was given the registration that turns it on -- derived from
# that registration rather than asked for separately, so the two halves cannot disagree
# (scripts/dev/lib.sh, BUILDING.md). Nothing to do in a build from source: it resolves to nothing
# and the app ships without the route, which is supported.
# shellcheck source=scripts/dev/lib.sh
source "$ROOT/scripts/dev/lib.sh"
CORE_FEATURES=()
CORE_FEATURE_LIST="$(core_cargo_features)"
if [[ -n "$CORE_FEATURE_LIST" ]]; then
  CORE_FEATURES=(--features "$CORE_FEATURE_LIST")
fi

echo "==> [1/3] Cross-compiling mailcal-bindings ($PROFILE) for ${#TARGETS[@]} Apple slices${CORE_FEATURE_LIST:+ (+$CORE_FEATURE_LIST)}"
for t in "${TARGETS[@]}"; do
  echo "    - $t"
  case "$t" in
    aarch64-apple-ios|aarch64-apple-ios-sim)
      (
        unset MACOSX_DEPLOYMENT_TARGET
        export IPHONEOS_DEPLOYMENT_TARGET="$IOS_DEPLOYMENT_TARGET"
        cargo rustc --manifest-path "$ROOT/Cargo.toml" -p mailcal-bindings "${BINDINGS_CRATE_TYPES[@]}" --target "$t" ${CARGO_PROFILE_ARGS[@]+"${CARGO_PROFILE_ARGS[@]}"} ${CORE_FEATURES[@]+"${CORE_FEATURES[@]}"}
      )
      ;;
    aarch64-apple-darwin)
      (
        unset IPHONEOS_DEPLOYMENT_TARGET
        export MACOSX_DEPLOYMENT_TARGET="$MACOS_DEPLOYMENT_TARGET"
        cargo rustc --manifest-path "$ROOT/Cargo.toml" -p mailcal-bindings "${BINDINGS_CRATE_TYPES[@]}" --target "$t" ${CARGO_PROFILE_ARGS[@]+"${CARGO_PROFILE_ARGS[@]}"} ${CORE_FEATURES[@]+"${CORE_FEATURES[@]}"}
      )
      ;;
  esac
done

echo "==> [2/3] Regenerating Swift bindings + L10n into MailcalBindings"
mkdir -p "$BIND"
rm -f "$BIND"/*.swift
cargo run --manifest-path "$ROOT/Cargo.toml" --quiet --bin uniffi-bindgen -- \
  generate --library "$ROOT/target/aarch64-apple-ios-sim/$PROFILE/libmailcal_bindings.dylib" \
  --language swift --out-dir "$BIND"
cargo run --manifest-path "$ROOT/Cargo.toml" --quiet -p mailcal-l10n -- \
  generate --target swift --root "$ROOT" --out "$BIND"

# Bundle the shared rich-composer editor (clients/composer/dist/editor.html) into MailcalUI as an SPM
# resource so it loads via Bundle.module, without it the composer WKWebView falls back to an
# empty stub (no editor, no quoted original). git-ignored; copied from the shared source.
#
# Rebuilt from clients/composer/src first, so what gets copied is what the sources say (the bundle
# is committed, not generated per build, see scripts/dev/composer-bundle.sh).
bash "$ROOT/scripts/dev/composer-bundle.sh"
COMPOSER="$PKG/Sources/MailcalUI/composer"
mkdir -p "$COMPOSER"
cp "$ROOT/clients/composer/dist/editor.html" "$COMPOSER/editor.html"

# A Swift SPM target must hold ONLY Swift files, move the C header + modulemap out to the
# xcframework's Headers (the binary target vends the `mailcal_bindingsFFI` C module from there).
HDR="$ARTIFACTS/headers"
rm -rf "$HDR"; mkdir -p "$HDR"
mv "$BIND/mailcal_bindingsFFI.h" "$HDR/"
mv "$BIND/mailcal_bindingsFFI.modulemap" "$HDR/module.modulemap"

echo "==> [3/3] Assembling Mailcal.xcframework (${TARGETS[*]})"
rm -rf "$ARTIFACTS/Mailcal.xcframework"
SLICE_ARGS=()
for t in "${TARGETS[@]}"; do
  SLICE_ARGS+=(-library "$ROOT/target/$t/$PROFILE/libmailcal_bindings.a" -headers "$HDR")
done
xcodebuild -create-xcframework "${SLICE_ARGS[@]}" \
  -output "$ARTIFACTS/Mailcal.xcframework" >/dev/null

# Sign it with whatever local identity exists. Nothing about distribution depends on this, a
# static archive carries no signature into the app, and xcodebuild never looks, but the Xcode IDE
# will not use a binary target it has not been told to trust, and for an UNSIGNED one it decides
# that per content. This script rewrites the content on every run, so the IDE re-asks ("The
# Framework Mailcal.xcframework is unsigned") after every core rebuild; with that dialog standing,
# a build fails with "no library for this platform was found in Mailcal.xcframework", an error
# that describes a missing slice while all three are on disk. Signed, the trust is recorded against
# the author and asked once. (Ad-hoc "-" would put us back on the content: its identity IS the
# cdhash.) A machine with no identity loses only the IDE convenience.
SIGN_IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null |
  awk -F'"' '/Apple Development|Developer ID/ {print $2; exit}')"
if [[ -n "$SIGN_IDENTITY" ]]; then
  codesign --force --sign "$SIGN_IDENTITY" --timestamp=none "$ARTIFACTS/Mailcal.xcframework" ||
    echo "warning: could not sign Mailcal.xcframework; Xcode will ask to trust it after each rebuild" >&2
fi
echo "==> Done. Slices: $(ls "$ARTIFACTS/Mailcal.xcframework" | grep -v Info.plist | grep -v _CodeSignature | tr '\n' ' ')"

# The MCP stdio relay an assistant spawns to reach the running app (docs/mcp.md). A separate
# BINARY, not a library slice: an MCP client executes it as a child process, so it ships beside
# the app's own executable in Contents/MacOS and the Xcode copy phase puts it there. macOS only:
# iOS hosts no server, so there is nothing for a relay to reach.
echo "==> [4/4] Building the allodia-mcp relay (macOS)"
(
  unset IPHONEOS_DEPLOYMENT_TARGET
  export MACOSX_DEPLOYMENT_TARGET="$MACOS_DEPLOYMENT_TARGET"
  cargo build --manifest-path "$ROOT/Cargo.toml" -p mailcal-mcp-shim --bin allodia-mcp \
    --target aarch64-apple-darwin ${CARGO_PROFILE_ARGS[@]+"${CARGO_PROFILE_ARGS[@]}"}
)
cp "$ROOT/target/aarch64-apple-darwin/$PROFILE/allodia-mcp" "$ARTIFACTS/allodia-mcp"
