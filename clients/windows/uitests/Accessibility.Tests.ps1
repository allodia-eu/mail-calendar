#!/usr/bin/env pwsh
# What a screen reader ANNOUNCES, the one property of the UI that no other gate in this repo can
# see, and that nothing about looking at the app reveals.
#
# Both rules here were red when this file was written (2026-07-31), on a build whose every other
# gate was green:
#
#   * All ten mailbox rows announced "Allodia.Mailcal.ViewModels.MailRow". A ListViewItem filled
#     from a DataTemplate has no UIA Name of its own, so its peer falls back to ToString() on the
#     bound object, and the default ToString() is the type name. Narrator therefore read the same
#     eleven syllables for every message in the mailbox, with no sender, no subject, and no date.
#   * Six buttons announced NOTHING at all: the five reading-pane actions (Reply, Reply all,
#     Forward, Archive, Delete) and the connection-status button. Each has an icon+label panel as
#     its Content rather than a plain string, and WinUI derives a Name only from the latter. The
#     reading five collapse to icon-only at narrow widths, which is precisely when the spoken name
#     is the only name a user has.
#
# Neither is visible in a screenshot, the rows look right, the buttons are legible. Neither can
# reach `Mailcal.Tests`, which links no WinUI. This suite is the only machine that looks.
#
# The dataset is `showcase` because these are rules about PROJECTION and MARKUP, not about
# transport: no mail action is dispatched, and the seeded locale is pinned to `en` so the state
# words ("Unread", "Flagged") are stable strings.

# Names that are a .NET type rather than a human phrase. Deliberately shaped (two or more dotted
# segments, no spaces) rather than pinned to "MailRow": the defect is the DataTemplate fallback,
# which will announce whatever class next arrives in a list, so the rule has to catch the shape.
$TypeNamePattern = '^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*){2,}$'

# Control types a screen-reader user navigates TO and acts on. A Text/Image element may legitimately
# be unnamed (decorative, or its container speaks for it); a Button may not.
$InteractivePattern = 'Button|ListItem|CheckBox|ComboBox|TabItem|MenuItem|Hyperlink'

