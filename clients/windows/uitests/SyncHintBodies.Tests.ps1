# The background hint's SECOND phase: warming message bodies after a sync.
#
# WHY IT IS ITS OWN SUITE. A `MAILCAL_*` switch is read once at startup, so one launch is one
# state, the folder phase and the body phase cannot be staged in the same run, and
# SyncHint.Tests.ps1 owns the folder one. What differs here is only which string the client picks,
# but that is exactly the class of bug this suite exists for: an unread field reads as `false`, the
# folder branch is taken, and the footer says "0 of 0 folders" for three minutes of real
# downloading. Nothing headless can see that.
#
# WHAT THE PHASE IS. After a pass syncs an account's folders, every synced message's body is warmed
# so the mailbox reads offline. On a first sync it is the longer half, nearly six minutes on the
# largest account measured, and it used to report nothing at all. It has no total (the warm drains
# against what is still missing), so the caption says how far it has got and no more.
#
# THE FIXTURE is the `en` showcase seed, whose bring-up signals Surface.SyncProgress at least once.

$Account = 'eva.jansen@example.com'
# Deliberately over a thousand, so the caption cannot pass without the client's thousands
# separator, the runner pins the showcase locale to `en`, which fixes it to a comma.
$Bodies = 2022
$ExpectedHint = "Syncing $Account — 2,022 messages so far"

$Suite = @{
  Dataset = 'showcase'
  # No denominator: that is what selects the body phase, and it is the warm's real shape.
  Env     = @{ MAILCAL_FAKE_SYNC_HINT = "${Account}:$Bodies" }
  Cases   = @(
    @{
      Name = 'a body warm reports how many messages are down so far, not a folder count'
      Body = {
        $hint = Wait-UiaElement -AutomationId 'SyncHint' -TimeoutSec 30
        if (-not $hint) {
          throw "no SyncHint within 30s. Either the staged hint never reached the window (is this a DEBUG build? MAILCAL_FAKE_SYNC_HINT is compiled out of Release), or SyncHintVisible is not bound"
        }
        $rect = $hint.Current.BoundingRectangle
        if ([double]::IsInfinity($rect.X) -or $rect.Width -le 0 -or $rect.Height -le 0) {
          throw "the hint is in the tree but not rendered (bounds $rect)"
        }
        # The whole caption. A client that never read `warmingBodies` takes the folder branch and
        # renders "Syncing $Account, 0 of 0 folders", which is present, visible, and wrong.
        Assert-Equal $ExpectedHint $hint.Current.Name `
          'the body phase must report messages warmed so far, with no denominator invented for it'
      }
    }
  )
}
