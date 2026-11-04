#!/usr/bin/env pwsh
# Reading what the UI suites' runner says, for run-ui-tests.ps1: which suites there are, before a
# run, and what each case did, after one. Apart from the runner so agent-suites.tests.ps1 can test
# both on captured output. Dot-source it; it defines functions.

<#
.SYNOPSIS
The suites the test program compiled, from its own listing (-list full/json), as one
@{ Name; Class; Dataset } each: Name is how -Filter matches it, "<Name>.Tests" for a
<Name>Tests class.
.DESCRIPTION
A suite declares its dataset as a trait on its class, and every case in it carries that one. A
suite with none would run under no -Dataset and one with two under both, so either is refused.
An explicit case runs only when asked for by name, so it places no suite.
#>
function ConvertFrom-AgentTestList {
  param([Parameter(Mandatory)] [string] $Listing)
  @(ConvertFrom-Json $Listing) | Where-Object { -not $_.Explicit } | Group-Object Class | ForEach-Object {
    $class = $_.Name -replace '^.*\.', ''
    $datasets = @($_.Group | ForEach-Object { $_.Traits.Dataset } | Where-Object { $_ } | Sort-Object -Unique)
    if ($datasets.Count -ne 1) {
      $declared = if ($datasets) { "the datasets $($datasets -join ' and ')" } else { 'no [Trait("Dataset", …)]' }
      throw "$class declares $declared, so no one -Dataset runs it"
    }
    [pscustomobject]@{ Name = ($class -replace 'Tests$', '') + '.Tests'; Class = $class; Dataset = $datasets[0] }
  }
}

<#
.SYNOPSIS
What each case did, read from the xUnit v2+ XML report the suites' runner writes (-result-xml), as
@{ Passed; Failed; Skipped } lists of "<suite> / <case>" and, for the last two, ": <why>".
.DESCRIPTION
A failure's reason is its message, every line of it joined: the message an assertion was given, or
an exception's type and message followed by each inner one's. A skip's reason is the one the case
gave. An error outside any case, such as a launch fixture that failed to stop its app, is a failure
named after what raised it, because a run that leaves one is not a clean one.
#>
function ConvertFrom-AgentTestReport {
  param([Parameter(Mandatory)] [xml] $Report)
  $result = @{ Passed = @(); Failed = @(); Skipped = @() }
  foreach ($test in @($Report.SelectNodes('//test'))) {
    $suite = $test.GetAttribute('type') -replace '^.*\.', '' -replace 'Tests$', ''
    $case = "$suite.Tests / $($test.GetAttribute('method'))"
    # NotRun is an explicit case nobody asked for, which is not a result.
    switch ($test.GetAttribute('result')) {
      'Pass' { $result.Passed += $case }
      'Skip' { $result.Skipped += "${case}: $("$($test.SelectSingleNode('reason').InnerText)".Trim())" }
      'Fail' { $result.Failed += "${case}: $(Join-AgentMessage $test.SelectSingleNode('failure/message'))" }
    }
  }
  foreach ($fault in @($Report.SelectNodes('//errors/error'))) {
    # Named after the suite when a suite raised it ("test-class-cleanup" of a class), else as given.
    $name = "$($fault.GetAttribute('name'))" -replace '^.*\.Suites\.(\w+)Tests$', '$1.Tests'
    $what = (@($name, "($($fault.GetAttribute('type')))") | Where-Object { $_ -and $_ -ne '()' }) -join ' '
    $result.Failed += "${what}: $(Join-AgentMessage $fault.SelectSingleNode('failure/message'))"
  }
  $result
}

function Join-AgentMessage {
  param($Node)
  if (-not $Node) { return '' }
  (@("$($Node.InnerText)" -split '\r?\n' | ForEach-Object { $_.Trim() } | Where-Object { $_ }) -join ' ')
}
