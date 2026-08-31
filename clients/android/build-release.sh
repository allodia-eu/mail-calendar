#!/usr/bin/env bash
# Build the Android RELEASE APK, the thing that actually ships, and the only build whose
# performance is worth comparing against anyone else's app.
#
#   ./build-release.sh [--install] [--bundle]
#
# `--bundle` also builds the .aab Play requires. The APK is what you sideload and what performance
# is measured on; Play accepts only the bundle, so a release needs both and they are built from one
# cdylib cross-compile rather than two.
#
# It differs from build-and-run.sh (the debug loop) in two ways that matter:
#
#   1. The cdylib is built WITHOUT the `dev-harness` feature. That feature compiles in a custom-root
#      TLS path so a debug build can trust the local Stalwart harness's self-signed certificate. It
#      is inert unless MAILCAL_EXTRA_CA is set, but "inert" is not "absent", and a shipped binary
#      has no business being able to trust a certificate a file on disk told it to.
#
#   2. Gradle runs R8: shrink, optimise, obfuscate. That is not a size tweak. An unminified Compose
#      build is several times slower than a minified one, so a debug build is not evidence about how
#      the app performs, it is evidence about how the debug build performs.
#
# Signing: put a gitignored `app/keystore.properties` beside the module (storeFile, storePassword,
# keyAlias, keyPassword) and the APK comes out signed. Without one it builds UNSIGNED, deliberately,
# so that R8 and its keep rules are still exercised on every machine and in CI. R8 breakage in the
# JNA bindings does not fail the build; it fails at runtime, in the build you ship (see
# app/proguard-rules.pro).
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)" # the repository root

# Every registration this build uses must be present, or the compile fails naming the missing ones
# (crates/mailcal-oauth/build.rs). A build without them is legitimate, it simply offers no Google
# or Microsoft sign-in, which is exactly why a *shipped* one has to be refused rather than
# produced: it would look correct everywhere except in front of a user.
export MAILCAL_REQUIRE_INJECTED_CONFIG=1

export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
NDK_DIR="$(ls -d "$ANDROID_HOME/ndk/"*/ 2>/dev/null | sort -V | tail -1)"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${NDK_DIR%/}}"
ADB="$ANDROID_HOME/platform-tools/adb"
# Every ABI the APK ships, and the Rust target each one needs. This list is the twin of
# `defaultConfig.ndk.abiFilters` in app/build.gradle.kts: an ABI Gradle packages but cargo-ndk did
# not build installs fine and then dies at the first `System.loadLibrary`, so the two must agree.
# check-android-native-libs.sh asserts that on the packaged artifact at the end of this script.
ABIS=(arm64-v8a x86_64)
RUST_TARGETS=(aarch64-linux-android x86_64-linux-android)
# The installed package name follows the brand (docs/branding.md), so it is read rather than
# written here: an unbranded build installs under a different id, and a script holding this one
# would install fine and then launch nothing.
# shellcheck source=scripts/dev/brand.sh
. "$ROOT/scripts/dev/brand.sh"
brand_load
PKG="$MAILCAL_APP_ID"

INSTALL=false
BUNDLE=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --install) INSTALL=true; shift ;;
    --bundle) BUNDLE=true; shift ;;
    *) echo "unknown flag: $1" >&2; exit 2 ;;
  esac
done

echo "==> Cross-compiling the release cdylib for ${ABIS[*]} (NO dev-harness)"
command -v cargo-ndk >/dev/null 2>&1 || cargo install cargo-ndk
rustup target add "${RUST_TARGETS[@]}" >/dev/null
# The debug loop's jniLibs may carry a dev-harness build; remove it so a stale one cannot be picked
# up and shipped. Nothing here is precious, it is all regenerated below.
rm -rf "$HERE/app/src/main/jniLibs"
# The Allodia sign-in, when this build was given the registration that turns it on -- derived from
# that registration rather than asked for separately, so the two halves cannot disagree
# (scripts/dev/lib.sh, BUILDING.md). It resolves to nothing in a build from source.
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"
CORE_FEATURES=()
ALLODIA_FEATURE="$(core_cargo_features)"
if [ -n "$ALLODIA_FEATURE" ]; then
  CORE_FEATURES=(--features "$ALLODIA_FEATURE")
