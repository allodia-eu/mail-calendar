#!/usr/bin/env pwsh
# Registers the Windows App Runtime the client is built against, for a host that has none.
#
#   ./install-windows-app-runtime.ps1              # x64
#   ./install-windows-app-runtime.ps1 -Arch arm64
#
# The app is FRAMEWORK-DEPENDENT in both build shapes (Mailcal.csproj), so without the runtime
# registered at the pinned version or later it does not start at all. A CI runner always needs
# this, and the UI suite there otherwise reports thirty suites failing to find a window; a developer
# machine needs it after the pin moves past the runtime Visual Studio installed.
#
# The packages come from the RESTORED NUGET PACKAGE rather than a download, which is the whole
# point of doing it this way: `Microsoft.WindowsAppSDK.Runtime` carries the four MSIX packages the
# redistributable installer would fetch, at exactly the version this build resolved, so the runtime
# registered here cannot be a different version from the one compiled against. An aka.ms link
# would drift the first time Microsoft moved "latest".
[CmdletBinding()]
param([ValidateSet('x64', 'arm64')] [string] $Arch = 'x64')
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'this only means anything on Windows.' }

$csproj = Join-Path $PSScriptRoot '..\Mailcal\Mailcal.csproj'
$pinned = ([xml] (Get-Content -Raw -LiteralPath $csproj)).SelectNodes('//PackageReference') |
  Where-Object { $_.Include -eq 'Microsoft.WindowsAppSDK' } |
  Select-Object -First 1 -ExpandProperty Version
if (-not $pinned) { throw "no Microsoft.WindowsAppSDK PackageReference in $csproj" }

# The two package ids move in lockstep (the meta-package depends on its own version of the runtime),
# so one version answers for both. If that ever stops being true this fails naming the directory it
# looked for, which is the failure you want: loud, and pointing at the thing to look up.
$store = if ($env:NUGET_PACKAGES) { $env:NUGET_PACKAGES } else { Join-Path $HOME '.nuget\packages' }
$msix = Join-Path $store "microsoft.windowsappsdk.runtime\$pinned\tools\MSIX\win10-$Arch"
if (-not (Test-Path -LiteralPath $msix)) {
  throw "no runtime packages at $msix. Restore the client first (build-and-run.ps1), and check that Microsoft.WindowsAppSDK.Runtime still ships at the meta-package's own version ($pinned)."
}

# Already registered? Then leave it alone: Add-AppxPackage on a package that is present fails with
# 0x80073D06, and a script that cannot be run twice is a script CI runs exactly once and nobody
# runs by hand.
# Matched on ARCHITECTURE too. A machine can carry the x86 framework and not the one this build
# needs, and "some version of it is there" would then skip the install and leave the app unable to
# start, which is the symptom this script exists to remove.
$want = if ($Arch -eq 'x64') { 'X64' } else { 'Arm64' }
# At this version OR LATER: the app's dependency on the runtime is a minimum, so a newer one
# already serves it and there is nothing to register.
$framework = Get-AppxPackage | Where-Object {
  $_.Name -eq 'Microsoft.WindowsAppRuntime.2' -and [version] $_.Version -ge [version] "$pinned.0" -and "$($_.Architecture)" -eq $want
}
if ($framework) {
  $framework | ForEach-Object { Write-Host "==> Windows App Runtime $($_.Version) ($want) is already registered, which satisfies $pinned" }
  exit 0
}

# The framework first: the other three declare a dependency on it, so a run that starts anywhere
# else fails on the first package rather than the last.
$order = @(
  'Microsoft.WindowsAppRuntime.2.msix'
  'Microsoft.WindowsAppRuntime.Main.2.msix'
  'Microsoft.WindowsAppRuntime.Singleton.2.msix'
  'Microsoft.WindowsAppRuntime.DDLM.2.msix'
)
foreach ($name in $order) {
  $path = Join-Path $msix $name
  if (-not (Test-Path -LiteralPath $path)) { throw "$name is missing from $msix" }
  Write-Host "==> registering $name"
  Add-AppxPackage -Path $path
}

# Verified by asking Windows, not by the absence of an error: Add-AppxPackage has more than one way
# to report a partial success, and an unregistered framework is exactly the state whose symptom is
# an app that will not start.
$registered = Get-AppxPackage | Where-Object {
  $_.Name -eq 'Microsoft.WindowsAppRuntime.2' -and [version] $_.Version -ge [version] "$pinned.0" -and "$($_.Architecture)" -eq $want
}
if (-not $registered) {
  throw "the packages installed but Microsoft.WindowsAppRuntime.2 is not registered, so the app would still not start."
}
$registered | ForEach-Object { Write-Host "==> registered $($_.Name) $($_.Version) $($_.Architecture)" }
