#!/usr/bin/env pwsh
# Tests for uitests/agent-suites.ps1's reading of `dotnet test` output, the only place an agent
# suite's failure reason reaches run-ui-tests.ps1's summary. Run by build-and-run.ps1 (so the
# `windows` CI job gates them) and standalone. It lives here rather than in uitests/ because the
# runner there loads every *.tests.ps1 as a UI suite:
#
#   ./agent-suites.tests.ps1
#
# Plain assertions rather than Pester, and exit code as the contract, for the reasons
# screenshot-frame.tests.ps1 gives: 0 all passed, 1 something failed.
#
# The fixture is the console logger's own shape, copied from a run. A failed assertion's message
# carries neither "Exception" nor "Assert", so a reader keyed on those words reports the failure
# with no reason at all, and an intermittent failure then leaves nothing to diagnose it from.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'uitests/agent-suites.ps1')

$script:Failed = 0
$script:Passed = 0

function Assert-Equal($expected, $actual, [string] $what) {
  if ("$expected" -eq "$actual") {
    $script:Passed++
  }
  else {
    $script:Failed++
    Write-Host "  FAIL $what, expected <$expected>, got <$actual>" -ForegroundColor Red
  }
}

$output = @(
  '  Passed Allodia.Mailcal.UiTests.Suites.FolderDropTests.AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox [4 s]',
  '  Failed Allodia.Mailcal.UiTests.Suites.FolderDropTests.DroppedBackOnProjectsItReturnsThere [31 s]',
  '  Error Message:',
  '   the message left the Inbox',
  '  Stack Trace:',
  '     at Allodia.Mailcal.UiTests.Suites.FolderDropTests.<>c.<DroppedBackOnProjectsItReturnsThere>b__6_0(App app, FolderFlow folders) in D:\repos\FolderDropTests.cs:line 63',
  '  Failed Allodia.Mailcal.UiTests.Suites.ImapSignInTests.AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl [9 s]',
  '  Error Message:',
  '   System.InvalidOperationException : the form never reached the sign-in offer within 25 s',
  '   ---- inner reason',
  '  Stack Trace:',
  '     at Allodia.Mailcal.UiTests.Suites.ImapSignInTests.Watch(App app) in D:\repos\ImapSignInTests.cs:line 113',
  '  Skipped Allodia.Mailcal.UiTests.Suites.DraftsTests.ASavedDraftComesBack [1 ms]',
  '  the harness is not up',
  '  Failed Allodia.Mailcal.UiTests.Suites.PrintTests.TheJobPrintsTheMessageItWasOpenedFor [2 s]',
  '  Passed Allodia.Mailcal.UiTests.Suites.PrintTests.PrintIsOfferedBelowTheExportOnceTheBodyHasArrived [1 s]'
)
$result = ConvertFrom-AgentTestOutput -Lines $output

Assert-Equal 'FolderDrop.Tests / AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox' ($result.Passed[0]) 'a pass is named by its suite and case'
Assert-Equal 'FolderDrop.Tests / DroppedBackOnProjectsItReturnsThere: the message left the Inbox' ($result.Failed[0]) `
  "a failed assertion's reason is its message, though it names no exception and no assert"
Assert-Equal 'ImapSignIn.Tests / AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl: System.InvalidOperationException : the form never reached the sign-in offer within 25 s ---- inner reason' ($result.Failed[1]) `
  'a thrown exception keeps its type and every line of the message, and nothing of the stack'
Assert-Equal 'Drafts.Tests / ASavedDraftComesBack: the harness is not up' ($result.Skipped[0]) 'a skip carries the line after it'
Assert-Equal 'Print.Tests / TheJobPrintsTheMessageItWasOpenedFor: ' ($result.Failed[2]) 'a failure with no message block reads no other case as its reason'
Assert-Equal 2 ($result.Passed.Count) 'the case after a messageless failure is still read'
Assert-Equal 'Passed,Failed,Failed,Skipped,Failed,Passed' (($result.Order | ForEach-Object Outcome) -join ',') 'the cases keep the order they ran in'
Assert-Equal 0 ((ConvertFrom-AgentTestOutput -Lines @()).Order.Count) 'no output is no cases'

Write-Host "agent-suites: $script:Passed passed$(if ($script:Failed) { ", $script:Failed failed" })"
exit ([int]($script:Failed -gt 0))