fi
NDK_ABI_ARGS=()
for abi in "${ABIS[@]}"; do NDK_ABI_ARGS+=(-t "$abi"); done
# DWARF off for this build only, which is what decides the symbol content of a shipped Android
# app. The symbol TABLE is the linker's and stays, so a crash stack in the user's diagnostic log
# still names every function; only the file and line go.
#
# Measured on the arm64 slice, deflated, which is what a device downloads:
#
#   fully stripped     10.4 MiB   bare addresses          <- what AGP's strip would give
#   symbol table       11.9 MiB   function names          <- here
#   line tables        24.2 MiB   names, file and line
#   what shipped       40.6 MiB   names, file and line, for every dependency too
#
# Names are what a support log needs to be readable; file and line cost more than double for a
# precision nobody reading one has asked for. docs/logging.md holds the whole trade, including why
# app/build.gradle.kts exempts this one library from AGP's strip (AGP's is `--strip-unneeded`,
# which takes the symbol table with the DWARF and would leave `<unknown>` at every frame).
#
# One env var rather than a strip afterwards, and the difference is not style. `llvm-strip
# --strip-debug` on the output produces a library 0.24 MiB smaller deflated, it also drops local
# symbols, but it means locating a tool inside the NDK by hand, and the path that did so was
# wrong on Windows: Git Bash's `uname -s` is `MINGW64_NT-…` where the NDK directory is
# `windows-x86_64`. Not generating the DWARF is also strictly less work than generating ~60 MiB of
# it per ABI and discarding it.
#
# `CARGO_PROFILE_RELEASE_STRIP=debuginfo` is the equivalent one-liner if a build ever needs the
# stripping to happen after codegen instead; measured, the two agree to within 2.4 KiB deflated.
(cd "$ROOT" && CARGO_PROFILE_RELEASE_DEBUG=0 cargo ndk "${NDK_ABI_ARGS[@]}" -o "$HERE/app/src/main/jniLibs" build -p mailcal-bindings --release ${CORE_FEATURES[@]+"${CORE_FEATURES[@]}"})

for abi in "${ABIS[@]}"; do
  so="$HERE/app/src/main/jniLibs/$abi/libmailcal_bindings.so"
  [ -f "$so" ] || { echo "error: cargo-ndk produced no $abi cdylib at $so" >&2; exit 1; }
done

# The Kotlin binding + the localised resources are generated by Gradle itself (see the
# "Generated sources" block in app/build.gradle.kts), so every entry point, this script, Android
# Studio, `./gradlew :app:test`, and CI, regenerates them from one definition instead of each
# remembering to. Nothing to do here.

echo "==> Assembling the release APK (R8: shrink + optimize + obfuscate)"
(cd "$HERE" && ./gradlew --quiet :app:assembleRelease)

SIGNED="$HERE/app/build/outputs/apk/release/app-release.apk"
UNSIGNED="$HERE/app/build/outputs/apk/release/app-release-unsigned.apk"

if [[ -f "$SIGNED" ]]; then
  APK="$SIGNED"
  echo "==> Signed with the upload key from app/keystore.properties"
else
  APK="$UNSIGNED"
  echo "==> UNSIGNED (no app/keystore.properties). Fine for measuring; not installable as-is."
fi
echo "    $APK"

# Google Play is a remote gate, and it rejects an upload whose native libraries are not 16 KB page
# aligned, costing a build number and a slow round-trip. The property is per-`.so` and per-ABI, so
# "the build succeeded" says nothing about it; assert it here, where it costs a second.
"$ROOT/scripts/dev/check-android-native-libs.sh" "$APK"

if [[ "$BUNDLE" == true ]]; then
  echo "==> Bundling the release .aab (the only format Play accepts)"
  (cd "$HERE" && ./gradlew --quiet :app:bundleRelease)
  AAB="$HERE/app/build/outputs/bundle/release/app-release.aab"
  [[ -f "$AAB" ]] || { echo "bundleRelease produced no .aab at $AAB" >&2; exit 1; }
  echo "    $AAB"
  # The same remote gate, asserted on the artifact that actually goes to Play. The APK passing says
  # nothing about the bundle: they are packaged separately, from the same .so files.
  "$ROOT/scripts/dev/check-android-native-libs.sh" "$AAB"
fi

if [[ "$INSTALL" == true ]]; then
  if [[ "$APK" == "$UNSIGNED" ]]; then
    # An unsigned APK cannot be installed. Sign it with the local debug key purely so it can be run
    # on a developer's own device, this is a measurement build, never a distributable one.
    BT="$(ls -d "$ANDROID_HOME/build-tools/"* | sort -V | tail -1)"
    echo "==> Signing with the local DEBUG key so it can be installed (measurement only, never ship this)"
    "$BT/zipalign" -f -p 4 "$APK" "$HERE/app/build/outputs/apk/release/aligned.apk"
    "$BT/apksigner" sign \
      --ks "$HOME/.android/debug.keystore" --ks-pass pass:android \
      --ks-key-alias androiddebugkey --key-pass pass:android \
      --out "$HERE/app/build/outputs/apk/release/debug-signed.apk" \
      "$HERE/app/build/outputs/apk/release/aligned.apk"
    APK="$HERE/app/build/outputs/apk/release/debug-signed.apk"
  fi
  echo "==> Installing + launching"
  "$ADB" wait-for-device
  "$ADB" install -r "$APK"
  "$ADB" shell am start -n "$PKG/.MainActivity" >/dev/null
  echo "==> Running. R8 breakage in the JNA bindings shows up HERE, not at build time, watch for it:"
  echo "    \"$ADB\" logcat -s Mailcal AndroidRuntime"
fi
