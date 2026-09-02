# The background-sync hint lives INSIDE the footer's status line, it never takes a row of its own.
#
# WHY IT IS A RULE AND NOT A PREFERENCE. The bar (SyncProgressPlacement.Tests.ps1) belongs to a
# download the user started and is waiting on, and it is allowed its own row. This is the other
# half: a poll tick, an IDLE push, a boot catch-up, work nobody asked for. Given a row it would
# resize the list every few minutes for the rest of the session. So it is a caption beside
# "Connected", and the list's geometry is identical whether it is showing or not.
#
# WHY THIS NEEDS A STAGED HINT. A real background pass is up for a fraction of a second against
# any local fixture. MAILCAL_FAKE_SYNC_HINT (debug builds only,
# crates/mailcal-app/src/sync_progress_staged.rs) substitutes the snapshot the host reads and
# nothing else, so everything below it is real: the surface signal, the FFI record, the two bound
# properties, the footer's status line, and the layout pass that places it. A hook that set
# SyncHintVisible on the model instead would keep passing after the core-to-client wiring was cut.
#
# WHAT IT PROVES. That the account was NAMED (the client resolves the id against its own account
# list, a hint that printed a raw internal id would satisfy a visibility check and read as a bug
# to the user), that the folder counts travelled, and that the hint shares the footer's row rather
# than opening one above it.
#
# THE FIXTURE is the `en` showcase seed, whose bring-up dispatches RefreshMail and so signals
# Surface.SyncProgress at least once, which is what carries the staged snapshot to the window. Its
# primary account's id IS its address (boot/inmemory.rs mints the id from the identity), so the id
# staged below is the one the client must resolve to a name.

$Account = 'eva.jansen@example.com'
# Deliberately not round, and deliberately not equal: "0 of 0" would be indistinguishable from an
# unwired binding, and equal counts could pass on either half alone.
$Done = 3
$Total = 12
# Named so it cannot collide with a local inside a case Body: PowerShell variables are
# case-INSENSITIVE, so a `$expected` assigned in a Body IS this variable.
#
# Filled in from the catalog rather than spelled out here. A copy of the sentence goes stale the
# moment the wording moves and nothing notices, because this suite runs on no CI machine: it needs
# a Windows desktop session. It had gone stale, against a separator the dash-hygiene sweep changed
# from an em dash to a colon.
$SyncHintCatalog = Get-Content (Join-Path $PSScriptRoot '../../../messages/en.json') -Raw |
  ConvertFrom-Json
$ExpectedHint = $SyncHintCatalog.sync_hint_account.
  Replace('{account}', $Account).
  Replace('{done}', "$Done").
  Replace('{total}', "$Total")

<#
.SYNOPSIS
The staged hint's caption, once the window has read the surface.
#>
function Get-SyncHint {
  $hint = Wait-UiaElement -AutomationId 'SyncHint' -TimeoutSec 30
  if (-not $hint) {
    throw "no SyncHint within 30s. Either the staged hint never reached the window (is this a DEBUG build? MAILCAL_FAKE_SYNC_HINT is compiled out of Release, app.log carries the warning it writes whenever it is in force), or Surface.SyncProgress no longer reaches the projection, or SyncHintVisible is not bound"
  }
  $hint
}

$Suite = @{
  Dataset = 'showcase'
  # The background pass no fixture will hold still for. Debug-only; the runner clears it after.
  Env     = @{ MAILCAL_FAKE_SYNC_HINT = "${Account}:$Done/$Total" }
  Cases   = @(
    @{
      Name = 'a background sync names its account and its folder counts'
      Body = {
        $hint = Get-SyncHint
        $null = Get-RenderedBounds -Element $hint -What 'the background-sync hint'
        # The whole caption, not a substring: the address proves the client resolved the staged
        # id against its own account list, and the counts prove both numbers travelled. A hint
        # drawn from a hardcoded Visible would satisfy the geometry below and fail this.
        Assert-Equal $ExpectedHint $hint.Current.Name `
          'the hint must name the account and how far through its folders the pass is'
      }
    },
    @{
      Name = 'the hint shares the footer row, and never opens one of its own'
      Body = {
        # THE RULE. Beside "Connected", on the same line, at the same height, not stacked above
        # it and not in a strip between the list and the footer. Both are captions in one
        # StackPanel, so their vertical centres coincide; a hint that had taken its own row would
        # sit clear above the status button instead.
        $hint = Get-RenderedBounds -Element (Get-SyncHint) -What 'the background-sync hint'
        $status = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ConnectionStatus') -What "the footer's connection status"
        $hintCentre = $hint.Top + ($hint.Height / 2)
        $statusCentre = $status.Top + ($status.Height / 2)
        Assert-True ([Math]::Abs($hintCentre - $statusCentre) -le 4) `
          "the hint's centre ($hintCentre) must line up with the connection status' ($statusCentre), they share the footer's status line, and a hint on its own row would resize the list every poll tick"
        Assert-True ($hint.Right -le $status.Left) `
          "the hint (right edge $($hint.Right)) must sit before the connection status (left edge $($status.Left)), the footer reads count, then what is arriving, then whether we are connected"

        # AND THE STATUS IS STILL INSIDE THE FOOTER, not shoved under the action buttons. This is
        # the half a centre-line check cannot see: the status line used to be a horizontal
        # StackPanel, which measures its children against infinite width, so the hint took its
        # full desired width and the status was pushed past the column edge and clipped away
        # entirely, present in the tree, `Empty` on screen. The hint holds the elastic column
        # now, so it is the thing that trims.
        $compose = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ComposeButton') -What 'the compose button'
        Assert-True ($status.Right -le $compose.Left) `
          "the connection status (right edge $($status.Right)) must stay clear of the compose button (left edge $($compose.Left)), a hint that took its full width instead of trimming pushes the status out of the footer"
      }
    },
    @{
      Name = 'a background sync raises no bar, so the message list does not move'
      Body = {
        # The reason the hint exists at all. A background pass used to be promoted to the download
        # bar once it committed anything, which resized the list for work the user never started.
        # Nothing stages a download in this suite, so any ProgressBar here is that promotion back.
        $bars = @(Find-UiaElements -Type 'ProgressBar')
        Assert-Equal 0 $bars.Count `
          'a background sync must not raise the download bar, the hint is what it says instead'

        # And the list still ends where the footer begins: the hint took no space from it.
        $list = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'RowsList') -What 'the message list'
        $hint = Get-RenderedBounds -Element (Get-SyncHint) -What 'the background-sync hint'
        Assert-True ($hint.Top -ge $list.Bottom) `
          "the hint's top ($($hint.Top)) must be at or below the list's bottom ($($list.Bottom))"
      }
    }
  )
}
