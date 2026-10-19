#!/usr/bin/env pwsh
# The UI suites that reach the app through its test agent (clients/windows/Mailcal.UiTests), run
# from run-ui-tests.ps1 so one command still runs every suite. Dot-source it; it defines functions.
#
# A suite there is a `<Name>Tests` class in Mailcal.UiTests/Suites, named here `<Name>.Tests` like
# a file in this directory, so `-Filter Drafts*` selects `DraftsTests`. Each declares its dataset
# as a trait, `[Trait("Dataset", "showcase")]`, which is what -Dataset selects on; it launches the
# app itself, through the agent.

$AgentSuitesProject = Join-Path $PSScriptRoot '..\Mailcal.UiTests'

<#
.SYNOPSIS
The agent suites -Filter selects, as @{ Name = 'Drafts.Tests'; Class = 'DraftsTests'; Dataset = 'harness' }.
A suite that declares no dataset is refused: it would run under no -Dataset at all.
#>
function Get-AgentSuites {
  param([Parameter(Mandatory)] [string] $Filter)
  Get-ChildItem -Path (Join-Path $AgentSuitesProject 'Suites') -Filter '*Tests.cs' |
    ForEach-Object {
      $declared = Select-String -LiteralPath $_.FullName -Pattern '\[Trait\("Dataset", "([a-z-]+)"\)\]' | Select-Object -First 1
      if (-not $declared) { throw "$($_.Name) declares no [Trait(""Dataset"", …)], so no -Dataset would run it" }
      [pscustomobject]@{
        Name    = ($_.BaseName -replace 'Tests$', '') + '.Tests'
        Class   = $_.BaseName
        Dataset = $declared.Matches[0].Groups[1].Value
      }
    } |
    Where-Object { $_.Name -like $Filter } |
    Sort-Object Name
}

<#
.SYNOPSIS
Runs the given agent suites with `dotnet test` and returns what each case did, as
@{ Passed; Failed; Skipped } lists of "<suite> / <case>: <why>".
.DESCRIPTION
-RequireHarness reaches them as MAILCAL_UITESTS_REQUIRE_HARNESS=1, so a missing harness fails their
cases instead of skipping them, as it does every other harness suite's.
#>
function Invoke-AgentSuites {
  param([Parameter(Mandatory)] [object[]] $Suites, [switch] $RequireHarness)
  $filter = ($Suites | ForEach-Object { "FullyQualifiedName~.Suites.$($_.Class)." }) -join '|'
  if ($RequireHarness) { $env:MAILCAL_UITESTS_REQUIRE_HARNESS = '1' }
  try {
    $output = & dotnet test $AgentSuitesProject --nologo --filter $filter --logger 'console;verbosity=normal' 2>&1 |
      ForEach-Object { "$_" }
    $exit = $LASTEXITCODE
  }
  finally {
    Remove-Item env:MAILCAL_UITESTS_REQUIRE_HARNESS -ErrorAction SilentlyContinue
  }
  $result = @{ Passed = @(); Failed = @(); Skipped = @() }
  for ($i = 0; $i -lt $output.Count; $i++) {
    if ($output[$i] -notmatch '^\s+(Passed|Failed|Skipped) Allodia\.Mailcal\.UiTests\.Suites\.(\w+)Tests\.(\w+)') { continue }
    $case = "$($Matches[2]).Tests / $($Matches[3])"
    switch ($Matches[1]) {
      'Passed' { $result.Passed += $case; Write-Host "  PASS  $case" -ForegroundColor Green }
      'Skipped' {
        $why = if ($i + 1 -lt $output.Count) { $output[$i + 1].Trim() } else { '' }
        $result.Skipped += "${case}: $why"; Write-Host "  SKIP  $case" -ForegroundColor Yellow
      }
      'Failed' {
        $why = ($output[($i + 1)..([Math]::Min($i + 3, $output.Count - 1))] | Where-Object { $_ -match 'Exception|Assert' } | Select-Object -First 1)
        $result.Failed += "${case}: $("$why".Trim())"; Write-Host "  FAIL  $case" -ForegroundColor Red
        Write-Host "        $("$why".Trim())" -ForegroundColor Red
      }
    }
  }
  # A build that failed, or a run that reported nothing, is a failure that names no case.
  if ($exit -ne 0 -and $result.Failed.Count -eq 0) {
    $result.Failed += "agent suites: dotnet test exited $exit without a failing case; its output ends: $(($output | Select-Object -Last 5) -join ' | ')"
  }
  $result
}
