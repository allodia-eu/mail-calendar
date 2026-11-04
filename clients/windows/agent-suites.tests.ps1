#!/usr/bin/env pwsh
# Tests for uitests/agent-suites.ps1's reading of the UI suites' runner: its listing, which decides
# which suites a -Dataset runs, and its result report, the only place a UI suite's failure reason
# reaches run-ui-tests.ps1's summary. Run by build-and-run.ps1 (so the `windows` CI job gates them)
# and standalone:
#
#   ./agent-suites.tests.ps1
#
# Plain assertions rather than Pester, and exit code as the contract, for the reasons
# screenshot-frame.tests.ps1 gives: 0 all passed, 1 something failed.
#
# The fixtures are the runner's own -list full/json and -result-xml shapes, copied from a run and
# cut to what the readers look at.
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

function Assert-Throws([scriptblock] $body, [string] $like, [string] $what) {
  try {
    & $body | Out-Null
  }
  catch {
    Assert-Equal $true ($_.Exception.Message -like $like) "$what (it threw: $($_.Exception.Message))"
    return
  }
  $script:Failed++
  Write-Host "  FAIL $what, nothing was thrown" -ForegroundColor Red
}

$suites = 'Allodia.Mailcal.UiTests.Suites'

# The listing: one entry per case, each carrying its class's traits.
function Case([string] $class, [string] $method, [string[]] $datasets, [bool] $explicit = $false) {
  [ordered]@{
    Class    = "$suites.$class"
    Method   = $method
    Explicit = $explicit
    Traits   = $(if ($datasets) { @{ Dataset = $datasets } } else { @{} })
  }
}
function Listing([object[]] $cases) { ConvertTo-Json -InputObject @($cases) -Depth 4 -Compress }

$listed = @(ConvertFrom-AgentTestList -Listing (Listing @(
      (Case 'UnreadEmphasisTests' 'AnUnreadMessageBoldsItsSubject' @('showcase')),
      (Case 'UnreadEmphasisTests' 'AReadMessageLeavesSubjectAndSenderAlone' @('showcase')),
      (Case 'DraftsTests' 'ASavedDraftComesBack' @('harness')),
      (Case 'ProbeTests' 'OnlyWhenAsked' @() $true)
    )) | Sort-Object Name)
Assert-Equal 'Drafts.Tests=harness,UnreadEmphasis.Tests=showcase' (($listed | ForEach-Object { "$($_.Name)=$($_.Dataset)" }) -join ',') `
  'a suite is its class, named as -Filter matches it, once however many cases it holds, and an explicit case places none'
Assert-Equal 'DraftsTests' ($listed[0].Class) 'a suite keeps its class name, which is what the runner selects by'
Assert-Throws { ConvertFrom-AgentTestList -Listing (Listing @((Case 'OrphanTests' 'Anything' @()))) } '*OrphanTests declares no *' `
  'a suite that declares no dataset is refused, since no -Dataset would run it'
Assert-Throws { ConvertFrom-AgentTestList -Listing (Listing @((Case 'TwiceTests' 'One' @('harness')), (Case 'TwiceTests' 'Two' @('showcase')))) } `
  '*TwiceTests declares the datasets harness and showcase*' 'a suite whose cases declare two datasets is refused, since both would run it'

# The report.
function Test([string] $class, [string] $method, [string] $result, [string] $inner = '') {
  "<test name=""$suites.$class.$method"" result=""$result"" type=""$suites.$class"" method=""$method"">$inner</test>"
}

$report = [xml] @"
<assemblies schema-version="3">
  <assembly name="Mailcal.UiTests.dll">
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
      $(Test 'FolderDropTests' 'AMessageDroppedOnTheInboxLeavesProjectsAndArrivesInTheInbox' 'Pass')
      $(Test 'FolderDropTests' 'DroppedBackOnProjectsItReturnsThere' 'Fail' @'
<failure exception-type="Xunit.Sdk.TrueException"><message>the message left the Inbox</message>
<stack-trace>   at Allodia.Mailcal.UiTests.Suites.FolderDropTests.DroppedBackOnProjectsItReturnsThere() in D:\repos\FolderDropTests.cs:line 63</stack-trace></failure>
'@)
      $(Test 'ImapSignInTests' 'AServerOfferingBothLeadsWithTheSignInAndKeepsThePasswordBehindItsOwnControl' 'Fail' @'
<failure exception-type="System.InvalidOperationException"><message>System.InvalidOperationException : the form never reached the sign-in offer within 25 s
---- System.TimeoutException : inner reason</message>
<stack-trace>   at Allodia.Mailcal.UiTests.Suites.ImapSignInTests.Watch(App app) in D:\repos\ImapSignInTests.cs:line 113</stack-trace></failure>
'@)
      $(Test 'DraftsTests' 'ASavedDraftComesBack' 'Skip' '<reason>the harness is not up (scripts/dev/harness.sh up)</reason>')
    </collection>
    <collection name="showcase">
      $(Test 'SyncHintTests' 'TheHintSharesTheFooterRowAndNeverOpensOneOfItsOwn' 'Pass')
      $(Test 'NotificationLaunchTests' 'ANotificationActivationDoesNotFailFastOnStartup' 'Skip')
      $(Test 'UnreadEmphasisTests' 'AnUnreadMessageBoldsItsSubject' 'Fail' @'
<failure exception-type="Xunit.Sdk.EqualException"><message>Assert.Equal() Failure: Values differ
Expected: 600
Actual:   400</message></failure>
'@)
      $(Test 'ExplicitTests' 'OnlyWhenAsked' 'NotRun')
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
Assert-Equal 'UnreadEmphasis.Tests / AnUnreadMessageBoldsItsSubject: Assert.Equal() Failure: Values differ Expected: 600 Actual:   400' ($result.Failed[2]) `
  "an assertion's own message keeps every line"
Assert-Equal 'Drafts.Tests / ASavedDraftComesBack: the harness is not up (scripts/dev/harness.sh up)' ($result.Skipped[0]) 'a skip carries the reason the case gave'
Assert-Equal 'NotificationLaunch.Tests / ANotificationActivationDoesNotFailFastOnStartup: ' ($result.Skipped[1]) 'a skip without a reason reads as one'
Assert-Equal "SyncHint.Tests (test-class-cleanup): Class fixture type '$suites.SyncHintApp' threw in Dispose ---- System.InvalidOperationException : the app would not stop" ($result.Failed[3]) `
  'an error outside any case is a failure, named after the suite that raised it'
Assert-Equal '2 passed, 4 failed, 2 skipped' "$($result.Passed.Count) passed, $($result.Failed.Count) failed, $($result.Skipped.Count) skipped" `
  'an explicit case nobody asked for is no result'
Assert-Equal 0 ((ConvertFrom-AgentTestReport -Report ([xml] '<assemblies />')).Failed.Count) 'an empty report is no cases'

Write-Host "agent-suites: $script:Passed passed$(if ($script:Failed) { ", $script:Failed failed" })"
exit ([int]($script:Failed -gt 0))