# The calendar agenda, which the sweep above cannot reach on its own: this suite opens on the mail
# list, and `Get-UiaTree` only ever sees what is on screen. That is exactly how the agenda kept the
# DataTemplate defect this file was written to catch, long after the mailbox lost it, the rule was
# right and simply never looked here. The agenda rather than the grid because the grid is a drawn
# canvas whose events are peers with no text of their own. (Each suite carries its own copy of this:
# the runner loads one file at a time under -Filter.)
function Show-Agenda {
  if (-not (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button)) {
    $nav = Find-UiaElement -AutomationId 'NavCalendar'
    if (-not $nav) { throw 'no Calendar entry in the navigation pane' }
    Invoke-UiaElement $nav -SettleMs 1500
    if (-not (Wait-UiaElement -AutomationId 'CalendarViewMenu' -TimeoutSec 30)) {
      throw 'the calendar did not come up'
    }
  }
  $existing = Find-UiaElement -AutomationId 'CalendarAgenda'
  if ($existing) { return $existing }
  Invoke-UiaElement (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button) -SettleMs 900
  $flyout = Find-UiaElements -Type 'Menu' -Root ([System.Windows.Automation.AutomationElement]::RootElement) |
    Where-Object { $_.Current.ClassName -eq 'MenuFlyout' } | Select-Object -First 1
  if (-not $flyout) { throw 'the view flyout did not open' }
  $items = @(Find-UiaElements -Type 'MenuItem' -Root $flyout)
  Assert-Equal 6 $items.Count 'the view flyout is day / 3 days / work week / week / month / agenda, if that changed, the positional pick below is opening something else'
  Invoke-UiaElement $items[-1] -SettleMs 1500
  $agenda = Find-UiaElement -AutomationId 'CalendarAgenda'
  if (-not $agenda) { throw 'the agenda list did not come up' }
  return $agenda
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'no interactive element announces a .NET type name'
      Body = {
        $offenders = @(Get-UiaTree |
          Where-Object {
            $_.Current.ControlType.ProgrammaticName -match $InteractivePattern -and
            $_.Current.Name -match $TypeNamePattern
          })
        $what = ($offenders | ForEach-Object { $_.Current.Name } | Sort-Object -Unique) -join ', '
        Assert-Equal 0 $offenders.Count (
          "a UIA Name that is a class name is what a screen reader reads aloud, so these elements " +
          "are unusable without sight and identical to each other. Announced: $what")
      }
    },
    @{
      Name = 'every mailbox row announces its sender, subject and date'
      Body = {
        $rows = @(Get-MailRows)
        Assert-GreaterThan 0 $rows.Count 'the showcase seed puts messages in the list'
        foreach ($row in $rows) {
          $spoken = $row.Current.Name
          # The row's own visible text, which is what the announcement must contain. Scoped to the
          # row (uia.ps1 trap 2) and typed to Text so the sidebar and reading pane cannot supply it.
          $texts = @(Get-UiaTree $row |
            Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text } |
            ForEach-Object { $_.Current.Name } |
            Where-Object { $_ })
          Assert-GreaterThan 0 $texts.Count 'a seeded row draws at least a subject'
          foreach ($text in $texts) {
            # The count badge on a conversation row ("3") is chrome, not identity, the spoken name
            # carries sender/subject/date, and a bare numeral would match anything.
            if ($text -match '^\d+$') { continue }
            Assert-True ($spoken -like "*$text*") (
              "the row shows '$text' but announces '$spoken', a screen reader must hear what the " +
              'row says, or the two surfaces are describing different messages')
          }
        }
      }
    },
    @{
      Name = 'the avatar beside a sender is not announced'
      Body = {
        # docs/avatars.md: the circle is decoration. The row already announces the sender's name,
        # and the monogram is its first letters restated, announced, Narrator reads a letter
        # before every message in the mailbox. Nothing about the rendered row shows the defect,
        # and the control is a Grid, so only its Raw flag keeps the TextBlock inside it quiet.
        #
        # A monogram is one or two letters, all capitals. No seeded subject, sender or date is,
        # and a conversation's count badge is digits, so anything matching inside a row is the
        # avatar, exposed.
        $rows = @(Get-MailRows)
        Assert-GreaterThan 0 $rows.Count 'the showcase seed puts messages in the list'
        $spoken = @(foreach ($row in $rows) {
          Get-UiaTree $row |
            Where-Object {
              $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and
              $_.Current.Name -cmatch '^[A-Z]{1,2}$'
            } | ForEach-Object { $_.Current.Name }
        })
        Assert-Equal 0 $spoken.Count (
          'a monogram reaching the accessibility tree makes a screen reader read initials before ' +
          "every sender's name. Announced: $($spoken -join ', ')")
      }
    },
    @{
      Name = 'every button carries a spoken name'
      Body = {
        $unnamed = @(Get-UiaTree |
          Where-Object {
            $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
            -not $_.Current.Name
          })
        $where = ($unnamed | ForEach-Object {
            $r = $_.Current.BoundingRectangle
            "($([int]$r.X),$([int]$r.Y)) $([int]$r.Width)x$([int]$r.Height)"
          }) -join ' '
        Assert-Equal 0 $unnamed.Count (
          'an unnamed Button announces only "button", a WinUI Button derives a Name from string ' +
          'Content but NOT from an icon+label panel, so any such button needs an explicit ' +
          "AutomationProperties.Name. Unnamed at: $where")
      }
    },
    @{
      Name = 'every settings picker carries a spoken name'
      Body = {
        # A ComboBox derives no Name from a sibling heading, neither the TextBlock an account card
        # draws above it nor the one Group() draws, because neither is a relation the accessibility
        # tree carries. Seven pickers were silent when this case was written: language, time zone,
        # default send account, and both the fetch-depth and message-size pickers on each account
        # card. Each announces only "combo box" plus its current value, so the one thing a screen
        # reader cannot recover is WHICH setting is being changed.
        #
        # Swept per category, because the detail panel builds only the open one.
        $dialog = Get-SettingsDialog
        $categories = @(Find-UiaElements -Type 'ListItem' -Root $dialog | ForEach-Object { $_.Current.Name })
        Assert-GreaterThan 0 $categories.Count 'the Settings dialog lists its categories'
        $unnamed = @()
        foreach ($category in $categories) {
          $panel = Open-SettingsCategory $category
          $unnamed += @(Find-UiaElements -Type 'ComboBox' -Root $panel |
            Where-Object { -not $_.Current.Name } |
            ForEach-Object { $category })
        }
        Assert-Equal 0 $unnamed.Count (
          'a picker with no name announces only its value, so a screen-reader user hears what the ' +
          'setting is set TO but never what it IS. Give it AutomationProperties.Name (the catalog ' +
          "field label) or a ComboBox Header. Unnamed under: $($unnamed -join ', ')")
      }
    },
    @{
      # Last, because it navigates away from the mail list the cases above assert on.
      Name = 'every agenda row announces its event, not its class'
      Body = {
        $agenda = Show-Agenda
        $rows = @(Find-UiaElements -Type 'ListItem' -Root $agenda)
        Assert-GreaterThan 0 $rows.Count 'the showcase seed puts events in the calendar'
        foreach ($row in $rows) {
          $spoken = $row.Current.Name
          Assert-True ($spoken -notmatch $TypeNamePattern) (
            "an agenda row announces '$spoken', a class name is what a screen reader reads aloud, " +
            'so every event in the day sounds identical and none of them says when it is')
          $texts = @(Get-UiaTree $row |
            Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text } |
            ForEach-Object { $_.Current.Name } |
            Where-Object { $_ })
          Assert-GreaterThan 0 $texts.Count 'a seeded event draws at least a title'
          foreach ($text in $texts) {
            Assert-True ($spoken -like "*$text*") (
              "the row shows '$text' but announces '$spoken', a screen reader must hear what the " +
              'row says, or the two surfaces are describing different events')
          }
        }
      }
    }
  )
}
