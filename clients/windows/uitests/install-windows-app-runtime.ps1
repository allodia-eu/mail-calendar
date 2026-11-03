#!/usr/bin/env pwsh
# Registers the Windows App Runtime the client is built against, for a host that has none.
#
#   ./install-windows-app-runtime.ps1              # x64
#   ./install-windows-app-runtime.ps1 -Arch arm64
#
# The app is FRAMEWORK-DEPENDENT in both build shapes (Mailcal.csproj), so without the runtime
# registered at the resolved version or later it does not start at all. A CI runner always needs
# this, and the UI suite there otherwise reports thirty suites failing to find a window; a developer
# machine needs it after the pin moves past the runtime Visual Studio installed.
#
# The packages come from the RESTORED NUGET PACKAGE rather than a download, which is the whole
# point of doing it this way: `Microsoft.WindowsAppSDK.Runtime` carries the four MSIX packages the
# redistributable installer would fetch, at exactly the version this build resolved, so the runtime
# registered here cannot be a different version from the one compiled against. An aka.ms link
# would drift the first time Microsoft moved "latest". Which version that is, and what counts as
# already having it, is windows-app-runtime.ps1's to say.
[CmdletBinding()]
param([ValidateSet('x64', 'arm64')] [string] $Arch = 'x64')
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'this only means anything on Windows.' }

$client = Join-Path $PSScriptRoot '..'
. (Join-Path $client 'windows-app-runtime.ps1')
$runtime = Get-WindowsAppRuntime -ProjectDir (Join-Path $client 'Mailcal')
$framework = Get-WindowsAppRuntimeFramework -Runtime $runtime -Arch $Arch

# Already served? Then leave it alone: Add-AppxPackage on a package that is present fails with
# 0x80073D06, and a script that cannot be run twice is a script CI runs exactly once and nobody
# runs by hand.
$present = Get-RegisteredWindowsAppRuntime -Framework $framework
if ($present) {
  $present | ForEach-Object { Write-Host "==> $($_.Name) $($_.Version) ($Arch) is already registered, which serves $($framework.Version)" }
  exit 0
}

# The framework first: the other three declare a dependency on it, so a run that starts anywhere
# else fails on the first package rather than the last.
$msix = Split-Path -Parent $framework.Path
$order = @($framework.Path) + @('Main', 'Singleton', 'DDLM' | ForEach-Object {
    $kind = $_
    $path = Get-ChildItem -LiteralPath $msix -Filter "Microsoft.WindowsAppRuntime.$kind.*.msix" |
      Select-Object -First 1 -ExpandProperty FullName
    if (-not $path) { throw "the $kind package is missing from $msix" }
    $path
  })
foreach ($path in $order) {
  Write-Host "==> registering $(Split-Path -Leaf $path)"
  Add-AppxPackage -Path $path
}

# Verified by asking Windows, not by the absence of an error: Add-AppxPackage has more than one way
# to report a partial success, and an unregistered framework is exactly the state whose symptom is
# an app that will not start.
$registered = Get-RegisteredWindowsAppRuntime -Framework $framework
if (-not $registered) {
  throw "the packages installed but $($framework.Name) $($framework.Version) ($Arch) is not registered, so the app would still not start."
}
$registered | ForEach-Object { Write-Host "==> registered $($_.Name) $($_.Version) $($_.Architecture)" }
