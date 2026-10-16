# What the app closes, the runtime gets back: a composer, an editor's web view, a reading window
# and the composer window raised from it.
#
# WHY A PROBE AND NOT THE SCREEN. A closed window that is never freed looks exactly like one that
# is. Its WebView2 renderer still exits, because closing a WebView2 ends that whatever holds the
# managed object, so even a process count stays flat. What grows is the app's own memory, a few
# megabytes per window, for as long as it runs. MAILCAL_RELEASE_PROBE makes the debug build watch
# what it closes, collect until it is gone, and log what is still held (Services/ReleaseProbe.cs);
# this suite reads that line. `Mailcal.Tests` cannot host a window, so nothing else can ask.
#
# WHY SHOWCASE. Nothing here needs a real transport: it opens and closes, and never sends. So CI
# runs it.

. (Join-Path $PSScriptRoot 'appwindows.ps1')
. (Join-Path $PSScriptRoot '../applog.ps1')

$ReleaseCycles = 3
$ReleaseWatchPattern = '\[info\] release probe: watching a closed '
$ReleaseAnswerPattern = '\[info\] release probe: (?<held>\d+) of (?<total>\d+) closed still held'

<#
.SYNOPSIS
Where this launch's probe stands: how many closes it has been told about, and how many of those its
latest answer found still held.
.DESCRIPTION
A case is judged on what it ADDED to both, because the count runs for the whole launch: a case after
one that leaked would otherwise fail for the earlier case's objects.
#>
function Get-ReleaseState {
  $session = Get-AppLogNewestSession
  $answers = [regex]::Matches($session, $ReleaseAnswerPattern)
  @{
    Watches = [regex]::Matches($session, $ReleaseWatchPattern).Count
    Held    = if ($answers.Count -gt 0) { [int] $answers[$answers.Count - 1].Groups['held'].Value } else { 0 }
  }
}

<#
.SYNOPSIS
Waits for the probe to answer for every close since -Before was read, and throws if it found more
held than it did then.
.DESCRIPTION
The answer has to come AFTER the last watch line: an earlier one answered for less than this case
closed. The probe gives up after about ten seconds of collections, so a timeout here means it never
answered at all.
#>
function Assert-ReleasedSince {
  param([Parameter(Mandatory)] [hashtable] $Before, [Parameter(Mandatory)] [string] $What, [int] $TimeoutSec = 30)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ($timer.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $session = Get-AppLogNewestSession
    $watches = [regex]::Matches($session, $ReleaseWatchPattern)
    $answers = [regex]::Matches($session, $ReleaseAnswerPattern)
    if ($watches.Count -gt $Before.Watches -and $answers.Count -gt 0) {
      $answer = $answers[$answers.Count - 1]
      if ($answer.Index -gt $watches[$watches.Count - 1].Index) {
        Assert-Equal $watches.Count ([int] $answer.Groups['total'].Value) `
          'the probe answered for a different number of closes than it logged, so this line is not about this case'
        $added = [int] $answer.Groups['held'].Value - $Before.Held
        Assert-True ($added -le 0) `
          "$added closed object(s) are still in memory after $What. Something the app keeps, often a handler on a native object, still refers to them (docs/client-traps.md)"
        return
      }
    }
    Start-Sleep -Milliseconds 300
  }
  if ((Get-ReleaseState).Watches -le $Before.Watches) {
    throw "the probe was never told about a close during $What, so this case proved nothing. Is MAILCAL_RELEASE_PROBE reaching the app, and is this a debug build?"
  }
  throw "the probe did not answer within ${TimeoutSec}s of $What"
}

function Open-ReleaseComposer {
  $compose = Wait-UiaElement -AutomationId 'ComposeButton' -TimeoutSec 30
  if (-not $compose) { throw 'no ComposeButton within 30s, the mail list never drew its action row' }
  Invoke-UiaElement $compose -SettleMs 0
  if (-not (Wait-UiaElement -AutomationId 'SendButton' -TimeoutSec 30)) {
    throw 'no composer within 30s of pressing Compose'
  }
}

