#!/usr/bin/env pwsh
# Windows client, Store packaging path. Produces a dual-arch (x64 + arm64) MSIX
# bundle as an UNSIGNED .msixupload, ready to upload to Partner Center where Microsoft signs
# it on ingestion (no code-signing cert needed). The dev loop lives in build-and-run.ps1;
# this is its release/Store twin: same Rust cdylib -> C# bindings front half, then MSBuild's
# MSIX packaging targets instead of a bare `dotnet build` + launch.
#
#   ./package.ps1                      # Release .msixupload (unsigned), x64 + arm64, for the Store
#   ./package.ps1 -Version 1.2.0.0     # stamp the package version (pins either path)
#   ./package.ps1 -Sign                # self-signed, installable sideload set, on-device testing
#
# The finished Store artifact is copied to clients/windows/build/release-<VERSION>/ before this
# exits. Every run wipes Mailcal/AppPackages first, so without that the next build destroys the
# bundle the last one produced, including one already submitted, which is the copy you want when
# the Store asks for a re-upload or you need to prove what shipped.
#
# The Store upload is intentionally UNSIGNED (Microsoft signs on ingestion), so it can't be
# installed locally. -Sign instead self-signs with a throwaway dev cert and builds a SideloadOnly
# package you can install on a test device. Local testing only, never distribute a self-signed build.
#
# -Sign auto-stamps a monotonically increasing version (unless -Version pins one), so re-running
# it and reinstalling is an in-place UPGRADE, no "remove the old package first" (0x80073CFB) dance.
#
# Prereqs: Visual Studio 2026 (MSBuild + the MSIX packaging targets), .NET 10 SDK, Rust MSVC,
# uniffi-bindgen-cs (see README). Identity placeholders in Package.appxmanifest must be
# replaced with the Allodia Partner Center values before the upload is accepted.
[CmdletBinding()]
param(
  [string] $Version,
  [ValidateSet('StoreUpload', 'SideloadOnly', 'CI')] [string] $PackageMode = 'StoreUpload',
  [switch] $Sign
)
$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot
$root = (Resolve-Path "$here/../..").Path                 # the repository root
$proj = Join-Path $here 'Mailcal/Mailcal.csproj'

# Every OAuth client registration this build uses must be present, or the compile fails naming the
# missing ones (crates/mailcal-oauth/build.rs). A build without them is legitimate -- it simply
# offers no Google or Microsoft sign-in -- which is exactly why a *shipped* one has to be refused
# rather than produced: it would look correct everywhere except in front of a user.
$env:MAILCAL_REQUIRE_INJECTED_CONFIG = '1'

# The marketing version, single source of truth in the top-level /VERSION file (docs/versioning.md);
# the csproj derives the assembly <Version> from the same file. Default the Store package version to
# that marketing version with a .0 revision (the Store reserves the revision field for its own
# repackaging). -Version still overrides for a one-off; -Sign auto-versions the sideload path below.
$semver = (Get-Content (Join-Path $root 'VERSION') -Raw).Trim()
if ($semver -notmatch '^\d+\.\d+\.\d+$') {
  throw "/VERSION must be MAJOR.MINOR.PATCH (got '$semver')."
}
if (-not $Version -and -not $Sign) { $Version = "$semver.0" }

# Assert the cdylib links the CRT statically (the flags themselves live in the workspace
# .cargo/config.toml). A dynamically-linked CRT crashes the app at launch on any machine without the
# Visual C++ Redistributable, including Microsoft's Store certification hosts, which is how 1.0.0.0
# failed cert. See rust-crt.ps1.
. (Join-Path $here 'rust-crt.ps1')

