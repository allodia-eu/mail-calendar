# Android client

A Jetpack Compose app rendering the mailbox-list snapshot driven by the Rust core through
the UniFFI `MailcalApp` object, the empirical proof that the reactive Rust → Compose
binding holds.

The Rust core is identical across platforms; only the renderer differs. The same
`dispatch(Intent)` in / `surface_changed` + immutable-snapshot out loop drives both.

## Run

```sh
./build-and-run.sh
```

It:

- cross-compiles the cdylib for `arm64-v8a` via `cargo-ndk` (installing it if needed) into
  `app/src/main/jniLibs/`;
- generates the Kotlin bindings (from the host cdylib) into `app/src/main/java/uniffi/`;
- assembles the debug APK with Gradle;
- installs + launches it on a connected device/emulator, then captures `screenshot.png`
  and prints the `Mailcal` round-trip log line.

Needs the Android SDK + an NDK under `$ANDROID_HOME` (default `~/Library/Android/sdk`) and a
booted device/emulator (`$ANDROID_HOME/emulator/emulator -list-avds`). Verified on an
`arm64-v8a` API-36 emulator with NDK 30, AGP 8.13, Kotlin 2.2.20, Gradle 9.3, JDK 21.

## ABIs: 64-bit only (`arm64-v8a` + `x86_64`)

The app ships two ABIs, and **three things must agree or the app breaks in a way no build error
reports**: `defaultConfig.ndk.abiFilters` in `app/build.gradle.kts`, the `ABIS` array in
`build-and-run.sh` and `build-release.sh`, and `ALLOWED_ABIS` in the checker below. Gradle packages
every ABI in the filter whether or not cargo-ndk built a cdylib for it, an ABI that is filtered in
but not compiled produces an APK that installs happily and then dies at the first
`System.loadLibrary`. Both scripts therefore always cross-compile **both** targets
(`aarch64-linux-android`, `x86_64-linux-android`), so the debug loop works on an arm64 device and
on an x86_64 emulator alike.

32-bit (`armeabi-v7a`, `x86`) is deliberately out: `minSdk` is 31, Play has required 64-bit for
years, and those folders only ever carried JNA's and Compose's own `.so`s riding in from their AARs.

**16 KB page alignment is a Play gate, and it is per-`.so`, per-ABI.** Google Play rejects an upload
whose native libraries' ELF LOAD segments align to less than 16 KB, and a dependency can be aligned
on one ABI and not another, which is precisely how it bit us: JNA **5.14**'s `x86_64`
`libjnidispatch.so` aligned to 4 KB while its `arm64-v8a` build was fine. **JNA 5.19.1 is a version
floor, not a preference**, it is the first release aligned on every ABI. Don't lower it.

`build-release.sh` asserts all of this on the packaged APK via
[`scripts/dev/check-android-native-libs.sh`](../../scripts/dev/check-android-native-libs.sh),
which fails on an unexpected ABI, on an ABI missing `libmailcal_bindings.so`, and on any LOAD
segment under 16 KB. **Run it on the `.aab` too before uploading** (`./gradlew :app:bundleRelease`)
The bundle is the artifact Play actually checks.

One warning survives all of this and is expected: Android Studio's APK Analyzer reports
`PT_GNU_RELRO … is not a suffix` on JNA's library. That is a runtime-hardening advisory, not an
alignment failure, Play warns, it does not reject. Don't chase it.

## Files

- `app/src/main/java/eu/allodia/mailcal/MainActivity.kt`, the Compose app (Observer →
  main-thread hop → snapshot render).
- `app/build.gradle.kts`, `build.gradle.kts`, `settings.gradle.kts`, Gradle config.
- `build-and-run.sh`, cdylib → bindings → APK → install/run.
- `app/src/main/jniLibs/` + `app/src/main/java/uniffi/`, generated build artifacts
  (gitignored; rebuilt by the script).

This is a spike; the eventual home is the `allodia-clients` repo with a full Android Studio
project. It lives here so the binding can be proven on a real device/emulator.