$Suite = @{
  Dataset = 'showcase'
  Env     = @{ MAILCAL_RELEASE_PROBE = '1' }
  Cases   = @(
    @{
      Name = 'a composer closed in the pane is released'
      Body = {
        $before = Get-ReleaseState
        for ($i = 0; $i -lt $ReleaseCycles; $i++) {
          Open-ReleaseComposer
          # The editor draws in a browser window of its own once its WebView2 is up.
          $timer = [Diagnostics.Stopwatch]::StartNew()
          do {
            $editor = Find-UiaElement -AutomationId 'Editor'
            if ($editor -and $editor.Current.NativeWindowHandle -ne 0) { break }
            Start-Sleep -Milliseconds 200
          } while ($timer.Elapsed.TotalSeconds -lt 30)
          $null = Close-Composer
        }
        Assert-ReleasedSince -Before $before -What "$ReleaseCycles composers were discarded"
      }
    },
    @{
      Name = 'a composer discarded before its editor has loaded is released'
      Body = {
        # Discarded while EnsureCoreWebView2Async may still be running: nothing waits for the
        # editor to load first.
        $before = Get-ReleaseState
        for ($i = 0; $i -lt $ReleaseCycles; $i++) {
          Open-ReleaseComposer
          $null = Close-Composer
        }
        Assert-ReleasedSince -Before $before -What "$ReleaseCycles composers were discarded at once"
      }
    },
    @{
      Name = 'a reading window, and the composer window raised from it, are released'
      Body = {
        Close-ExtraWindows
        $before = Get-ReleaseState
        for ($i = 0; $i -lt $ReleaseCycles; $i++) {
          Show-Mailbox
          $row = Get-MailRows | Where-Object { $_.Current.IsEnabled } | Select-Object -First 1
          if (-not $row) { throw 'the message list is empty, so no window can be opened' }
          Invoke-WinappClick $row -Double
          $timer = [Diagnostics.Stopwatch]::StartNew()
          while ((Get-ExtraWindows).Count -lt 1 -and $timer.Elapsed.TotalSeconds -lt 20) { Start-Sleep -Milliseconds 250 }
          $reading = Get-ExtraWindows | Select-Object -First 1
          if (-not $reading) { throw 'double-clicking the first row opened no window' }
          $root = [System.Windows.Automation.AutomationElement]::FromHandle($reading.Handle)
          $reply = $null
          $timer.Restart()
          while (-not $reply -and $timer.Elapsed.TotalSeconds -lt 20) {
            $reply = Find-UiaElement -AutomationId 'ReadingReply' -Root $root
            if (-not $reply) { Start-Sleep -Milliseconds 250 }
          }
          if (-not $reply) { throw 'the reading window never offered Reply' }
          Invoke-UiaElement $reply -SettleMs 0
          $timer.Restart()
          while ((Get-ExtraWindows).Count -lt 2 -and $timer.Elapsed.TotalSeconds -lt 30) { Start-Sleep -Milliseconds 250 }
          if ((Get-ExtraWindows).Count -lt 2) { throw 'Reply in the reading window opened no composer window' }
          # The editor is let load, so its WebView2 is a live one when the window closes.
          Start-Sleep -Seconds 2
          # WM_CLOSE is the window's own close: the composer keeps an untouched reply without asking.
          Close-ExtraWindows
        }
        Assert-ReleasedSince -Before $before -What "$ReleaseCycles reading and composer windows were closed"
      }
    },
    @{
      Name = 'a signature editor is released when it is cancelled'
      Body = {
        $before = Get-ReleaseState
        try {
          for ($i = 0; $i -lt $ReleaseCycles; $i++) {
            $dialog = Open-SettingsCategory -Name 'Signatures'
            $add = Find-UiaElements -Name 'New signature' -Type 'Button' -Root $dialog | Select-Object -First 1
            if (-not $add) { throw 'Settings → Signatures offers no New signature' }
            Invoke-UiaElement $add -SettleMs 2500
            $cancel = Find-UiaElements -Name 'Cancel' -Type 'Button' -Root (Get-SettingsDialog) | Select-Object -First 1
            if (-not $cancel) { throw 'the signature editor offers no Cancel' }
            Invoke-UiaElement $cancel -SettleMs 800
          }
        }
        finally {
          if ($dialog = Find-SettingsDialog) {
            $close = Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $dialog
            if ($close) { Invoke-UiaElement $close }
          }
        }
        Assert-ReleasedSince -Before $before -What "$ReleaseCycles signature editors were cancelled"
      }
    }
  )
}
