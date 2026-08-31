#!/usr/bin/env bash
# Assert, on a packaged Android artifact, the two native-code invariants Google Play checks at
# upload; before we spend a build number finding out.
#
#     scripts/dev/check-android-native-libs.sh <app.apk|app.aab>
#
# Play's "your app doesn't support 16 KB memory pages" error is a property of the ELF LOAD
# segments of every `.so` in the artifact: each must declare `p_align` >= 16 KB, or the app cannot
# be mapped on a device with 16 KB pages. It is *per-ABI*, which is what makes it easy to get
# wrong; a dependency's arm64 build can be aligned while its x86_64 build is not, and the
# rejection names neither. That is exactly how it bit us: JNA 5.14 ships one `libjnidispatch.so`
# per ABI and only the arm64 one was aligned, so the upload was rejected over ABIs we did not even
# build for. (JNA 5.19.1 aligns all of them; hence the version floor in app/build.gradle.kts.)
#
# So this checks three things:
#
#   1. Only the expected ABIs are packaged (app/build.gradle.kts pins `abiFilters`; an AAR can
#      otherwise reintroduce an ABI silently, and a filter that stops filtering looks exactly
#      like a filter that works).
#   2. Each expected ABI is present AND carries our own cdylib. Gradle happily packages an ABI
#      for which cargo-ndk built nothing; the APK then installs on such a device and dies at the
#      first `System.loadLibrary`, which no build-time error and no store check would have caught.
#   3. Every LOAD segment of every packaged `.so` aligns to >= 16 KB.
#
# Not checked: PT_GNU_RELRO padding. Android Studio's APK Analyzer warns when a RELRO segment is
# not a suffix of its LOAD segment (JNA's arm64 build is not, and no JNA release fixes it), but
# that is a runtime-hardening advisory, not a 16 KB alignment failure; Play warns, it does not
# reject. Do not "fix" it here by failing the build.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/dev/lib.sh
source "$HERE/lib.sh"

ARTIFACT="${1:-}"
[[ -n "$ARTIFACT" ]] || die "usage: $(basename "$0") <app.apk|app.aab>"
[[ -f "$ARTIFACT" ]] || die "no such file: $ARTIFACT"

# Must match `defaultConfig.ndk.abiFilters` in clients/android/app/build.gradle.kts and the `ABIS`
# arrays in clients/android/build-{and-run,release}.sh.
ALLOWED_ABIS=(arm64-v8a x86_64)
CORE_LIB="libmailcal_bindings.so"
MIN_ALIGN=16384 # 16 KB

is_allowed_abi() {
  local candidate="$1" abi
  for abi in "${ALLOWED_ABIS[@]}"; do
    [[ "$abi" == "$candidate" ]] && return 0
  done
  return 1
}

# llvm-readelf from the NDK; it is the only ELF reader guaranteed present on a machine that can
# build this client, and the host `readelf`/`otool` cannot read Android ELF on macOS.
readelf_bin() {
  if command -v llvm-readelf >/dev/null 2>&1; then command -v llvm-readelf; return; fi
  local sdk
  for sdk in "${ANDROID_HOME:-}" "$HOME/Library/Android/sdk" "${LOCALAPPDATA:-}/Android/Sdk"; do
    [[ -n "$sdk" ]] || continue
    local found
    found="$(ls "$sdk"/ndk/*/toolchains/llvm/prebuilt/*/bin/llvm-readelf 2>/dev/null | sort -V | tail -1)"
    [[ -n "$found" ]] && { printf '%s\n' "$found"; return; }
  done
}

READELF="$(readelf_bin)"
[[ -n "$READELF" ]] || die "llvm-readelf not found: install an NDK under \$ANDROID_HOME"
require_cmd unzip

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# An APK holds `lib/<abi>/*.so`; an app bundle holds `<module>/lib/<abi>/*.so`. Both match on the
# `lib/<abi>/` infix, so one glob covers the two shapes.
unzip -q -o "$ARTIFACT" '*/lib/*/*.so' 'lib/*/*.so' -d "$WORK" 2>/dev/null || true