# The Store rejects any package whose Version has a NON-ZERO revision (the fourth field), it
# reserves that field for its own repackaging. Partner Center only says so *after* the upload
# finishes, so a 137 MB round-trip is the feedback loop. Refuse locally instead. Sideload builds are
# exempt: they never reach the Store, and -Sign deliberately auto-versions the revision so a rebuild
# upgrades in place (see below).
if ($Version) {
  if ($Version -notmatch '^\d+\.\d+\.\d+\.\d+$') {
    throw "-Version must be four dot-separated numbers (e.g. 1.0.1.0); got '$Version'."
  }
  if ($PackageMode -eq 'StoreUpload' -and -not $Sign -and [int]($Version -split '\.')[3] -ne 0) {
    # Suggest the next BUILD number, not the same version with the revision zeroed, that one is
    # already ingested, and the Store rejects a re-upload of a version it has seen.
    $f = $Version -split '\.'
    $suggested = "$($f[0]).$($f[1]).$([int]$f[2] + 1).0"
    throw @"
-Version '$Version' has a non-zero revision. The Store rejects that on ingestion:
"Apps are not allowed to have a Version with a revision number other than zero specified in
the app manifest." Bump the build field instead, e.g. $suggested
"@
  }
}

# 0. Clean prior build outputs. The MSIX packaging targets are aggressively incremental and have
#    repeatedly shipped STALE content from these dirs: a stale Upload/AppxManifest.xml (a changed
#    PublisherDisplayName / identity that never re-emitted into the .msixupload, so the Store
#    rejected it) and stale split-language PRIs. This is the rarely-run release path, so a
#    from-scratch pack is the right default; the Rust target/ at the repo root is untouched, so only
#    the C# side rebuilds (~1-2 min).
$projDir = Join-Path $here 'Mailcal'
foreach ($sub in 'bin', 'obj', 'AppPackages', 'BundleArtifacts') {
  $p = Join-Path $projDir $sub
  if (Test-Path $p) { Remove-Item -Recurse -Force $p }
}
Write-Host "==> Cleaned prior build outputs (bin, obj, AppPackages, BundleArtifacts)" -ForegroundColor DarkGray

# 1. Build the Rust cdylib for BOTH arches in release, the bundle ships both, and the csproj
#    resolves each arch's DLL per $(Platform) (target/<triple>/release/mailcal_bindings.dll).
#    One cargo invocation with two --target flags (stable since 1.64), NOT two parallel cargo
#    processes: a single job scheduler overlaps one arch's serial tail (linking, leaf crates)
#    with the other's parallel work and compiles host build-deps/proc-macros once, whereas two
#    processes oversubscribe the cores, duplicate host work, and fight over the target-dir lock.
$triples = @('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')
foreach ($triple in $triples) {
  rustup target add $triple | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "rustup target add $triple failed" }
}
#    The MCP stdio relay (docs/mcp.md) is built in the same invocation, per arch: it is a separate
#    executable an MCP client spawns, it ships at each package's root, and the App Execution Alias
#    in Package.appxmanifest points at it by name.
Write-Host "==> Building the Rust cdylib + MCP relay for both arches ($($triples -join ', '), release)" -ForegroundColor Cyan
$targetArgs = $triples | ForEach-Object { '--target', $_ }
#    The Allodia sign-in, when this build was given the registration that turns it on -- derived
#    from that registration rather than asked for separately, so the two halves cannot disagree
#    (core-features.ps1, BUILDING.md). Nothing is added in a build from source.
. (Join-Path $here 'core-features.ps1')
$featureArgs = Get-CoreCargoFeatures -Root $root
& cargo build -p mailcal-bindings -p mailcal-mcp-shim --release @targetArgs @featureArgs
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
foreach ($triple in $triples) {
  Assert-StaticCrt (Join-Path $root "target/$triple/release/mailcal_bindings.dll")
  Assert-StaticCrt (Join-Path $root "target/$triple/release/allodia-mcp.exe")
}
Write-Host "==> Both cdylibs and both relays link the CRT statically (no VCRUNTIME140 import)" -ForegroundColor DarkGray

