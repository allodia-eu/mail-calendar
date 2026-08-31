#!/usr/bin/env pwsh
# Windows client build: Rust cdylib -> generated C# UniFFI bindings -> headless
# gate -> WinUI 3 app -> launch. The Windows twin of ../macos/build-and-run.sh and
# ../android/build-and-run.sh; same reactive loop, only the renderer differs.
#
# Dual-arch: -Arch arm64 (default on an arm64 host) or x64. The matching Rust target is
# added on demand and the cdylib is built under target/<triple>/, so native and
# cross-compiled builds share one deterministic layout. .NET 10 builds the WinUI app for
# the paired RID and bundles the arch-matched DLL.
#
#   ./build-and-run.ps1                 # build + run for the host arch (Debug)
#   ./build-and-run.ps1 -Arch x64       # cross-build the x64 client
#   ./build-and-run.ps1 -Configuration Release -NoRun
[CmdletBinding()]
param(
  [ValidateSet('arm64', 'x64')] [string] $Arch,
  [ValidateSet('Debug', 'Release')] [string] $Configuration = 'Debug',
  [switch] $NoRun
)
$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot
$root = (Resolve-Path "$here/../..").Path  # the repository root

# Assert the cdylib links the CRT statically (the flags live in the workspace .cargo/config.toml).
# A dynamic CRT crashes the app at launch on any machine lacking the Visual C++ Redistributable.
. (Join-Path $here 'rust-crt.ps1')

# The app reads its dev switches from the environment exactly once, at startup, and a stray one
# quietly opens a *different mailbox* than the developer expects, the showcase dataset, or the
# harness, while everything on screen still looks like a normal debug run. The only trace is a
# line in app.log, which nobody reads when the app came up fine. Both switches are legitimate
# (scripts/dev/boot.sh sets MAILCAL_DEV_ACCOUNT on purpose), so this doesn't refuse the launch, it
# just makes the mailbox impossible to mistake, and says how to get back to real accounts. (The one
# exception is a stalwart-imap launch with no usable harness CA: that cannot connect at all, so it
# refuses rather than dropping the developer on a setup form with an opaque TLS error.)
# The parsing mirrors ShowcaseMode.IsOn / MailboxModel.ResolveDevAccount so the banner can't
# disagree with what the app actually does, including that BOTH switches are compiled out of a
# Release build (`#if DEBUG`), so a Release launch always opens the real accounts no matter what is
# set (as is the harness-CA trust the IMAP mode needs, which the Rust core gates on debug_assertions).
# (Interpolate rather than use `?.`: `$env:X?.Trim()` parses the `?` as part of the variable name.)
$debugBuild = $Configuration -eq 'Debug'
$showcase = "$env:MAILCAL_SHOWCASE".Trim().ToLowerInvariant()
$devAccount = "$env:MAILCAL_DEV_ACCOUNT".Trim().ToLowerInvariant()
$showcaseSet = $showcase -and $showcase -notin @('0', 'false', 'no', 'off')
$devAccountSet = $devAccount -and $devAccount -ne 'personal'
$showcaseOn = $debugBuild -and $showcaseSet
$devAccountOn = $debugBuild -and $devAccountSet
$mailbox = 'your stored accounts'
if ($showcaseSet -and -not $debugBuild) {
  Write-Host "==> MAILCAL_SHOWCASE=$showcase is set but this is a $Configuration build, showcase mode is compiled out, so this opens YOUR REAL accounts" -ForegroundColor Yellow
}
if ($devAccountSet -and -not $debugBuild) {
  Write-Host "==> MAILCAL_DEV_ACCOUNT=$devAccount is set but this is a $Configuration build, the dev-account switch (and the harness CA trust) are compiled out, so this opens YOUR REAL accounts" -ForegroundColor Yellow
}
if ($showcaseOn) {
  $mailbox = "the in-memory showcase dataset (MAILCAL_SHOWCASE=$showcase)"
  Write-Host "==> MAILCAL_SHOWCASE=$showcase is set, this launch shows the showcase dataset, NOT your accounts" -ForegroundColor Magenta
  Write-Host "    For your real mailboxes:  Remove-Item env:MAILCAL_SHOWCASE" -ForegroundColor Magenta
}
elseif ($devAccountOn) {
  switch ($devAccount) {
    'stalwart' {
      $mailbox = 'the local Stalwart harness over JMAP (MAILCAL_DEV_ACCOUNT=stalwart)'
      Write-Host "==> MAILCAL_DEV_ACCOUNT=stalwart, this launch shows the harness mailbox, NOT your accounts" -ForegroundColor Magenta
    }
    'stalwart-multi' {
      # The same harness over JMAP, connected as TWO accounts (alice + bob). It exists for
      # contacts: the engine merges people across accounts on a shared address, which a
      # single-account boot cannot show.
      $mailbox = 'the local Stalwart harness over JMAP as two accounts (MAILCAL_DEV_ACCOUNT=stalwart-multi)'
      Write-Host "==> MAILCAL_DEV_ACCOUNT=stalwart-multi, this launch shows the harness mailbox as TWO accounts (alice + bob), NOT your accounts" -ForegroundColor Magenta
    }
    'stalwart-imap' {
      # The harness's IMAP listener serves a self-signed cert, which the debug core trusts only as
      # an extra root read from the PEM named by MAILCAL_EXTRA_CA. When that file is absent the core
      # adds no anchor and says nothing (dev_tls returns an empty vector by design, a dev
      # convenience must never break the normal trust path), so the only symptom is an opaque TLS
      # failure on the setup form. Refuse here instead, where the fix is one command away.
      if (-not $env:MAILCAL_EXTRA_CA) {
        throw "MAILCAL_DEV_ACCOUNT=stalwart-imap needs MAILCAL_EXTRA_CA (the harness's self-signed IMAP cert), which is unset. Boot through scripts/dev/boot.sh windows --account stalwart-imap, which extracts it and sets this for you."
      }
      if (-not (Test-Path -LiteralPath $env:MAILCAL_EXTRA_CA -PathType Leaf)) {
        throw "MAILCAL_EXTRA_CA points at '$env:MAILCAL_EXTRA_CA', which is not a readable file. The cert is regenerated on every harness up/reset, run scripts/dev/harness.sh up, then boot via scripts/dev/boot.sh windows --account stalwart-imap."
      }
      $mailbox = 'the local Stalwart harness over IMAP (MAILCAL_DEV_ACCOUNT=stalwart-imap)'
      Write-Host "==> MAILCAL_DEV_ACCOUNT=stalwart-imap, this launch shows the harness mailbox over IMAP (IDLE push + full mail actions), NOT your accounts" -ForegroundColor Magenta
      Write-Host "    Trusting the harness IMAP cert from $env:MAILCAL_EXTRA_CA (debug builds only)" -ForegroundColor Magenta
    }
    default {
      Write-Host "==> MAILCAL_DEV_ACCOUNT=$devAccount is not supported on Windows, falling back to YOUR REAL accounts" -ForegroundColor Yellow
      Write-Host "    Use 'stalwart' (JMAP), 'stalwart-multi' (two JMAP accounts) or 'stalwart-imap' (IMAP) here." -ForegroundColor Yellow
    }
  }
  Write-Host "    For your real mailboxes:  Remove-Item env:MAILCAL_DEV_ACCOUNT" -ForegroundColor Magenta
}

