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

# The slice step [2/3] reads UniFFI's metadata out of. Every slice carries the same metadata, so
# the last one this build produced will do, and the slice list stays free to narrow.
BINDGEN_SLICE="${TARGETS[${#TARGETS[@]} - 1]}"

# `cargo rustc --crate-type`, not `cargo build`, and this script is the only caller that does it.
# Both crate types are needed here and nowhere else: step [3/3] links `libmailcal_bindings.a` into
# every xcframework slice (an iOS app bundle can't ship a loose dylib), and step [2/3] reads a
# slice's `.dylib` to generate the Swift bindings. Cargo has no per-target crate-type, so the
# manifest lists only what every host needs (`cdylib`, `lib`) and the Apple-only `staticlib` is
# asked for right here, see crates/mailcal-bindings/Cargo.toml. Leaving it in the manifest cost
# Windows, Android and Linux 1.9 GB of archive apiece that nothing on those platforms opens.
#
# `--lib` names the target the crate types apply to. The package has an example beside the library,
# and `cargo rustc` refuses to apply `--crate-type` when the selection is ambiguous.
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

# Everything generated below is written through `install_generated`, which leaves a file whose bytes
# did not move alone, timestamp included. Xcode and SwiftPM both decide what to recompile from
# mtimes, and this script regenerates unconditionally, so rewriting the 20,000-line
# `mailcal_bindings.swift` with identical bytes is enough to recompile it and all 115 files of
# MailcalUI behind it, however warm the build cache. The C# generator preserves timestamps for the
# same reason (crates/mailcal-bindgen-cs).
#
# Holding a file still is not enough on a runner, where a checkout has none of these: they are
# gitignored, so each one is written fresh whatever this does locally. Hence the mirror under
# `build/`, the directory CI caches, holding the previous run's copy beside the build products made
# from it. A file this generation does not change is restored from there WITH the timestamp those
# products were built against, so they stay up to date.
MIRROR="$HERE/build/generated"
install_generated() {
  local src=$1 dst=$2
  local mirror="$MIRROR/${dst#"$HERE"/}"
  mkdir -p "$(dirname "$dst")" "$(dirname "$mirror")"
  if ! cmp -s "$src" "$dst"; then
    if cmp -s "$src" "$mirror"; then
      cp -p "$mirror" "$dst"
    else
      cp "$src" "$dst"
    fi
  fi
  cmp -s "$dst" "$mirror" || cp -p "$dst" "$mirror"
}

echo "==> [2/3] Regenerating Swift bindings + L10n into MailcalBindings"
# Generated into a staging directory, so `install_generated` has the file already in place, and the
# mirror, to compare against.
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cargo run --manifest-path "$ROOT/Cargo.toml" --quiet -p mailcal-bindgen-uniffi -- \
  generate --library "$ROOT/target/$BINDGEN_SLICE/$PROFILE/libmailcal_bindings.dylib" \
  --language swift --out-dir "$STAGE"
cargo run --manifest-path "$ROOT/Cargo.toml" --quiet -p mailcal-l10n -- \
  generate --target swift --root "$ROOT" --out "$STAGE"

# A Swift SPM target must hold ONLY Swift files, move the C header + modulemap out to the
# xcframework's Headers (the binary target vends the `mailcal_bindingsFFI` C module from there).
HDR="$ARTIFACTS/headers"
install_generated "$STAGE/mailcal_bindingsFFI.h" "$HDR/mailcal_bindingsFFI.h"
install_generated "$STAGE/mailcal_bindingsFFI.modulemap" "$HDR/module.modulemap"
rm -f "$STAGE/mailcal_bindingsFFI.h" "$STAGE/mailcal_bindingsFFI.modulemap"

