#!/usr/bin/env pwsh
# Verifies that the Rust cdylib links the C runtime statically.
#
# The flag itself is NOT set here. It lives in the workspace `.cargo/config.toml`
# (`[target.<msvc-triple>] rustflags = ["-C", "target-feature=+crt-static"]`), so a plain
# `cargo build` is correct too, not just the builds these scripts drive. This file is the gate that
# proves it held.
#
# Why it matters. Rust's *-pc-windows-msvc targets link the CRT DYNAMICALLY by default, so
# mailcal_bindings.dll imports VCRUNTIME140.dll. That file is NOT part of Windows: it ships in the
# Visual C++ Redistributable, which every dev box has (Visual Studio installs it) and a clean
# machine does not. The MSIX can't rescue it either, nothing in our package graph carries it. We
# declare only Microsoft.WindowsAppRuntime.2, and that framework package declares no dependencies of
# its own, so Microsoft.VCLibs.140.00.UWPDesktop (which does ship vcruntime140.dll) is never pulled
# in. Microsoft links Microsoft.UI.Xaml.dll's own CRT statically for the same reason.
#
# On a clean box, then: LoadLibrary('mailcal_bindings.dll') fails with ERROR_MOD_NOT_FOUND, the
# first P/Invoke (MailboxModel's AvailableZones field initializer) throws DllNotFoundException
# inside App.OnLaunched, and WinUI fail-fasts with 0xc000027b / STATUS_STOWED_EXCEPTION, blaming
# Microsoft.UI.Xaml.dll. That is precisely how the app failed Microsoft Store certification, it
# crashed at launch on the cert machine while running fine on every machine that had ever seen
# Visual Studio. The architecture was never the problem; the missing redistributable was.
#
# Why a gate at all, when the config file already says so: a RUSTFLAGS environment variable
# SUPPRESSES target.*.rustflags outright (Cargo precedence), silently reverting the cdylib to a
# dynamic CRT. And no Rust or C# test can catch that, it is a property of the linked artifact, and
# it only bites on a host without the redistributable. So the check lives where the artifact is
# produced, and CI runs it on both paths: the per-commit Debug build (ci.yml -> build-and-run.ps1)
# and the release .msixupload (windows-release.yml -> package.ps1).

# Fail the build if the cdylib still depends on a dynamically-linked CRT. -Label names the DLL in
# the error when $Dll is a scratch copy (Assert-StaticCrtInPackage extracts to a temp file, and the
# random temp name tells the reader nothing about which package the offending cdylib came from).
function Assert-StaticCrt {
  param(
    [Parameter(Mandatory)] [string] $Dll,
    [string] $Label
  )

  if (-not (Test-Path $Dll)) { throw "Assert-StaticCrt: no binary at '$Dll'" }
  if (-not $Label) { $Label = $Dll }
  # A PE's import directory stores each dependency's name as plain ASCII, so scanning the image
  # catches a dynamically-linked CRT without parsing the headers.
  $ascii = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($Dll))
  $pattern = '(?i)\b(vcruntime\d+(_\d+)?|msvcp\d+|ucrtbase|api-ms-win-crt-[a-z0-9\-]+)\.dll'
  $dynamicCrt = [regex]::Matches($ascii, $pattern) | ForEach-Object { $_.Value } | Sort-Object -Unique
  if (-not $dynamicCrt) { return }

  # Only vcruntime/msvcp come from the redistributable and so actually break a clean machine; the
  # api-ms-win-crt-* forwarders are the UCRT, which is part of Windows. A static build has neither,
  # so both fail the gate, but say which ones are the ones that crash the app.
  $redist = @($dynamicCrt | Where-Object { $_ -match '(?i)^(vcruntime|msvcp)' })
  $verdict = if ($redist) {
    "It needs $($redist -join ', '), supplied by the Visual C++ Redistributable and not by Windows.
A clean machine, as Microsoft's Store certification hosts are, cannot load it, so the app crashes
at launch with 0xc000027b."
  }
  else {
    "It links the UCRT dynamically. That loads on a clean machine today, but the cdylib is meant to
carry no CRT dependency at all, so treat this as the build flag having been dropped."
  }

  throw @"
$Label imports the C runtime dynamically: $($dynamicCrt -join ', ').

$verdict

