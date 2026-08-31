#!/usr/bin/env bash
# Run the native-fault suite on a connected Android device or emulator; the one arm of
# crates/mailcal-bindings/src/native_fault.rs that no host gate reaches, because its `cfg` is
# selected by an Android target and `cargo test` never builds one.
#
# What it proves is that the install, the chain and the write work on bionic, and that the record
# Android writes is the banner alone; no frames, because bionic gained `backtrace(3)` at API 33
# and this client's minSdk is 31. Any API level proves that: it is a property of our own `cfg`, not
# of the device. (Widening that `cfg` fails at link time, before this suite runs.)
#
#   scripts/dev/test-android-native-fault.sh
#   scripts/dev/test-android-native-fault.sh --serial emulator-5554
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

SELF="$REPO_ROOT/scripts/dev/test-android-native-fault.sh"
SERIAL=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --serial) SERIAL="${2:?--serial needs an adb serial}"; shift 2 ;;
    -h|--help) sed -n '2,11p' "$SELF"; exit 0 ;;
    *) die "unknown argument '$1' (--serial)" ;;
  esac
done

ADB="$(adb_bin)"
[[ -n "$ADB" ]] || die "no adb found: install the SDK's platform-tools (or set ANDROID_HOME)"

# A device is a prerequisite, not an option: this script exists precisely because nothing else
# runs this code, so a silent skip would restore the gap it closes.
if [[ -z "$SERIAL" ]]; then
  # Collected the long way round because macOS ships bash 3.2, which has no `mapfile`; and macOS
  # is where a developer with a phone plugged in most often runs this.
  attached=()
  while IFS= read -r serial; do
    [[ -n "$serial" ]] && attached+=("$serial")
  done < <("$ADB" devices | tr -d '\r' | awk '$2 == "device" { print $1 }')
  case "${#attached[@]}" in
    0) die "no Android device or emulator attached: start one (scripts/dev/lib.sh knows the AVDs) and re-run" ;;
    1) SERIAL="${attached[0]}" ;;
    *) die "several devices attached (${attached[*]}): pick one with --serial" ;;
  esac
fi

# The device's own ABI, so this works on an arm64 phone and an x86_64 emulator alike. Building for
# the other one produces a binary the device cannot exec, several steps later.
abi="$("$ADB" -s "$SERIAL" shell getprop ro.product.cpu.abi | tr -d '\r')"
case "$abi" in
  arm64-v8a) rust_target="aarch64-linux-android"; ndk_abi="arm64-v8a" ;;
  x86_64) rust_target="x86_64-linux-android"; ndk_abi="x86_64" ;;
  *) die "unsupported device ABI '$abi': the APK ships arm64-v8a and x86_64 only" ;;
esac

api="$("$ADB" -s "$SERIAL" shell getprop ro.build.version.sdk | tr -d '\r')"
echo "==> $SERIAL: Android API $api, $abi ($rust_target)"

command -v cargo-ndk >/dev/null 2>&1 || cargo install cargo-ndk
rustup target add "$rust_target" >/dev/null

# cargo-ndk supplies the NDK linker and sysroot, the same way the cdylib cross-build does, so this
# script and clients/android/build-and-run.sh agree about what an Android build means.
echo "==> Cross-compiling the bindings test binary"
(cd "$REPO_ROOT" && cargo ndk -t "$ndk_abi" test -p mailcal-bindings --no-run)

# Found on disk rather than read from `--message-format=json`, because cargo-ndk swallows cargo's
# stdout: the JSON never reaches this script, and neither do the human-readable `Executable` lines
# that flag suppresses. The newest executable of the lib-test target is the one just built; every
# rebuild links a fresh one beside the last (AGENTS.md, "stale test binaries").
# `-exec ls -t {} +` rather than `-printf '%T@ %p'`: `-printf` is a GNU findutils extension and
# macOS ships BSD find, where it is not merely absent but *fatal*; and under `set -o pipefail`
# that kills the script at this assignment, before the `[[ -n ]]` guard below can say why.
binary="$(
  find "$REPO_ROOT/target/$rust_target/debug/deps" -maxdepth 1 -type f -perm -u+x \
    -name 'mailcal_bindings-*' ! -name '*.*' -exec ls -t {} + 2>/dev/null | head -1
)"
[[ -n "$binary" ]] || die "cargo produced no test binary for $rust_target"

remote=/data/local/tmp/mailcal-native-fault
cleanup() { "$ADB" -s "$SERIAL" shell rm -f "$remote" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> Pushing $(basename "$binary")"
"$ADB" -s "$SERIAL" push "$binary" "$remote" >/dev/null
"$ADB" -s "$SERIAL" shell chmod 755 "$remote"

# TMPDIR is what `std::env::temp_dir()` reads, and an adb shell sets none; without this the tests
# would write their scratch log to /tmp, which does not exist on Android.
echo "==> Running the native-fault suite on the device"
output="$("$ADB" -s "$SERIAL" shell "TMPDIR=/data/local/tmp $remote native_fault --nocapture; echo exit=\$?" | tr -d '\r')"
echo "$output"

# adb shell reports the *shell's* status, never the command's, so a failing suite exits 0 here.
# The echoed status above is the only truth about it.
grep -q '^exit=0$' <<<"$output" || die "the native-fault suite failed on $SERIAL"
grep -qE '^test result: ok\.' <<<"$output" || die "no test result line: the binary did not run on $SERIAL"
echo "==> Android native-fault suite passed (API $api, $abi)"
