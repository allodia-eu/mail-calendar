# The calendar's drawn week grid must be reachable over UI Automation, by id.
#
# WHY THIS EXISTS. The id used to sit on the bare `Grid` the surface is added into, and a bare layout
# panel gets no automation peer, so it reached nothing, on any display, and a wait for it could only
# time out. Nothing noticed because nothing asserted on it: an id no test names is indistinguishable
# from an id that does not work. This suite is what makes that difference visible, and it is the
# whole reason the id moved onto the surface itself.
#
# THE MONTH SURFACE IS NOT HERE, and that is the honest result rather than a thin suite. It
# overrides no `OnCreateAutomationPeer`, so it has no peer, so it is absent from the tree even while
# it is the view on screen, which also means a screen reader cannot read the month at all. Giving
# it an id would have produced a third handle that reaches nothing. Recorded in docs/calendar.md,
# "Known gaps"; §7's "a drawn grid has to be taught to speak" is what closing it looks like.
#
# WHY SHOWCASE. Nothing here needs a real transport, the question is whether a handle resolves to a
# rendered element, not whether anything synced, and the in-memory dataset seeds a week with events
# in it deterministically.
#
# NOTHING BELOW MATCHES A LOCALISED STRING: every handle is an AutomationId, including the view
# menu's, because the zoom levels are the one place a test has to name a view.

# showcase.ps1 relative to this file, captured at load: $PSScriptRoot inside a scriptblock binds to
# whoever invokes it, which is the runner.
$ShowcaseScript = Join-Path $PSScriptRoot '../showcase.ps1'

# A surface that is drawn at all is drawn big, it is the whole detail pane. The floor only has to
# separate "rendered" from "a peer with an empty rectangle", so it stays well clear of both.
$SurfaceFloorDip = 200

<#
.SYNOPSIS
Open the calendar on the showcase dataset, and return once the grid is on screen.
#>
function Open-ShowcaseCalendar {
  & $ShowcaseScript -Locale en -Screen calendar -NoCapture | Out-Null
  if (-not (Wait-UiaElement -AutomationId 'CalendarPeriod' -TimeoutSec 60)) {
    throw 'the calendar header never appeared, so nothing below is about the calendar'
  }
}

<#
.SYNOPSIS
Pick a zoom level from the header's view menu, by id.
#>
function Select-CalendarView {
  param([Parameter(Mandatory)] [string] $Id)
  Invoke-UiaElement (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button)
  $item = Wait-UiaElement -AutomationId $Id -Type MenuItem -TimeoutSec 10
  if (-not $item) { throw "the view menu has no item '$Id', the flyout did not open, or the id is gone" }
  Invoke-UiaElement $item
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'the week grid is reachable by id, and drawn'
      Body = {
        Open-ShowcaseCalendar
        $grid = Wait-UiaElement -AutomationId 'CalendarGrid' -TimeoutSec 30
        if (-not $grid) {
          throw "no CalendarGrid within 30s, an id on the host Grid instead of the surface reaches nothing, and no timeout is long enough to fix that"
        }
        $bounds = Get-RenderedBounds -Element $grid -What 'the week grid'
        Assert-GreaterThan (ConvertTo-UiaPixels $SurfaceFloorDip) $bounds.Height `
          "the grid rendered $($bounds.Height)px tall, it is the detail pane, so anything this small means the id found something that is not the surface"
      }
    },
    @{
      Name = 'picking a different zoom actually puts the week grid away'
      Body = {
        # What stops the case above from passing on a surface that is simply always there: the id
        # has to track what is on screen. Month is the pick, because it is the one zoom that swaps
        # the surface rather than re-drawing this one (§2).
        Open-ShowcaseCalendar
        $null = Get-RenderedBounds -Element (Wait-UiaElement -AutomationId 'CalendarGrid' -TimeoutSec 30) `
          -What 'the week grid'
        Select-CalendarView 'CalendarViewMonth'
        Assert-True (Wait-UiaGone -AutomationId 'CalendarGrid' -TimeoutSec 15) `
          'the week grid is still in the tree after switching to Month, either the view did not change, or the id is on something that outlives the surface it names'
      }
    }
  )
}