The workspace .cargo/config.toml sets -C target-feature=+crt-static for both MSVC triples, so this
should not happen. The usual cause is a RUSTFLAGS environment variable, which makes Cargo ignore
per-target rustflags entirely: unset it and rebuild. Otherwise check that .cargo/config.toml is
still present and that cargo is being run from inside this workspace.
"@
}

# The same assert, but against the artifact that actually ships. Asserting the cdylib under target/
# only covers the file MSBuild copies *from*: a stale bin/, a mis-resolved $(NativeLib), or a
# packaging step that picked up a different DLL would all sail past it. This opens the shipped
# container instead and checks the bytes inside it. .msixupload, .msixbundle and .msix are all Zips,
# and the first two nest the next one down, so walk them.
#
# It covers BOTH Rust binaries, and both for the same reason. The MCP relay (allodia-mcp.exe) is
# spawned by ANOTHER application, so a missing CRT there surfaces as "the server failed to start"
# inside the user's assistant, even less diagnosable than the app's own launch crash. And its
# PRESENCE is asserted here too: the csproj's Content copy is conditional, so a build with no relay
# under target/ packages silently and the only artefact that knows is the .msixupload, which nobody
# reads. That is exactly how the macOS build shipped an app with no relay in it (docs/mcp.md); a
# check that cannot fail is not a check, so this one inspects the shipped container.
function Assert-StaticCrtInPackage {
  param([Parameter(Mandatory)] [string] $Package)

  if (-not (Test-Path $Package)) { throw "Assert-StaticCrtInPackage: no package at '$Package'" }
  Add-Type -AssemblyName System.IO.Compression.FileSystem

  $checkedCdylib = 0
  $checkedRelay = 0
  # Each frame is a [name, byte[]] pair, so nested archives can be opened straight from memory.
  $pending = [System.Collections.Generic.Queue[object]]::new()
  $pending.Enqueue(@($Package, [System.IO.File]::ReadAllBytes($Package)))

  while ($pending.Count) {
    $frame = $pending.Dequeue()
    $label, $bytes = $frame[0], $frame[1]
    $zip = [System.IO.Compression.ZipArchive]::new([System.IO.MemoryStream]::new($bytes))
    try {
      foreach ($entry in $zip.Entries) {
        $isNested = $entry.FullName -match '(?i)\.(msix|msixbundle|appx)$'
        $isCdylib = $entry.FullName -match '(?i)(^|/)mailcal_bindings\.dll$'
        $isRelay = $entry.FullName -match '(?i)(^|/)allodia-mcp\.exe$'
        if (-not ($isNested -or $isCdylib -or $isRelay)) { continue }

        $buf = [System.IO.MemoryStream]::new()
        $stream = $entry.Open()
        try { $stream.CopyTo($buf) } finally { $stream.Dispose() }

        if ($isNested) {
          $pending.Enqueue(@("$label!$($entry.FullName)", $buf.ToArray()))
          continue
        }
        # Assert-StaticCrt takes a path, so land the entry on disk to reuse one implementation.
        $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
        try {
          [System.IO.File]::WriteAllBytes($tmp, $buf.ToArray())
          Assert-StaticCrt -Dll $tmp -Label "$label!$($entry.FullName)"
        }
        finally { Remove-Item $tmp -Force -ErrorAction SilentlyContinue }
        if ($isCdylib) { $checkedCdylib++ } else { $checkedRelay++ }
      }
    }
    finally { $zip.Dispose() }
  }

  if ($checkedCdylib -eq 0) { throw "Assert-StaticCrtInPackage: found no mailcal_bindings.dll inside '$Package'" }
  if ($checkedRelay -eq 0) {
    throw @"
Assert-StaticCrtInPackage: found no allodia-mcp.exe inside '$Package'.

The MCP relay is missing from the shipped package. Settings -> Advanced would still offer a
configuration snippet, and it would name a command the installed app does not carry, so every
assistant that followed it would report the server as broken. Nothing else catches this: the app
builds, launches and renders the whole panel without it.

Build it before packaging (package.ps1 does, for both arches):
  cargo build -p mailcal-mcp-shim --bin allodia-mcp --release --target <triple>
"@
  }
  Write-Host "==> Verified $checkedCdylib packaged cdylib(s) and $checkedRelay relay(s) link the CRT statically" -ForegroundColor DarkGray
}