# Collected the long way round because macOS ships bash 3.2, which has no `mapfile`.
LIBS=()
while IFS= read -r found; do LIBS+=("$found"); done < <(find "$WORK" -name '*.so' | sort)
[[ ${#LIBS[@]} -gt 0 ]] || die "no native libraries found in $(basename "$ARTIFACT"): is this an Android artifact?"

info "Checking ${#LIBS[@]} native librar$([[ ${#LIBS[@]} -eq 1 ]] && echo y || echo ies) in $(basename "$ARTIFACT")"

failures=0
for lib in "${LIBS[@]}"; do
  abi="$(basename "$(dirname "$lib")")"
  rel="lib/$abi/$(basename "$lib")"

  if ! is_allowed_abi "$abi"; then
    printf '  FAIL %-52s unexpected ABI %s: only %s ship (defaultConfig.ndk.abiFilters)\n' \
      "$rel" "$abi" "${ALLOWED_ABIS[*]}"
    failures=$((failures + 1))
    continue
  fi

  # `llvm-readelf -l` prints one row per program header; the last field of a LOAD row is p_align,
  # as hex (`0x4000`), which bash arithmetic reads directly. Every LOAD segment must clear 16 KB,
  # one 4 KB segment fails the whole artifact, so report the smallest.
  worst=""
  loads=0
  while read -r hex; do
    [[ -n "$hex" ]] || continue
    loads=$((loads + 1))
    align=$((hex))
    if ((align < MIN_ALIGN)) && { [[ -z "$worst" ]] || ((align < worst)); }; then worst="$align"; fi
  done < <("$READELF" -l "$lib" | awk '$1 == "LOAD" { print $NF }')

  # No LOAD rows means we did not actually read an ELF; an unparsed file must not pass silently,
  # or "ok" would mean "we found nothing to object to" rather than "we checked".
  if ((loads == 0)); then
    printf '  FAIL %-52s no ELF LOAD segments read: unreadable or not an ELF\n' "$rel"
    failures=$((failures + 1))
    continue
  fi

  if [[ -n "$worst" ]]; then
    printf '  FAIL %-52s LOAD alignment %s bytes, 16384 required (16 KB page sizes)\n' "$rel" "$worst"
    failures=$((failures + 1))
  else
    printf '  ok   %-52s\n' "$rel"
  fi
done

# An ABI that is packaged but has no cdylib is the one failure Play does *not* catch; it is a
# crash on the user's device, not a rejected upload. Check the expected set is complete, so a
# cargo-ndk target that silently stopped building cannot pass as "no libraries to object to".
for abi in "${ALLOWED_ABIS[@]}"; do
  if [[ -z "$(find "$WORK" -path "*/$abi/$CORE_LIB" -print -quit)" ]]; then
    printf '  FAIL %-52s missing: %s would install and crash at System.loadLibrary\n' \
      "lib/$abi/$CORE_LIB" "$abi"
    failures=$((failures + 1))
  fi
done

# What our own cdylib carries, which nothing else in the pipeline checks and three separate things
# can silently change: AGP's strip task copies a library through unchanged whenever its tool is
# missing or exits non-zero, and `keepDebugSymbols` exempts a file before it even looks for one.
# Every one of those failures reports success, so the outcome is asserted on the artifact instead.
#
# Both directions are a defect. Without `.symtab` a Rust backtrace in the user's diagnostic log is
# `<unknown>` at every frame; with `.debug_*` still present the download more than doubles for line
# numbers nobody reading a support log has asked for. docs/logging.md holds the trade.
while IFS= read -r lib; do
  abi="$(basename "$(dirname "$lib")")"
  rel="lib/$abi/$CORE_LIB"
  sections="$("$READELF" -S "$lib" 2>/dev/null || true)"
  if ! grep -q '\.symtab' <<<"$sections"; then
    printf '  FAIL %-52s no .symtab: a crash stack in the log would name no function\n' "$rel"
    failures=$((failures + 1))
  elif grep -q '\.debug_' <<<"$sections"; then
    printf '  FAIL %-52s .debug_* present: built without build-release.sh, or its DWARF override went\n' "$rel"
    failures=$((failures + 1))
  else
    printf '  ok   %-52s symbol table, no DWARF\n' "$rel"
  fi
done < <(find "$WORK" -name "$CORE_LIB" | sort)

if ((failures > 0)); then
  die "$failures problem$([[ $failures -eq 1 ]] || echo s) found: do not upload this build"
fi
info "All native libraries are 16 KB aligned, across exactly: ${ALLOWED_ABIS[*]}"
info "$CORE_LIB keeps its symbol table and carries no DWARF, on every ABI"
