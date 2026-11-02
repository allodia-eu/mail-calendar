#!/usr/bin/env pwsh
# Reading `dotnet test`'s output for run-ui-tests.ps1's summary, apart from the runner so
# agent-suites.tests.ps1 can test it on captured output. Dot-source it; it defines a function.

<#
.SYNOPSIS
What each case did, read from `dotnet test`'s console logger at normal verbosity, as
@{ Passed; Failed; Skipped } lists of "<suite> / <case>: <why>" and the same cases in run order.
.DESCRIPTION
A failure's reason is the logger's "Error Message:" block, every line up to "Stack Trace:". Its
first line is the message an assertion was given, which names neither an exception nor an assert
unless the case threw one.

A skip's reason is not beside the logger's "Skipped" line, which is followed by whatever the run
printed next. xUnit's own diagnostic lines carry it: "[xUnit.net <time>] <case> [SKIP]", then the
reason on the next of its lines.
#>
function Get-AgentSkipReason {
  param([string[]] $Lines, [string] $FullName)
  $prefix = '^\[xUnit\.net [^\]]+\]\s+'
  $skip = "$prefix$([regex]::Escape($FullName)) \[SKIP\]\s*$"
  for ($i = 0; $i -lt $Lines.Count - 1; $i++) {
    if ($Lines[$i] -match $skip -and $Lines[$i + 1] -match $prefix) {
      return ($Lines[$i + 1] -replace $prefix, '').Trim()
    }
  }
  ''
}

function ConvertFrom-AgentTestOutput {
  param([AllowEmptyCollection()] [AllowEmptyString()] [string[]] $Lines = @())
  $result = @{ Passed = @(); Failed = @(); Skipped = @(); Order = @() }
  for ($i = 0; $i -lt $Lines.Count; $i++) {
    if ($Lines[$i] -notmatch '^\s+(Passed|Failed|Skipped) Allodia\.Mailcal\.UiTests\.Suites\.(\w+)Tests\.(\w+)') { continue }
    $outcome = $Matches[1]
    $case = "$($Matches[2]).Tests / $($Matches[3])"
    $fullName = "Allodia.Mailcal.UiTests.Suites.$($Matches[2])Tests.$($Matches[3])"
    $why = ''
    switch ($outcome) {
      'Passed' { $result.Passed += $case }
      'Skipped' {
        $why = Get-AgentSkipReason -Lines $Lines -FullName $fullName
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