# A previously-launched instance keeps Mailcal.exe / mailcal_bindings.dll open, so the build's
# copy-to-output step fails after 10 retries (MSB3021, "the file is locked by Mailcal"). Stop
# any running instance first, so the dev loop is re-runnable while the app is still open, the
# Windows twin of the terminate-before-launch the Apple/Android flows do. (The app is single-
# instanced, so a stale process would also swallow a fresh MAILCAL_* launch hook, see control.ps1.)
#
# The MCP relay is the same trap wearing a different hat, and a worse one: allodia-mcp.exe is
# spawned by the *assistant*, not by us, and it lives as long as that client's session, so a
# developer with Claude Desktop connected has one running right now and did not start it. Without
# this the build fails at the copy step (MSB3021) naming a process they have never heard of.
$running = @(Get-Process Mailcal, allodia-mcp -ErrorAction SilentlyContinue)
if ($running) {
  $names = ($running | Group-Object ProcessName | ForEach-Object { "$($_.Count) $($_.Name)" }) -join ', '
  Write-Host "==> Stopping $names process(es) to free the build output" -ForegroundColor Yellow
  $running | Stop-Process -Force
  Start-Sleep -Milliseconds 500  # let the OS release the file handles before the build copies over them
}

# Default the arch to the host's, so a bare run builds something that runs natively here.
$hostArch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
  'Arm64' { 'arm64' } 'X64' { 'x64' } default { 'x64' }
}
if (-not $Arch) { $Arch = $hostArch }

