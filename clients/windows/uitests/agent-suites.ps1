#!/usr/bin/env pwsh
# Reading the UI suites' result report for run-ui-tests.ps1's summary, apart from the runner so
# agent-suites.tests.ps1 can test it on a captured report. Dot-source it; it defines a function.

<#
.SYNOPSIS
What each case did, read from the xUnit v2+ XML report the suites' runner writes (-result-xml), as
@{ Passed; Failed; Skipped } lists of "<suite> / <case>: <why>" and the same cases in run order.
.DESCRIPTION
A failure's reason is its message, every line of it joined: the message an assertion was given, or
an exception's type and message followed by each inner one's. A skip's reason is the one the case
gave. An error outside any case, such as a launch fixture that failed to stop its app, is a failure
named after what raised it, because a run that leaves one is not a clean one.
#>
function ConvertFrom-AgentTestReport {
  param([Parameter(Mandatory)] [xml] $Report)
  $result = @{ Passed = @(); Failed = @(); Skipped = @(); Order = @() }
  # A case with no start time, if the runner ever writes one, goes last rather than ending the read.
  $tests = @($Report.SelectNodes('//test') | Sort-Object {
      $start = [DateTimeOffset]::MaxValue
      [void] [DateTimeOffset]::TryParse($_.GetAttribute('start-rtf'), [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::None, [ref] $start)
      $start
    })
  foreach ($test in $tests) {
    $suite = $test.GetAttribute('type') -replace '^.*\.', '' -replace 'Tests$', ''
    $case = "$suite.Tests / $($test.GetAttribute('method'))"
    # NotRun is an explicit case nobody asked for, which is not a result.
    $outcome = @{ Pass = 'Passed'; Skip = 'Skipped'; Fail = 'Failed' }[$test.GetAttribute('result')]
    if (-not $outcome) { continue }
    $why = switch ($outcome) {
      'Skipped' { "$($test.SelectSingleNode('reason').InnerText)".Trim() }
      'Failed' { Join-AgentMessage $test.SelectSingleNode('failure/message') }
      default { '' }
    }
    $result[$outcome] += if ($outcome -eq 'Passed') { $case } else { "${case}: $why" }
    $result.Order += [pscustomobject]@{ Outcome = $outcome; Case = $case; Why = $why }
  }
  foreach ($fault in @($Report.SelectNodes('//errors/error'))) {
    # Named after the suite when a suite raised it ("test-class-cleanup" of a class), else as given.
    $name = "$($fault.GetAttribute('name'))" -replace '^.*\.Suites\.(\w+)Tests$', '$1.Tests'
    $what = (@($name, "($($fault.GetAttribute('type')))") | Where-Object { $_ -and $_ -ne '()' }) -join ' '
    $why = Join-AgentMessage $fault.SelectSingleNode('failure/message')
    $result.Failed += "${what}: $why"
    $result.Order += [pscustomobject]@{ Outcome = 'Failed'; Case = $what; Why = $why }
  }
  $result
}

function Join-AgentMessage {
  param($Node)
  if (-not $Node) { return '' }
  (@("$($Node.InnerText)" -split '\r?\n' | ForEach-Object { $_.Trim() } | Where-Object { $_ }) -join ' ')
}
