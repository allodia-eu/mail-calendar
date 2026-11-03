#!/usr/bin/env pwsh
# The Windows App Runtime a build of the client resolved, and whether this machine has it
# registered. Dot-source it; it defines functions and does nothing on its own:
#
#     . "$PSScriptRoot/windows-app-runtime.ps1"            # from clients/windows/
#     $runtime = Get-WindowsAppRuntime -ProjectDir "$PSScriptRoot/Mailcal"
#     $framework = Get-WindowsAppRuntimeFramework -Runtime $runtime -Arch arm64
#     if (-not (Get-RegisteredWindowsAppRuntime -Framework $framework)) { ... }
#
# THE VERSION COMES FROM THE RESTORE, never from the csproj. The pin is the Microsoft.WindowsAppSDK
# meta-package, and the packages it pulls in are versioned on lines of their own (2.5.1 resolved
# Foundation 2.3.12), so the runtime package's version is whatever the restore says it is. Read
# from the graph, a bump cannot leave a caller staging or registering an older runtime than the one
# the build asks for. So every function here needs a restore first.
#
# THE FRAMEWORK'S IDENTITY COMES FROM ITS OWN MANIFEST, rather than from the NuGet version and a
# hard-coded name: what the app depends on, and what Get-AppxPackage reports, is the name and the
# four-part version inside the .msix, so that is what a comparison has to use.

Add-Type -AssemblyName System.IO.Compression.FileSystem

# The Microsoft.WindowsAppSDK.Runtime package the project at $ProjectDir restored: its version and
# the directory it was extracted to.
function Get-WindowsAppRuntime {
  param([Parameter(Mandatory)] [string] $ProjectDir)
  $assets = Join-Path $ProjectDir 'obj/project.assets.json'
  # `${}` around the variable because a bare `$assets:` is a scope qualifier to the parser, and the
  # file does not compile at all rather than failing here.
  if (-not (Test-Path -LiteralPath $assets)) {
    throw "the client has not been restored, so the Windows App Runtime it needs is unknown: ${assets} is missing. Build it first (build-and-run.ps1 -NoRun)."
  }
  $graph = Get-Content -LiteralPath $assets -Raw | ConvertFrom-Json
  $entry = $graph.libraries.PSObject.Properties.Name |
    Where-Object { $_ -like 'Microsoft.WindowsAppSDK.Runtime/*' } | Select-Object -First 1
  if (-not $entry) { throw "${assets} names no Microsoft.WindowsAppSDK.Runtime package." }
  $version = $entry.Split('/')[1]
  $root = $graph.packageFolders.PSObject.Properties.Name |
    ForEach-Object { Join-Path $_ "microsoft.windowsappsdk.runtime/$version" } |
    Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
  if (-not $root) { throw "Microsoft.WindowsAppSDK.Runtime $version is not in any package folder ${assets} names." }
  [pscustomobject]@{ Version = $version; Root = $root }
}

# The framework package for one architecture inside that runtime: its path, and the name and
# version its manifest declares. Only the framework: the Main, Singleton and DDLM packages beside it
# serve unpackaged apps and push notifications, and depend on it.
function Get-WindowsAppRuntimeFramework {
  param(
    [Parameter(Mandatory)] $Runtime,
    [Parameter(Mandatory)] [ValidateSet('x64', 'arm64')] [string] $Arch
  )
  $dir = Join-Path $Runtime.Root "tools/MSIX/win10-$Arch"
  $file = Get-ChildItem -LiteralPath $dir -Filter 'Microsoft.WindowsAppRuntime.*.msix' -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -notmatch '\.(DDLM|Main|Singleton)\.' } | Select-Object -First 1
  if (-not $file) { throw "no Windows App Runtime framework package for $Arch under $dir." }
  $zip = [System.IO.Compression.ZipFile]::OpenRead($file.FullName)
  try {
    $reader = [System.IO.StreamReader]::new($zip.GetEntry('AppxManifest.xml').Open())
    try { $manifest = [xml] $reader.ReadToEnd() } finally { $reader.Dispose() }
  }
  finally { $zip.Dispose() }
  [pscustomobject]@{
    Path    = $file.FullName
    Arch    = $Arch
    Name    = $manifest.Package.Identity.Name
    Version = [version] $manifest.Package.Identity.Version
  }
}

# What this machine has registered that serves $Framework: the same package name and architecture,
# at its version OR LATER, because the app's dependency on the runtime is a minimum. Matched on the
# architecture too, since a machine can carry the x86 framework and not the one a build needs.
# Empty when nothing does.
function Get-RegisteredWindowsAppRuntime {
  param([Parameter(Mandatory)] $Framework)
  $want = if ($Framework.Arch -eq 'x64') { 'X64' } else { 'Arm64' }
  Get-AppxPackage -Name $Framework.Name | Where-Object {
    "$($_.Architecture)" -eq $want -and [version] $_.Version -ge $Framework.Version
  }
}
