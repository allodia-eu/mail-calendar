#!/usr/bin/env pwsh
# Refuses a development core in what is about to be packaged (docs/ai.md rule 5).
#
# A development core (a debug build, or one with `dev-harness`) offers the writing assistant to
# everyone, so no package may carry one. The core compiles in a marker saying which kind it is
# (crates/mailcal-bindings/src/core_build.rs); this reads it from the DLL's bytes, the same two
# strings scripts/dev/check-core-build.sh reads, so a prebuilt cdylib is judged without building
# anything. A DLL with no marker at all is refused too: a check that passes on a missing marker
# passes on everything.

function Assert-ProductionCore {
  param(
    [Parameter(Mandatory)] [string] $Dll
  )

  if (-not (Test-Path $Dll)) { throw "Assert-ProductionCore: no binary at '$Dll'" }
  # As Assert-StaticCrt reads the import names: the marker is ASCII, so the image read as ASCII
  # holds it wherever it sits.
  $bytes = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($Dll))
  if ($bytes.Contains('mailcal-core-build:development')) {
    throw @"
$Dll is a DEVELOPMENT core (a debug build, or one with dev-harness). It offers the writing
assistant to everyone and must not be packaged: build it with build-native.ps1, which builds in
release without dev-harness (docs/ai.md rule 5).
"@
  }
  if (-not $bytes.Contains('mailcal-core-build:production')) {
    throw @"
$Dll carries no production core marker: a core built before it existed, or one the linker dropped
it from. Build it again with build-native.ps1.
"@
  }
}
