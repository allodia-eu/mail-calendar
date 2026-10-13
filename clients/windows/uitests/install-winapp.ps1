#!/usr/bin/env pwsh
# Puts Microsoft's `winapp` CLI on PATH for a host that has none, at the version the suites are
# written against.
#
#   ./install-winapp.ps1              # x64
#   ./install-winapp.ps1 -Arch arm64
#
# Every press in the UI suites goes through winapp (../winapp.ps1). A developer machine installs it
# once with `winget install Microsoft.WinAppCLI` and needs none of this. A CI runner does, and gets
# the release named below rather than whatever winget serves that day: the archive is checked
# against the SHA-256 GitHub published for it, so a moved or replaced release fails here, by name,
# rather than as a suite pressing the wrong thing.
#
# Moving the pin: take the new release's digests from
#   gh release view v<version> -R microsoft/winappCli --json assets
# and run the suites against it on a developer machine first.
[CmdletBinding()]
param([ValidateSet('x64', 'arm64')] [string] $Arch = 'x64')
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'this only means anything on Windows.' }

$version = '0.7.1'
$digests = @{
  x64   = 'd925d1e32cdc320b6d271f653fd2cd3c05b5d7558deb20d1b548bead77900fcf'
  arm64 = '4ed1edcc6d4a1a7c75bb70dbdbf77a69b3c650823c7e8061aa66f8adb1017f2e'
}

$root = Join-Path ([IO.Path]::GetTempPath()) "winapp-$version-$Arch"
$archive = "$root.zip"
Invoke-WebRequest -Uri "https://github.com/microsoft/winappCli/releases/download/v$version/winappcli-$Arch.zip" -OutFile $archive
$actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $digests[$Arch]) {
  throw "winappcli-$Arch.zip $version hashes to $actual, not the published $($digests[$Arch]); refusing to run it"
}
Expand-Archive -LiteralPath $archive -DestinationPath $root -Force
$exe = Get-ChildItem -LiteralPath $root -Recurse -Filter 'winapp.exe' | Select-Object -First 1
if (-not $exe) { throw "no winapp.exe in winappcli-$Arch.zip $version" }

$env:PATH = "$($exe.DirectoryName);$env:PATH"
if ($env:GITHUB_PATH) { Add-Content -LiteralPath $env:GITHUB_PATH -Value $exe.DirectoryName }
"winapp $(& $exe.FullName --version) at $($exe.FullName)"
