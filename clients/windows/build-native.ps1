#!/usr/bin/env pwsh
# The Windows release's native half: the Rust cdylib and the MCP relay, per architecture, in release,
# with the features this build's registrations ask for, and proven to link the CRT statically.
#
#   ./build-native.ps1                    # both arches, as package.ps1 does
#   ./build-native.ps1 -Arch arm64        # one, for a release job that builds each arch on its own runner
#
# What it leaves is what the package build reads, and nothing else: target/<triple>/release/
# mailcal_bindings.dll and mailcal-mcp.exe (Mailcal.csproj resolves both per $(Platform)). A job that
# builds them elsewhere puts those two files back at those paths and runs `package.ps1
# -PrebuiltNative`, which packages them without compiling anything.
#
# Like package.ps1, a shipped build: it sets MAILCAL_REQUIRE_INJECTED_CONFIG, so a build missing an
# OAuth registration fails by name rather than producing a binary without that sign-in.
[CmdletBinding()]
param(
  [ValidateSet('x64', 'arm64')] [string[]] $Arch = @('x64', 'arm64')
)
$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot
$root = (Resolve-Path "$here/../..").Path
. (Join-Path $here 'rust-crt.ps1')
. (Join-Path $here 'core-features.ps1')

$env:MAILCAL_REQUIRE_INJECTED_CONFIG = '1'

$tripleOf = @{ 'x64' = 'x86_64-pc-windows-msvc'; 'arm64' = 'aarch64-pc-windows-msvc' }
$triples = @($Arch | Select-Object -Unique | ForEach-Object { $tripleOf[$_] })

# From this checkout's root, as build-and-run.ps1 says why: cargo builds the workspace it finds
# there, and rustup and cargo read rust-toolchain.toml and .cargo/config.toml from it too.
Push-Location $root
try {
  foreach ($triple in $triples) {
    rustup target add $triple | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "rustup target add $triple failed" }
  }
  # Several arches in ONE cargo invocation (several --target flags), never parallel cargo
  # processes on one machine: a single job scheduler overlaps one arch's serial tail (linking, leaf
  # crates) with the other's parallel work and compiles host build-deps and proc-macros once,
  # whereas two processes oversubscribe the cores and fight over the target-dir lock. Separate
  # machines are a different matter, which is what -Arch is for.
  #
  # The MCP stdio relay (docs/mcp.md) is built in the same invocation: a separate executable an MCP
  # client spawns, shipped at each package's root, and named by the App Execution Alias in
  # Package.appxmanifest.
  Write-Host "==> Building the Rust cdylib + MCP relay ($($triples -join ', '), release)" -ForegroundColor Cyan
  $targetArgs = $triples | ForEach-Object { '--target', $_ }
  # The Allodia sign-in, when this build was given the registration that turns it on, derived from
  # that registration rather than asked for separately (core-features.ps1, BUILDING.md).
  $featureArgs = Get-CoreCargoFeatures -Root $root
  & cargo build -p mailcal-ffi -p mailcal-mcp-shim --release @targetArgs @featureArgs
  if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
  foreach ($triple in $triples) {
    Assert-StaticCrt (Join-Path $root "target/$triple/release/mailcal_bindings.dll")
    Assert-StaticCrt (Join-Path $root "target/$triple/release/mailcal-mcp.exe")
  }
  Write-Host "==> $($triples.Count) cdylib(s) and relay(s) link the CRT statically (no VCRUNTIME140 import)" -ForegroundColor DarkGray
}
finally {
  Pop-Location
}