# 2. Generate the C# bindings (arch-independent, either arch's cdylib yields the same C#) via
#    the vendored binshim crate (cargo run), so the generator is Cargo.lock-pinned, not a
#    separately `cargo install`ed binary.
Write-Host "==> Generating C# bindings" -ForegroundColor Cyan
$anyLib = Join-Path $root 'target/x86_64-pc-windows-msvc/release/mailcal_bindings.dll'
& cargo run -p mailcal-bindgen-cs -- --library $anyLib --out-dir (Join-Path $here 'Generated')
if ($LASTEXITCODE -ne 0) { throw "binding generation failed" }

# 2b. Generate the localised WinUI resources (.resw) + the typed L10n.cs from the shared inlang
#     catalog (messages/*.json), the l10n twin of build-and-run.ps1's step. WITHOUT this the Store
#     pack ships whatever Strings/ a prior dev run happened to leave on disk; on a clean checkout or
#     CI box (no Strings/ folder) that silently means NO localised resources, the package advertises
#     only the default language. mailcal-l10n is workspace-excluded, so this `cargo run` is the only
#     thing that produces them here.
Write-Host "==> Generating localized resources (mailcal-l10n)" -ForegroundColor Cyan
& cargo run --manifest-path "$root/Cargo.toml" --quiet -p mailcal-l10n -- generate --target winui --root "$root" --out "$here/Mailcal"
if ($LASTEXITCODE -ne 0) { throw "l10n resource generation failed" }

# 3. Ensure the tile/store assets exist. They're committed, but regenerate them from the brand
#    source icon if a checkout is missing them.
if (-not (Test-Path (Join-Path $here 'Mailcal/Images/StoreLogo.png'))) {
  Write-Host "==> Generating tile/store assets from source brand icon" -ForegroundColor Yellow
  & (Join-Path $here 'Mailcal/Images/generate-assets.ps1')
}

