# Windows client

A **WinUI 3** (Windows App SDK) desktop app rendering the mailbox-list snapshot driven by
the Rust core through the UniFFI `MailcalApp` object, the empirical proof that the reactive
Rust → WinUI binding holds.

The Rust core is identical across platforms; only the renderer differs. The same
`dispatch(Intent)` in / `surface_changed` + immutable-snapshot out loop drives all three,
here through **C#** bindings (UniFFI has no built-in C# target, so the generator is
NordSecurity's [`uniffi-bindgen-cs`](https://github.com/NordSecurity/uniffi-bindgen-cs)).

## Run

```pwsh
./build-and-run.ps1                 # build + run for the host arch (Debug), the everyday form
./build-and-run.ps1 -Arch x64       # cross-build the x64 client (see the warning below)
./build-and-run.ps1 -Configuration Release -NoRun
```

**Leave `-Arch` off unless you specifically need the other architecture.** It defaults to the
host, so naming an arch can only ever *cross*-build, which recompiles the Rust cdylib for a
target `cargo test` did not just build, **and drops the two gates that have to execute**: the
`MailcalVerify` FFI check is skipped (`Skipping the native gate (cross-arch build: …)`) and the
app is never launched, since a cross-arch binary cannot run here. The slower run proves less.
Don't copy the `windows` CI job's `-Arch x64`, that runner *is* x64, so for it that is the
native form. Both arches together are a **release-packaging** concern (`./package.ps1`), immediately before a
Store submission.

It:

- builds the Rust cdylib for the chosen arch (`-Arch arm64` | `x64`, default = host),
  adding the Rust target on demand and emitting under `target/<triple>/` so native and
  cross-compiled builds share one layout;
- generates the C# bindings from that cdylib into `Generated/` (arch-independent);
- builds + runs **`MailcalVerify`**, a headless gate that drives the demo loop and asserts
  the four seeded rows cross into C# (the deterministic check, the twin of macOS
  `verify.swift`), run when the build arch is the host's so it executes natively;
- builds the **`Mailcal`** WinUI app for the paired RID (`win-arm64` / `win-x64`),
  bundling the arch-matched native DLL, and launches it (host arch only).

### A dev build claims its own URI scheme (and why you may need one cleanup)

Browser sign-ins come back through a custom URI scheme, and **Windows registers a protocol per
user, not per build**. A developer machine typically has the Store app installed *and* runs this
dev loop, so if both claimed one scheme, the OS could not tell them apart: it puts up a
**"select an app" picker for a redirect carrying a live one-time auth code**, and an accidental
"Always" wires every future sign-in, *including the shipped app's*, to the wrong build,
silently and permanently.

So the scheme depends on packaging, decided by `AppIdentity.IsPackaged` (**not** `#if DEBUG`, a
Release *unpackaged* build is still a dev build sitting beside the Store app):

| Build | Scheme | Registered by |
|---|---|---|
| Packaged (Store/MSIX, `package.ps1`) | `eu.allodia.mailcal` | `Package.appxmanifest`, at install |
| Unpackaged dev (`build-and-run.ps1`) | `eu.allodia.mailcal.dev` | `Program.RegisterProtocolForUnpackaged`, at every launch |

Both are registered as Azure redirect URIs, so Microsoft sign-in works in either. JMAP needs no
portal entry at all (RFC 7591 dynamic registration sends whatever redirect URI we hand it), and
Google is unaffected (loopback).

### Screenshot / demo mode

Set **`MAILCAL_SHOWCASE=1`** before launching to boot into an in-memory **showcase** dataset
instead of the account-setup form, two fictional accounts with a full mailbox (folders, a
threaded conversation, an attachment, a remote-image newsletter) and a calendar, all from
bundled sample content. No network, no credential store, so **no personal mail can leak into a
store screenshot**:

```pwsh
$env:MAILCAL_SHOWCASE = 'en'; ./build-and-run.ps1       # English chrome + English sample mail
$env:MAILCAL_SHOWCASE = 'nl'; ./build-and-run.ps1       # Dutch chrome + Dutch sample mail
$env:MAILCAL_SHOWCASE = 'de'; ./build-and-run.ps1       # …any catalog locale: en nl de fr es it pt
$env:MAILCAL_SHOWCASE = '1';  ./build-and-run.ps1       # follow the app's own language choice
```

**`MAILCAL_SHOWCASE_SCREEN`** picks the screen the run drives to, `list` (default) · `reply` ·
`settings` · `add-account` · `calendar` · `invitation`, without a single pixel tap. To capture the
whole listing set, don't do it by hand: `scripts/dev/showcase.sh windows` builds once, then
relaunches the app per (locale, screen) and shoots the window, writing 42 PNGs to
`showcase-screenshots/`. Per-shot equivalent:

