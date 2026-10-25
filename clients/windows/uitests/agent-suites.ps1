#!/usr/bin/env pwsh
# The UI suites that reach the app through its test agent (clients/windows/Mailcal.UiTests), run
# from run-ui-tests.ps1 so one command still runs every suite. Dot-source it; it defines functions.
#
# A suite there is a `<Name>Tests` class in Mailcal.UiTests/Suites, named here `<Name>.Tests` like
# a file in this directory, so `-Filter Drafts*` selects `DraftsTests`.
# They all run on the harness dataset: they launch the app themselves, through the agent.

$AgentSuitesProject = Join-Path $PSScriptRoot '..\Mailcal.UiTests'

<#
.SYNOPSIS
The agent suites -Filter selects, as @{ Name = 'Drafts.Tests'; Class = 'DraftsTests' }.
#>
function Get-AgentSuites {
  param([Parameter(Mandatory)] [string] $Filter)
  Get-ChildItem -Path (Join-Path $AgentSuitesProject 'Suites') -Filter '*Tests.cs' |
    ForEach-Object {
      [pscustomobject]@{ Name = ($_.BaseName -replace 'Tests$', '') + '.Tests'; Class = $_.BaseName }
    } |
    Where-Object { $_.Name -like $Filter } |
    Sort-Object Name
}

<#
.SYNOPSIS
What each case did, read from `dotnet test`'s console logger at normal verbosity, as
@{ Passed; Failed; Skipped } lists of "<suite> / <case>: <why>" and the same cases in run order.
.DESCRIPTION
A failure's reason is the logger's "Error Message:" block, every line up to "Stack Trace:". Its
first line is the message an assertion was given, which names neither an exception nor an assert
unless the case threw one.
#>
function ConvertFrom-AgentTestOutput {
  param([AllowEmptyCollection()] [AllowEmptyString()] [string[]] $Lines = @())
  $result = @{ Passed = @(); Failed = @(); Skipped = @(); Order = @() }
  for ($i = 0; $i -lt $Lines.Count; $i++) {
    if ($Lines[$i] -notmatch '^\s+(Passed|Failed|Skipped) Allodia\.Mailcal\.UiTests\.Suites\.(\w+)Tests\.(\w+)') { continue }
    $outcome = $Matches[1]
    $case = "$($Matches[2]).Tests / $($Matches[3])"
    $why = ''
    switch ($outcome) {
      'Passed' { $result.Passed += $case }
      'Skipped' {
        $why = if ($i + 1 -lt $Lines.Count) { $Lines[$i + 1].Trim() } else { '' }
        $result.Skipped += "${case}: $why"
      }
      'Failed' {
        $message = @()
        $j = $i + 1
        if ($j -lt $Lines.Count -and $Lines[$j].Trim() -eq 'Error Message:') {
          for ($j++; $j -lt $Lines.Count -and $Lines[$j].Trim() -ne 'Stack Trace:'; $j++) {
            if ($Lines[$j] -match '^\s+(Passed|Failed|Skipped) Allodia\.') { break }
            if ($Lines[$j].Trim()) { $message += $Lines[$j].Trim() }
          }
        }
        $why = $message -join ' '
        $result.Failed += "${case}: $why"
      }
    }
    $result.Order += [pscustomobject]@{ Outcome = $outcome; Case = $case; Why = $why }
  }
  $result
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
  $result = ConvertFrom-AgentTestOutput -Lines $output
  foreach ($line in $result.Order) {
    switch ($line.Outcome) {
      'Passed' { Write-Host "  PASS  $($line.Case)" -ForegroundColor Green }
      'Skipped' { Write-Host "  SKIP  $($line.Case)" -ForegroundColor Yellow }
      'Failed' { Write-Host "  FAIL  $($line.Case)" -ForegroundColor Red; Write-Host "        $($line.Why)" -ForegroundColor Red }
    }
  }
  # A build that failed, or a run that reported nothing, is a failure that names no case.
  if ($exit -ne 0 -and $result.Failed.Count -eq 0) {
    $result.Failed += "agent suites: dotnet test exited $exit without a failing case; its output ends: $(($output | Select-Object -Last 5) -join ' | ')"
  }
  $result
}