$triple = @{ 'arm64' = 'aarch64-pc-windows-msvc'; 'x64' = 'x86_64-pc-windows-msvc' }[$Arch]
$rid = @{ 'arm64' = 'win-arm64'; 'x64' = 'win-x64' }[$Arch]
$platform = @{ 'arm64' = 'ARM64'; 'x64' = 'x64' }[$Arch]
$profileDir = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$nativeLib = Join-Path $root "target/$triple/$profileDir/mailcal_bindings.dll"

Write-Host "==> Target: $Arch ($triple -> $rid), $Configuration" -ForegroundColor Cyan

# 1. Build the Rust cdylib for the chosen arch (always under target/<triple>/ for a
#    uniform path whether native or cross-compiled). Add the target on demand.
Write-Host "==> Building the Rust cdylib ($triple)" -ForegroundColor Cyan
rustup target add $triple | Out-Null
if ($LASTEXITCODE -ne 0) { throw "rustup target add $triple failed" }
#    The MCP stdio relay (docs/mcp.md) rides along in the same invocation: it is a separate
#    executable an MCP client spawns, and the csproj lays it down beside Mailcal.exe. One cargo
#    call rather than two, so the two artifacts can never be built from different sources.
$cargoArgs = @('build', '-p', 'mailcal-bindings', '-p', 'mailcal-mcp-shim', '--target', $triple)
if ($Configuration -eq 'Release') { $cargoArgs += '--release' }
#    The Allodia sign-in, when this build was given the registration that turns it on -- derived
#    from that registration rather than asked for separately, so the two halves cannot disagree
#    (core-features.ps1, BUILDING.md). Nothing is added in a build from source.
. (Join-Path $here 'core-features.ps1')
$cargoArgs += Get-CoreCargoFeatures -Root $root
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
Assert-StaticCrt $nativeLib
$mcpRelay = Join-Path $root "target/$triple/$profileDir/allodia-mcp.exe"
if (-not (Test-Path -LiteralPath $mcpRelay)) {
  throw "the MCP relay was not produced at $mcpRelay, cargo reported success, so check the mailcal-mcp-shim bin name."
}

# 2. Generate the C# bindings from the built cdylib (arch-independent: the UniFFI
#    metadata is identical across targets, so either arch's DLL produces the same C#).
# Generate via the vendored binshim crate (cargo run), so the generator's version is pinned in
# Cargo.lock, no separate `cargo install` / PATH probe. It's excluded from default-members, so
# this -p invocation is the only thing that builds it.
Write-Host "==> Generating C# bindings" -ForegroundColor Cyan
& cargo run -p mailcal-bindgen-cs -- --library $nativeLib --out-dir (Join-Path $here 'Generated')
if ($LASTEXITCODE -ne 0) { throw "binding generation failed" }

# 3. The headless gate (the deterministic check): drive the demo loop and assert the
#    seeded rows cross the FFI into C#. Run it only when building for the host arch, so it
#    executes natively (a cross-arch console can't run here).
if ($Arch -eq $hostArch) {
  Write-Host "==> Running the headless binding verifier (the gate)" -ForegroundColor Cyan
  & dotnet run --project (Join-Path $here 'MailcalVerify') -c $Configuration "-p:NativeLib=$nativeLib"
  if ($LASTEXITCODE -ne 0) { throw "the binding verifier failed (exit $LASTEXITCODE)" }
}
else {
  Write-Host "==> Skipping the native gate (cross-arch build: $Arch on $hostArch)" -ForegroundColor Yellow
}

# 4. Generate the localised WinUI resources (.resw) + the typed L10n.cs accessor from the
#    shared inlang catalog, so the app builds against current strings (the l10n twin of the
#    binding generation above). Arch-independent codegen, so it runs once regardless of RID.
Write-Host "==> Generating localized resources (mailcal-l10n)" -ForegroundColor Cyan
& cargo run --manifest-path "$root/Cargo.toml" --quiet -p mailcal-l10n -- generate --target winui --root "$root" --out "$here/Mailcal"
if ($LASTEXITCODE -ne 0) { throw "l10n resource generation failed" }