```pwsh
./showcase.ps1 -Locale nl -Screen reply -Out shot.png
```

**The window rect is bigger than the window.** A sizeable window's `GetWindowRect` is inflated by an
*invisible* resize border (`SM_CXSIZEFRAME + SM_CXPADDEDBORDER`, 13 px at 200%, none at the top on a
restored window) that exists only for the mouse to grab. The app paints none of it, so `PrintWindow`
left it at the bitmap's zero-fill and every capture came out framed in a black L, which shipped into
the committed store set before anyone looked. `screenshot.ps1` crops that margin off and then
**asserts** no fully-black edge survives; showcase mode inflates its window by the same inset
(`ShowcaseFrameInset`), so what's left is 1440x900 logical exactly. Don't reach for
`DWMWA_EXTENDED_FRAME_BOUNDS` here: it is 2 px short per side, because DWM draws the visible border
itself and `PrintWindow`, which asks the *app* to render, can't produce pixels the app doesn't own.

The geometry lives in `screenshot-frame.ps1` and is covered by `screenshot-frame.tests.ps1`, which
`build-and-run.ps1` runs (so the `windows` CI job gates it) and which needs no app, window or
display. The shape that fooled us is a test case: because the caption is drawn across the *full*
window rect, an uncropped side margin is an **L**, not a rectangle, measure it down whole edge
columns and you find nothing at all.

`showcase.ps1` refuses to fire the shutter unless it has *proved* the app is in showcase mode: the
built assembly must carry the driver (a stale or Release binary would open your real accounts), and
the launched process must have logged that it entered showcase mode in the requested locale. A
screenshot of a real, fully-populated mailbox is a perfectly plausible PNG, file size cannot tell
you, so the check is made positively, before the app opens anything.

**Every listing language.** The Store wants a screenshot set per language, and Dutch chrome over
English mail reads as broken, so the var names the language: any locale the shared catalog ships
(`en` · `nl` · `de` · `fr` · `es` · `it` · `pt`) pins the UI *and* seeds the mailbox, folder names,
and calendar in that language, for this launch only (the developer's stored Settings language is
untouched). `MAILCAL_SHOWCASE=1` seeds whichever language the app already renders in. Every seed
carries the same messages, so each screenshot has its twin in all the others.

It backs `MailcalApp.NewShowcase(…, ShowcaseLocale)` (bindings `new_showcase` →
`boot::build_showcase`), kept separate from the four-row `new_demo` fixture the headless gate
asserts on. `ShowcaseMode.IsOn` is `#if DEBUG`-gated, as on Apple (`#if DEBUG`) and Android
(`FLAG_DEBUGGABLE`), so a shipped build ignores the flag outright rather than trusting nobody sets
it, `scripts/ci/check-showcase-flag.sh` keeps that guard, and the launcher's mailbox banner, honest.

**Dual-arch:** both `arm64` and `x64` are first-class. .NET 10 + the Windows App SDK build
each RID; the script pairs each with the matching Rust target (`aarch64-pc-windows-msvc` /
`x86_64-pc-windows-msvc`). On an arm64 host the x64 client is a cross-compile, and vice
versa.

## Prerequisites

- **Rust** with the MSVC toolchain (the script adds the second arch's target itself).
- **.NET 10 SDK** (`dotnet --version` ≥ 10).
- **Windows App SDK 2.x**, pulled in automatically as the `Microsoft.WindowsAppSDK` NuGet
  package (currently `2.4.0`); the app is unpackaged and WinAppSDK-self-contained, so no
  separate runtime install and no MSIX are needed.
- **Windows 11 SDK `10.0.26100`** (24H2), the build targets it; the OS floor is Windows 10
  2004 (`10.0.19041`). **Visual Studio 2026** (or its Build Tools) provides both.
- **`uniffi-bindgen-cs`** needs **no install**, it's vendored as the `mailcal-bindgen-cs`
  binshim crate and the scripts run it via `cargo run -p mailcal-bindgen-cs`, so the generator's
  version is pinned in `Cargo.lock` (the tag tracks the core's UniFFI 0.31, a generator built
  against a different UniFFI can't read the cdylib's metadata). It's excluded from the workspace
  `default-members`, so a bare `cargo build` and non-Windows hosts never compile it.

Latest-only, no legacy: the app's compile + runtime closure is **.NET 10** exclusively (no
.NET Framework). Verified on an `aarch64-pc-windows-msvc` host: Windows 11, VS Community
2026 (18.7), .NET 10.0.302, Windows App SDK 2.4.0, Windows 11 SDK 26100, Rust 1.96.

## Packaging for the Microsoft Store

The dev loop above is **unpackaged**, loose files, no MSIX, no signing, which is what makes
`dotnet run` ergonomic. Shipping is a separate, opt-in build path:

