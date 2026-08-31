# The download bar occupies its OWN row between the message list and the footer, never above the
# list. That placement is the rule this suite holds.
#
# WHY IT IS A RULE AND NOT A PREFERENCE. Above the list, every appearance and disappearance
# resized the ListView and moved every row under the pointer, for a background pass the user
# never started. Below it, the list's top edge cannot move: a row under the list can shrink the
# viewport, it can never push the rows down.
#
# WHY THIS NEEDS A STAGED DOWNLOAD. A real pass is up for a fraction of a second against any local
# fixture, short enough that this was checked by eye on every platform and asserted on none, and
# short enough that a suite racing it would be flaky rather than wrong. MAILCAL_FAKE_SYNC_PROGRESS
# (debug builds only, crates/mailcal-app/src/sync_progress_staged.rs) substitutes the snapshot the host
# reads and nothing else, so everything below it is real: the surface signal, the FFI record, the
# five bound properties, the Auto row, and the layout pass that places it. A hook that set
# SyncProgressVisible on the model instead would keep passing after the core-to-client wiring was
# cut, which is the one class of bug this suite exists to catch.
#
# WHAT IT PROVES, AND WHAT IT DOES NOT. It proves the geometry: the bar is below the list, above
# the footer, and the footer is still the bottom-most row (the renumbering this change had to
# make, the footer moved from Grid.Row 3 to 4). It does NOT watch the bar appear and disappear:
# a MAILCAL_* switch is read once at startup, so one launch is one state. That is enough, because
# the geometry IS the rule, a bar that begins below the list cannot move what is above it.
#
# THE FIXTURE is the `en` showcase seed, whose bring-up dispatches RefreshMail and so signals
# Surface.SyncProgress at least once, which is what carries the staged snapshot to the window.

# Deliberately not round numbers: a caption reading "0 of 0" would be indistinguishable from an
# unwired binding, and both counts carry a thousands separator so the caption cannot pass on the
# fetched half alone. The runner pins the showcase locale to `en`, which fixes the separator.
$Fetched = 1200
$Total = 3387
# Named so it cannot collide with a local inside a case Body: PowerShell variables are
# case-INSENSITIVE, so a `$caption` assigned in a Body IS this variable, and the pipeline on the
# right of that assignment would match against the empty local it had just created, which finds
# any Text element with no name and reports a confident PASS.
$ExpectedCaption = 'Downloading 1,200 of 3,387*'

<#
.SYNOPSIS
The staged download's ProgressBar, once the window has read the surface.
#>
function Get-DownloadBar {
  $bar = Wait-UiaElement -Type 'ProgressBar' -TimeoutSec 30
  if (-not $bar) {
    throw "no ProgressBar within 30s. Either the staged download never reached the window (is this a DEBUG build? MAILCAL_FAKE_SYNC_PROGRESS is compiled out of Release, app.log carries the warning it writes whenever it is in force), or Surface.SyncProgress no longer reaches the projection, or SyncProgressVisible is not bound"
  }
  # One bar, and it has to be THIS one: a second ProgressBar on this screen would make every
  # measurement below a coin toss over which element the walk reached first.
  $all = @(Find-UiaElements -Type 'ProgressBar')
  if ($all.Count -ne 1) {
    throw "expected exactly one ProgressBar on the mail list, found $($all.Count), this suite measures 'the' bar and can no longer tell which is which"
  }
  $bar
}

$Suite = @{
  Dataset = 'showcase'
  # The download no fixture will hold still for. Debug-only; the runner clears it after the suite.
  Env     = @{ MAILCAL_FAKE_SYNC_PROGRESS = "$Fetched/$Total" }
  Cases   = @(
    @{
      Name = 'a reported download raises the bar, carrying the counts the core gave it'
      Body = {
        $bar = Get-DownloadBar
        $null = Get-RenderedBounds -Element $bar -What 'the download bar'

        # The caption is what proves the numbers travelled, rather than only the visibility flag:
        # the client assembles it from SyncProgressText, which reads both counts. A bar drawn from
        # a hardcoded Visible would satisfy every geometric case below and fail this one.
        $shown = @(Find-UiaElements -Type 'Text' | ForEach-Object { $_.Current.Name })
        Assert-True ([bool] ($shown | Where-Object { $_ -like $ExpectedCaption })) `
          "the caption beside the bar must read '$ExpectedCaption', the counts the core reported. Text on screen: $($shown -join ' | ')"
      }
    },
    @{
      Name = 'the bar sits BELOW the message list, so a background sync cannot move a row'
      Body = {
        # THE REGRESSION. Above the list the bar was inside the banner stack, so its bottom edge
        # sat at or above the list's TOP, and every row moved by the bar's height each time a
        # sync started. Whatever else changes about the row, this comparison decides it.
        $list = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'RowsList') -What 'the message list'
        $bar = Get-RenderedBounds -Element (Get-DownloadBar) -What 'the download bar'
        Assert-True ($bar.Top -ge $list.Bottom) `
          "the bar's top ($($bar.Top)) must be at or below the list's bottom ($($list.Bottom)), above the list it resizes the list and shifts every row for a pass the user never started"
      }
    },
    @{
      Name = 'and ABOVE the footer, which is still the bottom-most row'
      Body = {
        # The other half of "its own row": the footer moved from Grid.Row 3 to 4 to make space for
        # it. Get that renumbering wrong and the two share a row, drawn over each other, which
        # the XAML compiler is perfectly happy with.
        $bar = Get-RenderedBounds -Element (Get-DownloadBar) -What 'the download bar'
        foreach ($id in 'ConnectionStatus', 'ComposeButton', 'RefreshButton') {
          $el = Get-RenderedBounds -Element (Find-UiaElement -AutomationId $id) -What "the footer's $id"
          Assert-True ($el.Top -ge $bar.Bottom) `
            "$id starts at $($el.Top), above the bar's bottom ($($bar.Bottom)), the footer keeps its own row under the bar, and overlapping controls still render"
        }
      }
    },
    @{
      Name = 'no message row is left underneath the bar'
      Body = {
        # The viewport gives up the space; it is not merely overdrawn. A list whose rows ran on
        # behind the bar would satisfy every comparison above while hiding mail under it.
        $bar = Get-RenderedBounds -Element (Get-DownloadBar) -What 'the download bar'
        $rows = @(Get-MailRows)
        Assert-GreaterThan 0 $rows.Count 'the showcase seed must put rows in the list, or this proves nothing'
        foreach ($row in $rows) {
          $top = $row.Current.BoundingRectangle.Top
          Assert-True ($top -lt $bar.Top) `
            "a message row starts at $top, at or below the bar's top ($($bar.Top)), the list must give up the space rather than draw beneath it"
        }
      }
    }
  )
}
