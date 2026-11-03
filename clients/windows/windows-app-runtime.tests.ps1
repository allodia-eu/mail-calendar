#!/usr/bin/env pwsh
# Tests for windows-app-runtime.ps1, the one place that says which Windows App Runtime a build
# resolved and whether this machine has it. Run by build-and-run.ps1 (so the `windows` CI job gates
# them) and standalone:
#
#   ./windows-app-runtime.tests.ps1
#
# Plain assertions rather than Pester, and exit code as the contract, for the reasons
# screenshot-frame.tests.ps1 gives: 0 all passed, 1 something failed.
#
# Over a restore graph and NuGet folder built on disk, and with Get-AppxPackage replaced by a
# function: what is under test is which runtime counts, and the cases that matter are a runtime
# package versioned apart from the pin, and a machine holding a runtime older or newer than the one
# resolved. Neither is a state this host can be put into without unregistering what it runs on.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'

$script:Failed = 0
$script:Passed = 0

function Assert-Equal($expected, $actual, [string] $what) {
  if ("$expected" -eq "$actual") {
    $script:Passed++
  }
  else {
    $script:Failed++
    Write-Host "  FAIL $what, expected <$expected>, got <$actual>" -ForegroundColor Red
  }
}

function Assert-Throws([scriptblock] $block, [string] $containing, [string] $what) {
  try {
    & $block | Out-Null
    $script:Failed++
    Write-Host "  FAIL $what, nothing was thrown" -ForegroundColor Red
  }
  catch {
    Assert-Equal $true ("$_".Contains($containing)) "$what (threw: $_)"
  }
}

. (Join-Path $PSScriptRoot 'windows-app-runtime.ps1')

$Sandbox = Join-Path ([IO.Path]::GetTempPath()) "mailcal-runtime-tests-$PID"
$Project = Join-Path $Sandbox 'Mailcal'
$Packages = Join-Path $Sandbox 'packages'

# A restore graph that resolved $runtime, under the package folder $Packages.
function Set-Restore([string] $runtime) {
  New-Item -ItemType Directory -Force -Path (Join-Path $Project 'obj') | Out-Null
  $graph = @{
    libraries      = @{ 'Microsoft.WindowsAppSDK/9.9.9' = @{}; "Microsoft.WindowsAppSDK.Runtime/$runtime" = @{} }
    packageFolders = @{ "$Packages\" = @{} }
  }
  $graph | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Project 'obj/project.assets.json')
}

# A runtime package's MSIX directory for one arch: the framework, declaring $identity, and the three
# packages beside it, which declare a different identity so that picking one of them shows.
function Set-RuntimePackage([string] $runtime, [string] $arch, [string] $identity) {
  $dir = Join-Path $Packages "microsoft.windowsappsdk.runtime/$runtime/tools/MSIX/win10-$arch"
  New-Item -ItemType Directory -Force -Path $dir | Out-Null
  foreach ($kind in '', 'Main.', 'Singleton.', 'DDLM.') {
    $version = if ($kind) { '0.0.0.1' } else { $identity }
    $name = "Microsoft.WindowsAppRuntime.$($kind)2"
    $file = Join-Path $dir "$name.msix"
    Remove-Item -LiteralPath $file -ErrorAction SilentlyContinue
    $zip = [System.IO.Compression.ZipFile]::Open($file, 'Create')
    try {
      $writer = [System.IO.StreamWriter]::new($zip.CreateEntry('AppxManifest.xml').Open())
      try { $writer.Write("<Package xmlns=""http://schemas.microsoft.com/appx/manifest/foundation/windows10""><Identity Name=""$name"" Version=""$version"" /></Package>") }
      finally { $writer.Dispose() }
    }
    finally { $zip.Dispose() }
  }
}

# What Get-AppxPackage answers: the function shadows the cmdlet for everything dot-sourced above.
$script:Registered = @()
function Get-AppxPackage([string] $Name) {
  $script:Registered | Where-Object { $_.Name -eq $Name }
}
function Registered([string] $version, [string] $arch) {
  [pscustomobject]@{ Name = 'Microsoft.WindowsAppRuntime.2'; Version = $version; Architecture = $arch }
}