```pwsh
./package.ps1                    # dual-arch (x64 + arm64) Store .msixupload, Release
./package.ps1 -Version 1.2.0.0   # stamp the package version
```

`package.ps1` builds both Rust cdylibs, regenerates the bindings, then drives **MSBuild**'s MSIX
packaging targets to emit an **unsigned `.msixupload`**. You **upload that to Partner Center
(the Allodia account); Microsoft signs it on ingestion**, so no code-signing certificate is
needed and there's no SmartScreen prompt for users. Submitting to the Store *also* makes the app
installable via `winget`.

What `-p:Packaged=true` flips (see `Mailcal.csproj`):

- **Unpackaged → MSIX** (`WindowsPackageType`), pulling in `Package.appxmanifest` + the `Images/`
  tiles.
- **WinApp SDK self-contained → framework-dependent**, the Store delivers the WinApp SDK
  framework package as a dependency, so it stays out of the upload.
- **.NET → self-contained per-RID**, so the package runs on a clean Windows 10 (2004+) box with
  no .NET 10 installed. Each arch in the bundle is paired with its matching Rust `mailcal_bindings.dll`.

Before the **first** upload, two things must be real:

1. **Identity**, replace the placeholders in `Package.appxmanifest` (`Identity/@Name`,
   `Identity/@Publisher`, `PublisherDisplayName`) with the values from the Allodia app
   reservation in Partner Center (or use VS *Associate App with the Store*). The Store rejects a
   mismatched identity.
2. **Tile/store art**, `Images/*.png` (MSIX tiles/splash/store logo) and `Images/app.ico` (the
   exe/taskbar/title-bar icon) are derived from the brand source icon via
   `Images/generate-assets.ps1`, which resolves it the same way every other client's generator does
   ([`docs/branding.md`](../../docs/branding.md)). `System.Drawing` makes this the one generator
   that cannot run off Windows, so a rebrand done elsewhere leaves these on the previous art.

Native AOT and `PublishSingleFile` are intentionally **not** used: both have open
WinUI 3 + .NET 10 regressions. ReadyToRun is a safe opt-in (`/p:PublishReadyToRun=true`) once the
basic upload is proven.

The Store also rejects any package whose version has a **non-zero revision** (the fourth field),
it reserves that field for its own repackaging. Partner Center only says so *after* the upload
completes, so `package.ps1` refuses such a `-Version` up front. Bump the build field:
`1.0.0.0` → `1.0.1.0`, not `1.0.0.1`.

### The Rust cdylib links the C runtime statically

The workspace [`.cargo/config.toml`](../../.cargo/config.toml) sets
`-C target-feature=+crt-static` for the two MSVC triples, so **every** `cargo build` of this
workspace produces a cdylib with no CRT dependency, not only the ones these scripts drive.
`rust-crt.ps1` is the gate that proves it held; both build scripts call it. Do not bypass it.

Rust's `*-pc-windows-msvc` targets link the CRT *dynamically* by default, so the cdylib imports
`VCRUNTIME140.dll`, a file that ships in the **Visual C++ Redistributable**, not in Windows. Every
dev box has it (Visual Studio installs it) and a clean machine does not, and nothing in the MSIX
package graph supplies it: we depend only on `Microsoft.WindowsAppRuntime.2`, which declares no
dependencies of its own, so `Microsoft.VCLibs.140.00.UWPDesktop` (which does carry
`vcruntime140.dll`) is never pulled in. Microsoft's own `Microsoft.UI.Xaml.dll` links its CRT
statically for exactly this reason.

On a clean machine the dynamic build therefore fails `LoadLibrary` with `ERROR_MOD_NOT_FOUND`. The
first P/Invoke, `MailboxModel`'s `AvailableZones` field initializer, throws `DllNotFoundException`
inside `App.OnLaunched`, WinUI fail-fasts, and the app dies at launch with `0xc000027b`
(`STATUS_STOWED_EXCEPTION`), blaming `Microsoft.UI.Xaml.dll`. **This is what failed Microsoft Store
certification for 1.0.0.0**, and it reproduces on *both* architectures, the cert host merely
happened to be x64. It cannot be caught by `build-and-run.ps1` on a developer machine, which is why
the assert runs in CI on both the per-commit Debug build and the release `.msixupload`.

The trade is that CRT fixes now arrive with an app rebuild rather than Windows Update. That is
acceptable here: a Rust cdylib touches almost none of the CRT. (`cc` reads `crt-static` back out of
the target features, so `libsqlite3-sys` and `ring` compile with `/MT` and stay consistent.)

A gate is still needed even though the config file pins the flag, because a `RUSTFLAGS` environment
variable **suppresses** `target.*.rustflags` outright (Cargo precedence) and would silently restore
the dynamic CRT. `Assert-StaticCrt` checks the built cdylib; `Assert-StaticCrtInPackage` re-checks
the bytes inside the shipped `.msixupload` → `.msixbundle` → `.msix`, since the first assert only
covers the file MSBuild copies *from*.