# 5. The calendar grid's unit tests, the ONLY thing that gates docs/calendar.md's hard rules.
#    A plain net10.0 assembly: no WinUI, no Windows TFM, no emulator, no test host, and it runs in
#    seconds. It is arch-independent (it links the pure Calendar/*.cs sources and never loads the
#    cdylib), so it runs on a cross-arch build too.
#
#    CalendarFlickTests is the one that matters, and it is the one no other kind of test can write:
#    it delivers a flick while the PREVIOUS turn is still mid-slide, which is the only condition that
#    reproduces the swallowed swipe. Synthetic touch (touch.ps1) cannot do that, it politely waits
#    for the grid to settle, which is testing the case that already worked (docs/calendar.md §9).
Write-Host "==> Running the calendar unit tests" -ForegroundColor Cyan
& dotnet test (Join-Path $here 'Mailcal.Tests') -c $Configuration --nologo
if ($LASTEXITCODE -ne 0) { throw "calendar unit tests failed" }

# 5b. The screenshot frame geometry, the crop that keeps the invisible window border out of a store
#     screenshot. It runs here, rather than in Mailcal.Tests, because the code under test is the
#     PowerShell capture path; it needs no app, no window and no display, so CI gates it for free.
#     Without it the only check on a store asset was a human looking at the finished PNG, which is
#     how a black-framed set reached showcase-screenshots/windows/.
Write-Host "==> Running the screenshot frame tests" -ForegroundColor Cyan
& (Join-Path $here 'screenshot-frame.tests.ps1')
if ($LASTEXITCODE -ne 0) { throw "screenshot frame tests failed" }

# The shared composer editor is Content-included from clients/composer/dist by Mailcal.csproj, and
# that bundle is a committed build output rather than one generated per build, so rebuild it from
# its TypeScript sources before MSBuild copies it. Without bun it says so and carries on: the
# committed artifact is what ships, and a silent skip is how a stale editor gets verified as new.
# (The bash twin is scripts/dev/composer-bundle.sh; this host has no bash.)
$composer = Join-Path $root 'clients/composer'
if (Get-Command bun -ErrorAction SilentlyContinue) {
  Push-Location $composer
  try {
    # `--check` exits non-zero when the bundle is stale, which is an ANSWER, not a failure, caught
    # rather than tested on $LASTEXITCODE alone, because PowerShell 7.4 turns a native command's
    # non-zero exit into a terminating error under $ErrorActionPreference = 'Stop'.
    $fresh = $false
    try {
      & bun run build.ts --check *> $null
      $fresh = ($LASTEXITCODE -eq 0)
    } catch { $fresh = $false }
    if ($fresh) {
      Write-Host "==> Composer editor: dist/editor.html is up to date" -ForegroundColor Cyan
    } else {
      & bun run build.ts *> $null
      if ($LASTEXITCODE -ne 0) { throw "composer editor bundle failed to build" }
      Write-Host "==> Composer editor: REBUILT dist/editor.html from src, commit it" -ForegroundColor Yellow
    }
  } finally { Pop-Location }
} else {
  Write-Host "==> Composer editor: bun is not installed, using the committed dist/editor.html AS IS." -ForegroundColor Yellow
}

# 6. Build the WinUI 3 app for the paired RID, bundling the arch-matched native DLL.
Write-Host "==> Building the WinUI app ($rid)" -ForegroundColor Cyan
& dotnet build (Join-Path $here 'Mailcal') -c $Configuration -r $rid `
  "-p:Platform=$platform" "-p:NativeLib=$nativeLib" "-p:McpRelay=$mcpRelay"
if ($LASTEXITCODE -ne 0) { throw "dotnet build failed" }

# Locate the built exe robustly (the Platform + RID nest into the output path).
$exe = Get-ChildItem -Path (Join-Path $here 'Mailcal/bin') -Recurse -Filter 'Mailcal.exe' |
Where-Object { $_.FullName -match [regex]::Escape($rid) } |
Sort-Object LastWriteTime -Descending | Select-Object -First 1

Write-Host "==> WinUI app built: $($exe.FullName)" -ForegroundColor Green

$logPath = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'
Write-Host "==> Logs: $logPath (rotates .1-.3, ~4 MB cap)" -ForegroundColor Green

# 6. Launch it (host arch only). It opens the first-run account-setup form, no stored
#    account yet, so nothing private renders; add your account in-app to see real mail.
#    Name the mailbox on the launch line: the banner above scrolls away behind a minutes-long
#    build, and this is the last thing left on screen when the window appears.
if (-not $NoRun -and $Arch -eq $hostArch) {
  Write-Host "==> Launching against $mailbox" -ForegroundColor Cyan
  Start-Process $exe.FullName
}
else {
  Write-Host "    Run it yourself: & '$($exe.FullName)'" -ForegroundColor Green
}
Write-Host "==> Done." -ForegroundColor Green
