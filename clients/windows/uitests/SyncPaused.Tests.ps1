# The status line's THIRD state: an account whose server asked us to wait.
#
# WHY IT IS ITS OWN SUITE. A `MAILCAL_*` switch is read once at startup, so one launch is one
# state; SyncHint.Tests.ps1 owns the folder phase and SyncHintBodies.Tests.ps1 the body phase.
#
# WHAT THE STATE IS. A provider answered promptly and declined the work for a while (Gmail's
# per-minute quota, a JMAP concurrency ceiling). Nothing is arriving for the account and nothing
# is wrong with it, which is the one combination neither the bar nor the hint can express, and
# which used to read as an outage.
#
# WHAT ONLY A REAL WINDOW CAN SHOW. The line is a short label and the wait lives in a tooltip, so
# the two are bound to different properties and only one of them is on screen. A client that bound
# the caption and forgot the tooltip renders a perfectly plausible "Sync paused" that never says
# when, and no headless check notices: both properties exist and both return a string.
#
# THE FIXTURE is the `en` showcase seed, whose bring-up signals Surface.SyncProgress at least once.

$PausedAccount = 'eva.jansen@example.com'
$PausedMinutes = 5
# From the catalog, never a copy of the sentence: this suite runs on no CI machine (it needs a
# Windows desktop session), so a spelled-out copy goes stale silently.
$SyncPausedCatalog = Get-Content (Join-Path $PSScriptRoot '../../../messages/en.json') -Raw |
  ConvertFrom-Json
$ExpectedLabel = $SyncPausedCatalog.sync_paused
$ExpectedDetail = $SyncPausedCatalog.sync_paused_detail.
  Replace('{account}', $PausedAccount).
  Replace('{count}', $PausedMinutes)

$Suite = @{
  Dataset = 'showcase'
  Env     = @{ MAILCAL_FAKE_SYNC_PAUSED = "${PausedAccount}:$PausedMinutes" }
  Cases   = @(
    @{
      Name = 'a paused account says so in the status line, and says when in its tooltip'
      Body = {
        $status = Wait-UiaElement -AutomationId 'SyncStatus' -TimeoutSec 30
        if (-not $status) {
          throw "no SyncStatus within 30s. Either the staged pause never reached the window (is this a DEBUG build? MAILCAL_FAKE_SYNC_PAUSED is compiled out of Release, app.log carries the warning it writes whenever it is in force), or SyncStatusVisible is not bound"
        }
        $rect = $status.Current.BoundingRectangle
        if ([double]::IsInfinity($rect.X) -or $rect.Width -le 0 -or $rect.Height -le 0) {
          throw "the status line is in the tree but not rendered (bounds $rect)"
        }
        # The spoken name is the whole sentence: bound explicitly, because a screen reader given
        # the TextBlock's own text would hear "Sync paused" and never the wait. Asserting it here
        # is how the tooltip's content is checked without hovering, they are the same string.
        Assert-Equal $ExpectedDetail $status.Current.Name `
          'the paused notice must carry the wait, not just the label'
      }
    }
    @{
      Name = 'the pause keeps the status line off its own row, exactly as the hint does'
      Body = {
        # The same rule the hint holds to: this is a caption in the footer's status line, not a
        # strip between the list and the footer, so the rows never move for it.
        $notice = Get-RenderedBounds -Element (Wait-UiaElement -AutomationId 'SyncStatus' -TimeoutSec 30) -What 'the paused notice'
        $connection = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ConnectionStatus') -What "the footer's connection status"
        $noticeCentre = $notice.Top + ($notice.Height / 2)
        $connectionCentre = $connection.Top + ($connection.Height / 2)
        Assert-True ([Math]::Abs($noticeCentre - $connectionCentre) -le 4) `
          "the paused notice's centre ($noticeCentre) must line up with the connection status' ($connectionCentre); they share the footer's status line"
        $list = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'RowsList') -What 'the message list'
        Assert-True ($notice.Top -ge $list.Bottom) `
          "the paused notice's top ($($notice.Top)) must be at or below the list's bottom ($($list.Bottom))"
      }
    }
  )
}
