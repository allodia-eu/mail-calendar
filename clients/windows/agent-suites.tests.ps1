#!/usr/bin/env pwsh
# Tests for uitests/agent-suites.ps1's reading of the UI suites' result report, the only place a UI
# suite's failure reason reaches run-ui-tests.ps1's summary. Run by build-and-run.ps1 (so the
# `windows` CI job gates them) and standalone:
#
#   ./agent-suites.tests.ps1
#
# Plain assertions rather than Pester, and exit code as the contract, for the reasons
# screenshot-frame.tests.ps1 gives: 0 all passed, 1 something failed.
#
# The fixture is the runner's own -result-xml shape, copied from a run and cut to the elements the
# reader looks at. The report lists a collection's cases as they finished and the collections in
# no promised order, so the cases are deliberately out of the order they started in.
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

$suites = 'Allodia.Mailcal.UiTests.Suites'
function Test([string] $class, [string] $method, [string] $result, [int] $second, [string] $inner = '') {
  "<test name=""$suites.$class.$method"" result=""$result"" start-rtf=""2026-11-03T21:57:$('{0:D2}' -f $second).0000000+00:00"" type=""$suites.$class"" method=""$method"">$inner</test>"
}

$report = [xml] @"
<assemblies schema-version="3">
  <assembly name="Mailcal.UiTests.dll" errors="1" failed="4" passed="3" skipped="2" total="9">
    <errors>
      <error type="test-class-cleanup" name="$suites.SyncHintTests">
        <failure exception-type="Xunit.Sdk.TestPipelineException">
          <message>Class fixture type '$suites.SyncHintApp' threw in Dispose
---- System.InvalidOperationException : the app would not stop</message>
          <stack-trace>   at $suites.UiApp.Dispose()</stack-trace>
        </failure>
      </error>
    </errors>
    <collection name="harness">
      $(Test 'FolderDropTests' 'DroppedBackOnProjectsItReturnsThere' 'Fail' 2 @'
<failure exception-type="Xunit.Sdk.TrueException"><message>the message left the Inbox</message>
<stack-trace>   at Allodia.Mailcal.UiTests.Suites.FolderDropTests.DroppedBackOnProjectsItReturnsThere() in D:\repos\FolderDropTests.cs:line 63</stack-trace></failure>
'@)
      $(Test 'FolderDropTests' 'AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox' 'Pass' 1)
      $(Test 'ImapSignInTests' 'AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl' 'Fail' 3 @'
<failure exception-type="System.InvalidOperationException"><message>System.InvalidOperationException : the form never reached the sign-in offer within 25 s
---- System.TimeoutException : inner reason</message>
<stack-trace>   at Allodia.Mailcal.UiTests.Suites.ImapSignInTests.Watch(App app) in D:\repos\ImapSignInTests.cs:line 113</stack-trace></failure>
'@)
      $(Test 'DraftsTests' 'ASavedDraftComesBack' 'Skip' 4 '<reason>the harness is not up (scripts/dev/harness.sh up)</reason>')
    </collection>
    <collection name="showcase">
      $(Test 'SyncHintTests' 'TheHintSharesTheFooterRowAndNeverOpensOneOfItsOwn' 'Pass' 5)
      $(Test 'NotificationLaunchTests' 'ANotificationActivationDoesNotFailFastOnStartup' 'Skip' 6)
      $(Test 'PrintTests' 'PrintIsOfferedBelowTheExportOnceTheBodyHasArrived' 'Pass' 8)
      $(Test 'PrintTests' 'TheJobPrintsTheMessageItWasOpenedFor' 'Fail' 7 '<failure exception-type="System.Exception" />')
      $(Test 'UnreadEmphasisTests' 'AnUnreadMessageBoldsItsSubject' 'Fail' 9 @'
<failure exception-type="Xunit.Sdk.EqualException"><message>Assert.Equal() Failure: Values differ
Expected: 600
Actual:   400</message></failure>
'@)
      $(Test 'ExplicitTests' 'OnlyWhenAsked' 'NotRun' 10)
    </collection>
  </assembly>
</assemblies>
"@
$result = ConvertFrom-AgentTestReport -Report $report

Assert-Equal 'FolderDrop.Tests / AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox' ($result.Passed[0]) 'a pass is named by its suite and case'
Assert-Equal 'FolderDrop.Tests / DroppedBackOnProjectsItReturnsThere: the message left the Inbox' ($result.Failed[0]) `
  "a failed assertion's reason is its message, though it names no exception and no assert"
Assert-Equal 'ImapSignIn.Tests / AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl: System.InvalidOperationException : the form never reached the sign-in offer within 25 s ---- System.TimeoutException : inner reason' ($result.Failed[1]) `
  'a thrown exception keeps its type and every line of the message, and nothing of the stack'
Assert-Equal 'Drafts.Tests / ASavedDraftComesBack: the harness is not up (scripts/dev/harness.sh up)' ($result.Skipped[0]) 'a skip carries the reason the case gave'
Assert-Equal 'NotificationLaunch.Tests / ANotificationActivationDoesNotFailFastOnStartup: ' ($result.Skipped[1]) `
  'a skip without a reason reads no other case as its reason'
Assert-Equal 'Print.Tests / TheJobPrintsTheMessageItWasOpenedFor: ' ($result.Failed[2]) 'a failure with no message reads no other case as its reason'
Assert-Equal 'UnreadEmphasis.Tests / AnUnreadMessageBoldsItsSubject: Assert.Equal() Failure: Values differ Expected: 600 Actual:   400' ($result.Failed[3]) `
  "an assertion's own message keeps every line"
Assert-Equal "SyncHint.Tests (test-class-cleanup): Class fixture type '$suites.SyncHintApp' threw in Dispose ---- System.InvalidOperationException : the app would not stop" ($result.Failed[4]) `
  'an error outside any case is a failure, named after the suite that raised it'
Assert-Equal 3 ($result.Passed.Count) 'the case after a messageless failure is still read'
Assert-Equal 'Passed,Failed,Failed,Skipped,Passed,Skipped,Failed,Passed,Failed,Failed' (($result.Order | ForEach-Object Outcome) -join ',') `
  'the cases keep the order they started in, an explicit case nobody asked for is no result, and errors come last'
Assert-Equal 0 ((ConvertFrom-AgentTestReport -Report ([xml] '<assemblies />')).Order.Count) 'an empty report is no cases'

Write-Host "agent-suites: $script:Passed passed$(if ($script:Failed) { ", $script:Failed failed" })"
exit ([int]($script:Failed -gt 0))