try {
  # 1. No restore: say so, and say how to get one, rather than guess a version.
  Assert-Throws { Get-WindowsAppRuntime -ProjectDir $Project } 'has not been restored' 'a missing restore is named'

  # 2. The version is the graph's, not the pin's: the meta-package above is 9.9.9.
  Set-Restore '2.6.5'
  Set-RuntimePackage '2.6.5' 'arm64' '2.6.5.0'
  Set-RuntimePackage '2.6.5' 'x64' '2.6.5.0'
  $runtime = Get-WindowsAppRuntime -ProjectDir $Project
  Assert-Equal '2.6.5' $runtime.Version 'the runtime version comes from the restore'

  # 3. The framework, never one of the three beside it, and its identity from its own manifest.
  $framework = Get-WindowsAppRuntimeFramework -Runtime $runtime -Arch arm64
  Assert-Equal 'Microsoft.WindowsAppRuntime.2.msix' (Split-Path -Leaf $framework.Path) 'the framework package is the one picked'
  Assert-Equal 'Microsoft.WindowsAppRuntime.2' $framework.Name 'the name is the manifest''s'
  Assert-Equal '2.6.5.0' $framework.Version 'the version is the manifest''s, all four parts'

  # 4. A restore naming a runtime the package folder does not hold.
  Set-Restore '2.6.6'
  Assert-Throws { Get-WindowsAppRuntime -ProjectDir $Project } 'not in any package folder' 'a runtime missing from the folders is named'
  Set-Restore '2.6.5'

  # 5. Registered at exactly that version: it serves.
  $script:Registered = @(Registered '2.6.5.0' 'Arm64')
  Assert-Equal 1 @(Get-RegisteredWindowsAppRuntime -Framework $framework).Count 'the same version serves'

  # 6. Only a NEWER one registered: it serves, because the dependency is a minimum. Asking for the
  #    exact version here is what sent a machine that was fine into a registration that fails.
  $script:Registered = @(Registered '2.7.0.0' 'Arm64')
  Assert-Equal 1 @(Get-RegisteredWindowsAppRuntime -Framework $framework).Count 'a newer version serves'

  # 7. Only OLDER ones registered: nothing serves, so the app would not start. Accepting any version
  #    here is what let a partial registration report success.
  $script:Registered = @((Registered '2.5.1.0' 'Arm64'), (Registered '2.6.4.0' 'Arm64'))
  Assert-Equal 0 @(Get-RegisteredWindowsAppRuntime -Framework $framework).Count 'an older version does not serve'

  # 8. The right version for another architecture does not serve this one.
  $script:Registered = @((Registered '2.6.5.0' 'X64'), (Registered '2.6.5.0' 'X86'))
  Assert-Equal 0 @(Get-RegisteredWindowsAppRuntime -Framework $framework).Count 'another architecture does not serve'
  $x64 = Get-WindowsAppRuntimeFramework -Runtime $runtime -Arch x64
  Assert-Equal 1 @(Get-RegisteredWindowsAppRuntime -Framework $x64).Count 'its own architecture does'

  # 9. Compared as versions, not as text: 2.10 is later than 2.9.
  $script:Registered = @(Registered '2.10.0.0' 'Arm64')
  Set-RuntimePackage '2.6.5' 'arm64' '2.9.0.0'
  $framework = Get-WindowsAppRuntimeFramework -Runtime $runtime -Arch arm64
  Assert-Equal 1 @(Get-RegisteredWindowsAppRuntime -Framework $framework).Count '2.10 serves a build that needs 2.9'
}
finally {
  Remove-Item -Recurse -Force -LiteralPath $Sandbox -ErrorAction SilentlyContinue
}

if ($script:Failed -gt 0) {
  Write-Host "==> windows-app-runtime: $($script:Failed) failed, $($script:Passed) passed" -ForegroundColor Red
  exit 1
}
Write-Host "==> windows-app-runtime: $($script:Passed) passed" -ForegroundColor Green