mkdir -p "$BIND"
for f in "$STAGE"/*.swift; do
  [[ -e "$f" ]] || continue
  install_generated "$f" "$BIND/$(basename "$f")"
done
# Drop what an earlier generation left behind and this one did not produce; a stale binding
# compiles and then disagrees with the core.
for f in "$BIND"/*.swift; do
  [[ -e "$f" ]] || continue
  [[ -e "$STAGE/$(basename "$f")" ]] || rm -f "$f"
done

# Bundle the shared rich-composer editor (clients/composer/dist/editor.html) into MailcalUI as an SPM
# resource so it loads via Bundle.module, without it the composer WKWebView falls back to an
# empty stub (no editor, no quoted original). git-ignored; copied from the shared source.
#
# Rebuilt from clients/composer/src first, so what gets copied is what the sources say (the bundle
# is committed, not generated per build, see scripts/dev/composer-bundle.sh).
bash "$ROOT/scripts/dev/composer-bundle.sh"
install_generated "$ROOT/clients/composer/dist/editor.html" "$PKG/Sources/MailcalUI/composer/editor.html"

echo "==> [3/3] Assembling Mailcal.xcframework (${TARGETS[*]})"
XCF="$ARTIFACTS/Mailcal.xcframework"
SLICE_ARGS=()
SLICE_LIBS=()
for t in "${TARGETS[@]}"; do
  SLICE_LIBS+=("$ROOT/target/$t/$PROFILE/libmailcal_bindings.a")
  SLICE_ARGS+=(-library "$ROOT/target/$t/$PROFILE/libmailcal_bindings.a" -headers "$HDR")
done

# What this framework is made of: the slice list, and the content of every archive and header going
# into it. CONTENT, not timestamps, and that distinction is the whole point.
#
# A cached Rust build keeps the compiled dependencies but not the linked artifact, so a runner
# re-links `libmailcal_bindings.a` on every run and it arrives freshly dated. It arrives byte for
# byte identical too, measured across separate runners. So a timestamp carries no information here,
# and acting on one is expensive: `-create-xcframework` rewrites every byte, and a framework that
# is merely re-dated recompiles MailcalBindings and all 115 files of MailcalUI above it. The
# manifest decides whether to assemble; the reference file beside it carries the timestamp this
# content was first assembled with, and goes back on at the end.
XCF_MANIFEST="$MIRROR/.xcframework-manifest"
XCF_REFERENCE="$MIRROR/.xcframework-reference"
xcframework_manifest() {
  printf '%s\n' "${TARGETS[*]}"
  shasum -a 256 "${SLICE_LIBS[@]}" "$HDR"/* | awk '{print $1}'
}

XCF_WANT="$(xcframework_manifest)"
XCF_CONTENT_MOVED=0
if [[ ! -f "$XCF_MANIFEST" ]] || [[ "$(cat "$XCF_MANIFEST")" != "$XCF_WANT" ]]; then
  XCF_CONTENT_MOVED=1
fi

if [[ "$XCF_CONTENT_MOVED" -eq 1 || ! -d "$XCF" ]]; then
  # Assembled beside the framework and swapped in, never in place, so an interrupted
  # `-create-xcframework` cannot leave a half-written directory standing as the real one. The
  # staging name still ends in `.xcframework`, which xcodebuild requires of its `-output`.
  STAGED_XCF="$ARTIFACTS/.Mailcal-staged.xcframework"
  rm -rf "$STAGED_XCF"
  xcodebuild -create-xcframework "${SLICE_ARGS[@]}" -output "$STAGED_XCF" >/dev/null
  rm -rf "$XCF"
  mv "$STAGED_XCF" "$XCF"

  # Sign it with whatever local identity exists. Nothing about distribution depends on this, a
  # static archive carries no signature into the app, and xcodebuild never looks, but the Xcode IDE
  # will not use a binary target it has not been told to trust, and for an UNSIGNED one it decides
  # that per content. Assembling the framework rewrites that content, so the IDE re-asks ("The
  # Framework Mailcal.xcframework is unsigned"); with that dialog standing, a build fails with
  # "no library for this platform was found in Mailcal.xcframework", an error that describes a
  # missing slice while all three are on disk. Signed, the trust is recorded against the author and
  # asked once. (Ad-hoc "-" would put us back on the content: its identity IS the cdhash.) A machine
  # with no identity loses only the IDE convenience.
  SIGN_IDENTITY="$(security find-identity -v -p codesigning 2>/dev/null |
    awk -F'"' '/Apple Development|Developer ID/ {print $2; exit}')"
  if [[ -n "$SIGN_IDENTITY" ]]; then
    codesign --force --sign "$SIGN_IDENTITY" --timestamp=none "$XCF" ||
      echo "warning: could not sign Mailcal.xcframework; Xcode will ask to trust it after each rebuild" >&2
  fi
else
  echo "    unchanged, kept: Mailcal.xcframework"
fi

# Mint a new reference the moment the content moves, and otherwise put the old one back over the
# whole tree. What Xcode and SwiftPM compare is the timestamp, so a framework reassembled from
# identical bytes has to look untouched or every target above it rebuilds.
if [[ "$XCF_CONTENT_MOVED" -eq 1 ]]; then
  mkdir -p "$MIRROR"
  : >"$XCF_REFERENCE"
  printf '%s\n' "$XCF_WANT" >"$XCF_MANIFEST"
fi
find "$XCF" -exec touch -r "$XCF_REFERENCE" {} +

echo "==> Done. Slices: $(ls "$XCF" | grep -v Info.plist | grep -v _CodeSignature | tr '\n' ' ')"

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