### Testing the install on a device

The Store upload is **unsigned** (Microsoft signs it on ingestion), so it can't be sideloaded as-is
, Windows refuses to install an unsigned MSIX. To smoke-test the install on a real device, build a
**self-signed** set instead:

```pwsh
./package.ps1 -Sign
```

This mints a throwaway dev cert whose subject matches the manifest `Publisher` (kept in
`CurrentUser\My`, reused across runs), signs the bundle, and emits
`AppPackages/Mailcal_<ver>_Test/` with the signed `.msixbundle` and its `.cer`. Install it
directly (the command is printed at the end of the run):

```pwsh
# 1. Trust the dev cert once (elevated):
Import-Certificate -FilePath .\Mailcal_<ver>_x64_arm64.cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople
# 2. Install, or upgrade in place on a rebuild:
Add-AppxPackage -Path .\Mailcal_<ver>_x64_arm64.msixbundle
```

A clean device also needs the **WinApp SDK runtime** (`Microsoft.WindowsAppRuntime`): it's
preinstalled on dev boxes; on a bare device install it from the Store/`winget` first. **Local
testing only, never distribute a self-signed build;** real distribution is Store-signed.

> **Why not `Add-AppDevPackage.ps1`?** The SDK's one-click sideload helper (`Add-AppDevPackage.ps1`
> + bundled `Dependencies/`) is **not** emitted for the **dual-arch bundle**: the SDK copies it into
> the `_Test` folder, then the bundle step (`AppxBundle=Always`) deletes and repacks that folder with
> only the `.msixbundle` + `.cer`, dropping the helper (a `microsoft.windows.sdk.buildtools.msix`
> 1.7.x limitation, it survives only for a single-arch package). The two commands above are the
> equivalent direct install.

`-Sign` auto-stamps a monotonically increasing version (Build = days since 2000, Revision =
seconds-since-midnight / 2), so re-running it and reinstalling **upgrades in place**, no need to
remove the old package first (MSIX blocks reinstalling the same `1.0.0.0` with changed content,
`0x80073CFB`). Pass `-Version x.y.z.w` to pin it instead.

## Files

- `Mailcal/`, the WinUI 3 app.
  - `App.xaml(.cs)`, `MainWindow.xaml(.cs)`, the app + the shell (NavigationView sidebar,
    detail host, time-zone-changed prompt).
  - `Views/`, `AccountSetupView`, `MailListView` (+ `MailRowTemplateSelector`),
    `CalendarView`.
  - `Dialogs/`, `RichComposeDialog` (the shared rich composer for new/reply/forward),
    `EventEditorDialog` (create + edit, with the writable-calendar picker), `EventDetailDialog`
    (the tap-to-open detail, with Edit/Delete), `CalendarManagerDialog`.
  - `Services/`, `MailboxModel` (the Observer pump + intent dispatch + a live OS
    time-zone-change watcher via `SystemEvents.TimeChanged`, the twin of macOS
    `MailcalModel.swift`), `CredentialStore` (Windows Credential Manager = the OS secure
    store), `TimeZones` (IANA-zone formatting + list).
  - `ViewModels/RowViewModels.cs`, the public, render-ready row types the XAML binds to
    (the generated UniFFI types are `internal`).
  - `Package.appxmanifest`, MSIX package identity + capabilities (Store build only).
  - `Images/`, launcher art: MSIX tile/store PNGs + `app.ico` (exe icon), all derived from the
    brand source icon by `generate-assets.ps1`.
- `MailcalVerify/`, the headless runtime gate (no UI, no network: drives the demo loop).
- `build-and-run.ps1`, cdylib → bindings → gate → WinUI app → launch (the dev loop).
- `package.ps1`, cdylib (both arches) → bindings → MSIX bundle → `.msixupload` (the Store path);
  `-Sign` instead builds a self-signed, installable sideload set for on-device testing.
- `rust-crt.ps1`, dot-sourced by both scripts: links the cdylib's C runtime statically and asserts
  the shipped DLL imports none. See "The Rust cdylib links the C runtime statically" above.
- `Generated/`, `**/bin/`, `**/obj/`, `**/AppPackages/`, build artifacts (gitignored; rebuilt).

Credentials live in the **Windows Credential Manager**, never a plaintext file, the
Windows counterpart of the macOS Keychain and Android EncryptedSharedPreferences. On first
run the app shows the account-setup form (no seed file); it writes the store, then connects.

This is a spike; the eventual home is the `allodia-clients` repo with a full Visual Studio
solution. It lives here so the binding can be proven on real Windows hardware. Visual
Studio isn't required, `dotnet` + the script cover build and run from the CLI.
