#!/usr/bin/env pwsh
# Tests for winapp.ps1's answer to an element that went away during a search. Run by
# build-and-run.ps1 (so the `windows` CI job gates them) and standalone:
#
#   ./winapp.tests.ps1
#
# Plain assertions rather than Pester, and exit code as the contract, for the reasons
# screenshot-frame.tests.ps1 gives: 0 all passed, 1 something failed.
#
# The failure under test depends on the app changing its tree in the moment winapp walks it, which
# no fixture can time. So winapp itself is replaced by a stub that answers the way winapp 0.7.1
# does, an error object on stdout and exit 1, a given number of times before it answers properly,
# and counts how often it was asked.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'winapp.ps1')

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

$Sandbox = Join-Path ([IO.Path]::GetTempPath()) "mailcal-winapp-tests-$PID"
New-Item -ItemType Directory -Force -Path $Sandbox | Out-Null
$Counter = Join-Path $Sandbox 'asked.txt'
$Stub = Join-Path $Sandbox 'winapp.cmd'
Set-Content -LiteralPath $Stub -Encoding ascii -Value @'
@echo off
set n=0
if exist "%WINAPP_STUB_COUNTER%" set /p n=<"%WINAPP_STUB_COUNTER%"
set /a n+=1
>"%WINAPP_STUB_COUNTER%" echo %n%
if %n% LEQ %WINAPP_STUB_FAILURES% goto fail
echo {"matchCount":1,"matches":[{"selector":"stub-1"}]}
exit /b 0
:fail
echo {"error":{"message":"%WINAPP_STUB_ERROR%"}}
exit /b 1
'@
# Get-WinappPath answers from this once it is set, so nothing below reaches the real winapp.
$script:WinappPath = $Stub
$env:WINAPP_STUB_COUNTER = $Counter

<#
.SYNOPSIS
Runs one Invoke-Winapp against a stub that fails -Failures times with -Message first.
.OUTPUTS
How often the stub was asked, the selector it answered with, and the error that escaped, if any.
#>
function Invoke-Stubbed {
  param([string] $Verb, [int] $Failures, [string] $Message = 'Element is no longer accessible')
  Remove-Item -LiteralPath $Counter -ErrorAction SilentlyContinue
  $env:WINAPP_STUB_FAILURES = "$Failures"
  $env:WINAPP_STUB_ERROR = $Message
  $answer = $null
  $thrown = ''
  try { $answer = Invoke-Winapp $Verb @('anything') -Window 1 }
  catch { $thrown = $_.Exception.Message }
  [pscustomobject]@{
    Asked = [int](Get-Content -LiteralPath $Counter -TotalCount 1)
    Selector = if ($answer) { $answer.matches[0].selector } else { '' }
    Thrown = $thrown
  }
}

try {
  # 1. The element came back on the next walk: the search answers, and the caller never sees it.
  $r = Invoke-Stubbed -Verb 'search' -Failures 2
  Assert-Equal 3 $r.Asked 'a search that met a vanished element twice is asked a third time'
  Assert-Equal 'stub-1' $r.Selector 'and the third answer is the one returned'
  Assert-Equal '' $r.Thrown 'and nothing escapes'

  # 2. It never settled: the search gives up after its attempts, and says why.
  $r = Invoke-Stubbed -Verb 'search' -Failures 99
  Assert-Equal $script:WinappSearchAttempts $r.Asked 'a search is asked no more than its attempts'
  Assert-Equal $true ($r.Thrown -match 'Element is no longer accessible') "the last failure escapes: '$($r.Thrown)'"

  # 3. A press is never repeated: it may have landed before the error was reported.
  $r = Invoke-Stubbed -Verb 'click' -Failures 1
  Assert-Equal 1 $r.Asked 'a click that met a vanished element is not asked again'
  Assert-Equal $true ($r.Thrown -match 'Element is no longer accessible') "and its failure escapes: '$($r.Thrown)'"

  # 4. Any other failure of a search is an answer, not a race.
  $r = Invoke-Stubbed -Verb 'search' -Failures 1 -Message 'Window not found'
  Assert-Equal 1 $r.Asked 'a search that failed for another reason is not asked again'
  Assert-Equal $true ($r.Thrown -match 'Window not found') "and its failure escapes: '$($r.Thrown)'"
}
finally {
  Remove-Item Env:WINAPP_STUB_COUNTER, Env:WINAPP_STUB_FAILURES, Env:WINAPP_STUB_ERROR -ErrorAction SilentlyContinue
  Remove-Item -Recurse -Force -LiteralPath $Sandbox -ErrorAction SilentlyContinue
}

if ($script:Failed -gt 0) {
  Write-Host "==> winapp: $($script:Failed) failed, $($script:Passed) passed" -ForegroundColor Red
  exit 1
}
Write-Host "==> winapp: $($script:Passed) passed" -ForegroundColor Green