# 4. Locate MSBuild, the MSIX bundle / .msixupload targets aren't in `dotnet build`, so this
#    step needs full MSBuild from the VS install (found via vswhere, no Dev prompt required).
#    Prefer the NATIVE-arch MSBuild: the default Bin\MSBuild.exe is 32-bit x86, which runs under
#    emulation on an arm64 host and is painfully slow. The native build lives in an arch subdir
#    (Bin\arm64\ on arm64, Bin\amd64\ on x64); fall back to the 32-bit default only if absent.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path $vswhere)) {
  throw "vswhere not found, install Visual Studio 2026 (or its Build Tools)."
}
$installPath = & $vswhere -latest -prerelease -products * -requires Microsoft.Component.MSBuild `
  -property installationPath | Select-Object -First 1
if (-not $installPath) { throw "No Visual Studio with MSBuild found via vswhere." }
$msbuildBin = Join-Path $installPath 'MSBuild\Current\Bin'
$msbuildArch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
  'Arm64' { 'arm64' } 'X64' { 'amd64' } default { 'amd64' }
}
$msbuild = Join-Path $msbuildBin "$msbuildArch\MSBuild.exe"
if (-not (Test-Path $msbuild)) { $msbuild = Join-Path $msbuildBin 'MSBuild.exe' }   # 32-bit fallback
if (-not (Test-Path $msbuild)) { throw "MSBuild not found under $msbuildBin." }
Write-Host "==> Using MSBuild ($msbuildArch): $msbuild" -ForegroundColor DarkGray

# 4a2. Put this build's identity into the manifest, and take it back out afterwards.
#      Package.appxmanifest is committed UNBRANDED (docs/branding.md), the public default, while
#      the Store matches the package name, the publisher GUID and the publisher display name
#      against the Partner Center reservation and rejects anything else on ingestion. So the
#      manifest is rewritten here and the committed bytes are restored by the `finally` around the
#      build, exactly as the version stamp below already does.
#
#      Before 4b deliberately: -Sign reads Identity/@Publisher back out of this file to choose the
#      self-signed certificate, and a rebrand after that point would sign with one publisher and
#      package under another.
$manifest = Join-Path $here 'Mailcal/Package.appxmanifest'
$manifestBackup = [System.IO.File]::ReadAllBytes($manifest)   # exact bytes, for a faithful restore
#      The rewrite is a Python script rather than PowerShell so it can be tested on any host
#      (scripts/dev/tests/test_msix_manifest.py), nothing about a wrong MSIX identity fails
#      locally; it fails at the Store, days later, having burned a submission. Windows installs
#      Python under either name, so both are tried.
$python = @('python3', 'python') |
  ForEach-Object { Get-Command $_ -ErrorAction SilentlyContinue } | Select-Object -First 1
if (-not $python) { throw "Python 3 is required (it puts the brand into Package.appxmanifest)." }
try {
  & $python.Source (Join-Path $root 'scripts/dev/msix_manifest.py') --manifest $manifest
  if ($LASTEXITCODE -ne 0) { throw "putting the brand into Package.appxmanifest failed" }
}
catch {
  [System.IO.File]::WriteAllBytes($manifest, $manifestBackup)
  throw
}
$brandedName = ([xml](Get-Content $manifest)).Package.Properties.DisplayName
Write-Host "==> Packaging as '$brandedName' ($((([xml](Get-Content $manifest)).Package.Identity.Name)))" -ForegroundColor Cyan

# 4b. Signing. The Store upload is unsigned (Microsoft signs on ingestion) and can't be installed
#     locally. -Sign self-signs with a throwaway dev cert whose subject matches the manifest
#     Publisher, and switches to SideloadOnly so the _Test set carries a .cer + Add-AppDevPackage.ps1.
#     Reuses a matching cert if present, else mints one in CurrentUser\My. Local testing ONLY.
$signArgs = @('/p:AppxPackageSigningEnabled=false')
if ($Sign) {
  $PackageMode = 'SideloadOnly'
  $publisher = ([xml](Get-Content (Join-Path $here 'Mailcal/Package.appxmanifest'))).Package.Identity.Publisher
  $cert = Get-ChildItem Cert:\CurrentUser\My |
  Where-Object { $_.Subject -eq $publisher -and $_.HasPrivateKey } | Select-Object -First 1
  if (-not $cert) {
    Write-Host "==> Minting self-signed dev cert ($publisher)" -ForegroundColor Yellow
    $cert = New-SelfSignedCertificate -Type Custom -Subject $publisher -KeyUsage DigitalSignature `
      -CertStoreLocation Cert:\CurrentUser\My `
      -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
  }
  Write-Host "==> Signing with dev cert ($($cert.Thumbprint))" -ForegroundColor Cyan
  $signArgs = @('/p:AppxPackageSigningEnabled=true', "/p:PackageCertificateThumbprint=$($cert.Thumbprint)")
}

# 4c. Auto-version the sideload build (unless -Version pins one). MSIX refuses to reinstall the
#     same version with different content (0x80073CFB), so a fixed manifest version forces a
#     manual remove-or-bump on every dev rebuild. A monotonically increasing Build.Revision makes
#     each rebuild an in-place UPGRADE instead. MAJOR.MINOR come from /VERSION ($semver) so the
#     sideload package version tracks the single source of truth (docs/versioning.md), not a stale
#     literal; Build.Revision then carry a monotonic timestamp, which consumes both remaining fields
#     so the semver PATCH is not represented in the *package* version. That's fine: the assembly
#     <Version> below still carries the full /VERSION semver, which is what DeviceFacts.cs reports.
#     Scheme (each field must be <= 65535): Build = days since 2000-01-01 (< 65535 until ~2179),
#     Revision = seconds-since-midnight / 2 (< 43200), monotonic within a day and across days. The
#     Store path keeps the deliberate Package.appxmanifest version; pass -Version to pin either path.
if ($Sign -and -not $Version) {
  $now = Get-Date
  $build = ($now.Date - [datetime]'2000-01-01').Days
  $rev = [int]($now.TimeOfDay.TotalSeconds / 2)
  $mm = $semver -split '\.'   # /VERSION MAJOR.MINOR.PATCH, validated above, take MAJOR.MINOR, not a hardcoded 1.0
  $Version = "$($mm[0]).$($mm[1]).$build.$rev"
  Write-Host "==> Auto-versioning sideload build: $Version (rebuild => in-place upgrade)" -ForegroundColor DarkGray
}

# 5. Stamp the version into Package.appxmanifest. Single-project MSIX packaging reads the package
#    version straight from the manifest's Identity/@Version, a build property (AppxPackageVersion)
#    is silently ignored here, bundle AND inner packages stay at the manifest's value. So write the
#    version into the manifest; the committed bytes go back after the build (the try/finally below),
#    which also undoes the rebrand from 4a2 and leaves the working tree clean.
if ($Version) {
  $current = [System.IO.File]::ReadAllText($manifest)
  $stamped = ($current -replace '(<Identity\b[^>]*?\bVersion=")[^"]*(")', "`${1}$Version`${2}")
  [System.IO.File]::WriteAllText($manifest, $stamped, (New-Object System.Text.UTF8Encoding($false)))
  Write-Host "==> Stamped package version $Version into Package.appxmanifest (restored after build)" -ForegroundColor DarkGray
}

# 6. Build the dual-arch MSIX bundle. -p:Packaged=true flips the csproj to the MSIX /
#    framework-dependent-WinAppSDK / self-contained-.NET shape; UapAppxPackageBuildMode picks the
#    output (.msixupload for the Store, or a sideload _Test set for -Sign).
$msbuildArgs = @(
  $proj, '/restore', '/v:minimal',
  '/p:Packaged=true',
  '/p:Configuration=Release',
  '/p:Platform=x64',
  '/p:AppxBundle=Always',
  '/p:AppxBundlePlatforms=x64|arm64',
  "/p:UapAppxPackageBuildMode=$PackageMode",
  '/p:GenerateAppxPackageOnBuild=true',
  # Assembly version = the /VERSION marketing string (what DeviceFacts.cs reports). The csproj reads
  # /VERSION for this too; passing it explicitly keeps the two identical regardless. The 4-part MSIX
  # package version is a separate thing, stamped into Package.appxmanifest by step 5 above.
  "/p:Version=$semver"
) + $signArgs
Write-Host "==> Building the MSIX bundle (x64|arm64, $PackageMode)" -ForegroundColor Cyan
try {
  & $msbuild @msbuildArgs
  if ($LASTEXITCODE -ne 0) { throw "MSIX bundle build failed (exit $LASTEXITCODE)" }
}
finally {
  if ($null -ne $manifestBackup) { [System.IO.File]::WriteAllBytes($manifest, $manifestBackup) }
}

# 6b. Gate the SHIPPED container, not just the cdylibs under target/. Those are only the files
#     MSBuild copies *from*: a stale bin/, a mis-resolved $(NativeLib), or a packaging step that
#     picked up a different DLL would all sail past the earlier assert and still upload a cdylib
#     that crashes on a clean machine. So open the artifact and check the bytes actually inside it.
#     Prefer the outermost container (the .msixupload nests the bundle, which nests both arches'
#     .msix), so one call covers everything that ships.
$shipped = $null
foreach ($ext in '*.msixupload', '*.msixbundle', '*.msix') {
  $shipped = Get-ChildItem -Path (Join-Path $here 'Mailcal') -Recurse -Filter $ext -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if ($shipped) { break }
}
if (-not $shipped) { throw "the build produced no .msixupload/.msixbundle/.msix to verify" }
Assert-StaticCrtInPackage $shipped.FullName

# 7. Report the artifact + next step.
$appPackages = Join-Path $here 'Mailcal/AppPackages'
if ($Sign) {
  $testDir = Get-ChildItem -Path $appPackages -Directory -Filter '*_Test' -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
  $bundle = if ($testDir) { Get-ChildItem $testDir.FullName -Filter '*.msixbundle' -ErrorAction SilentlyContinue | Select-Object -First 1 }
  $cer = if ($testDir) { Get-ChildItem $testDir.FullName -Filter '*.cer' -ErrorAction SilentlyContinue | Select-Object -First 1 }
  $devScript = if ($testDir) { Get-ChildItem $testDir.FullName -Filter 'Add-AppDevPackage.ps1' -ErrorAction SilentlyContinue | Select-Object -First 1 }

  if ($devScript) {
    # Single-arch (non-bundle) sideload sets keep the SDK's full helper layout.
    Write-Host "==> Signed sideload set ready: $($devScript.Directory.FullName)" -ForegroundColor Green
    Write-Host "    On the test device (Settings -> For developers -> Developer Mode = On): copy that" -ForegroundColor Green
    Write-Host "    folder over, then run Add-AppDevPackage.ps1 (right-click -> Run with PowerShell)." -ForegroundColor Green
  }
  elseif ($bundle -and $cer) {
    # The dual-arch BUNDLE step (AppxBundle=Always) deletes the _Test layout and repacks only the
    # .msixbundle + .cer into it, so the SDK's Add-AppDevPackage.ps1 + Dependencies don't survive
    # (a microsoft.windows.sdk.buildtools.msix 1.7.x limitation, the helper only persists for a
    # single-arch package). Install the signed bundle directly instead, works the same, and a
    # rebuilt (auto-versioned) bundle upgrades in place:
    Write-Host "==> Signed sideload bundle ready: $($bundle.FullName)" -ForegroundColor Green
    Write-Host "    The dual-arch bundle build doesn't emit Add-AppDevPackage.ps1 (the SDK's bundle step" -ForegroundColor DarkGray
    Write-Host "    wipes the _Test layout). Install the signed bundle directly:" -ForegroundColor DarkGray
    Write-Host "      # 1. Trust the dev cert once (elevated):" -ForegroundColor Green
    Write-Host "      Import-Certificate -FilePath '$($cer.FullName)' -CertStoreLocation Cert:\LocalMachine\TrustedPeople" -ForegroundColor Green
    Write-Host "      # 2. Install, or upgrade in place on a rebuild:" -ForegroundColor Green
    Write-Host "      Add-AppxPackage -Path '$($bundle.FullName)'" -ForegroundColor Green
    Write-Host "    A clean device also needs the WinApp SDK runtime (Microsoft.WindowsAppRuntime): it's" -ForegroundColor DarkGray
    Write-Host "    preinstalled on dev boxes; on a bare device install it from the Store/winget first." -ForegroundColor DarkGray
  }
  else {
    Write-Host "==> Build finished but no installable bundle found, check $appPackages." -ForegroundColor Yellow
  }
}
else {
  $pattern = if ($PackageMode -eq 'StoreUpload') { '*.msixupload' } else { '*.msixbundle' }
  $artifact = Get-ChildItem -Path (Join-Path $here 'Mailcal') -Recurse -Filter $pattern -ErrorAction SilentlyContinue |
  Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if ($artifact) {
    # Copied OUT of Mailcal/AppPackages, which the next run wipes before it builds. A Store bundle
    # outlives its build: it is what a re-upload needs, and what says which bytes were submitted.
    # Named by version so successive releases sit beside each other rather than overwriting.
    $kept = $artifact
    if ($PackageMode -eq 'StoreUpload') {
      $keepDir = Join-Path $here "build/release-$Version"
      New-Item -ItemType Directory -Force $keepDir | Out-Null
      Copy-Item $artifact.FullName -Destination $keepDir -Force
      $kept = Get-Item (Join-Path $keepDir $artifact.Name)
    }
    Write-Host "==> Package ready: $($kept.FullName)" -ForegroundColor Green
    if ($PackageMode -eq 'StoreUpload') {
      Write-Host "    Kept here so the next build cannot wipe it; upload this copy." -ForegroundColor DarkGray
      Write-Host "    Upload it in Partner Center (Allodia); Microsoft signs it on ingestion." -ForegroundColor Green
    }
  }
  else {
    Write-Host "==> Build finished but no $pattern found, check Mailcal/AppPackages." -ForegroundColor Yellow
  }
}
Write-Host "==> Done." -ForegroundColor Green
