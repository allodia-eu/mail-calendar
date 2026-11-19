# The calendar pane: cross-platform contract

**Scope.** The pane beside the calendar on a desktop (macOS, iPadOS at regular width, Windows,
Linux): the mini month that moves the grid, the list of calendars that shows, hides and colours
them, and the destinations pinned under both. A phone has no pane; its calendar manager stays, and
groups its calendars the way this pane does. The grid itself is [`calendar.md`](calendar.md)'s; the
pane beside mail and contacts is [`folder-pane.md`](folder-pane.md)'s.

**Principle.** The pane is the calendar's own furniture, as the folder tree is mail's: always on
screen, never a dialog. Everything it draws comes from one pull, `calendar_pane`, answered from the
in-memory calendar cache and the preferences; a client keeps only the month the person paged to.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **While the calendar is showing, the pane beside it is the calendar pane**, in the folder pane's column: the same splitter, persisted width, row height, type sizes, hover and selection treatment, in each platform's own controls and icons ([`icons.md`](icons.md)). Top to bottom: the mini month, the calendars, the destinations. Contacts keeps the folder tree. | A mail folder tree beside a calendar offers nothing the calendar can use, and a list of calendars kept in a dialog is one nobody opens. |
| 2 | **The mini month always shows six weeks**, headed by the seven weekdays in the person's week-start order (`MiniMonth::weekdays`, worded by the client) and led by a column of **ISO 8601** week numbers whatever the week start (`MiniWeek::number`). A Sunday-start row is numbered by the week its Monday falls in. Days outside the month are dimmed (`in_month`). | A pane whose height changes as it pages moves the list under it. ISO is the numbering a European calendar prints, and the grid's own week number (`GridDay::week`) is the same rule. |
| 3 | **Which month it shows is the core's.** With nothing paged: the month layout's own month; for a time grid, today's month while today is among the shown days and otherwise the month of the first shown day; for the agenda, today's. The arrows (`calendar_prev_month`, `calendar_next_month`) pass `previous` or `next` back as `CalendarPaneQuery::month`; the client clears it whenever the grid's shown days change, so the mini month snaps back to follow the grid. | The client does no date arithmetic, and the mini month never disagrees with the grid beside it for longer than the person asks it to. |
| 4 | **Today is a filled accent circle** (`is_today`), from the client's clock in the display zone. **The days the grid shows wear one soft accent band** (`shown`): `Start` and `End` round its ends within a row, so a band crossing a week boundary is two capsules; a single day is `Only`. The month layout and the agenda mark no days. | The band tells the person where the grid is without reading its header. |
| 5 | **Pressing a day moves the grid there** through `calendar_seat(date, layout)`, applied through the path the layout menu uses: Week and Work week keep their layout on the week holding the day, Day, 3 days and Month keep theirs at the day, and the agenda opens that day in Day. | Alignment stays a seat, never a side effect of a zoom ([`calendar.md`](calendar.md) §3), and every client lands in the same place. |
| 6 | **Calendars are listed under their account's address, in the core's account order** (`CalendarGroup`), never sorted by id; a shared store's calendars are a group of their own after the account that opens it ([`shared-mailboxes.md`](shared-mailboxes.md), rule 11). An account with no calendars has no group. The list takes the height the mini month and the destinations leave and scrolls on its own. | The grouping is decided once, so the pane, a phone's manager and a calendar picker (`calendar_groups`) agree. |
| 7 | **A group heading is drawn like the folder pane's account row**: heavier, with a disclosure chevron. Expansion lives in the core, is persisted per account id and changes with `PreferenceIntent::SetCalendarGroupExpanded`; a group nobody has touched is open, and removing the account forgets it. Under the heading, the account's notices mark it exactly as they mark its row in the folder tree ([`notices.md`](notices.md), rule 6). | The folder pane's rules 3 and 4, for the same reasons; and the pane replaces the tree, so without the mark an account's trouble would leave the screen with it. |
| 8 | **A calendar row is a round colour check, the name, and a lock** at the trailing edge when the calendar cannot be written (`can_write`, labelled `calendar_pane_read_only`). The check is filled with the calendar's colour and a tick when shown, a hollow ring in that colour when hidden. The whole row toggles, through `PreferenceIntent::SetCalendarVisible`, and redraws from the next pull with no copy of its own. | One target the size of the row, and one source of truth for what is shown. |
| 9 | **The row's context menu offers the colour** (`calendar_pane_color`, chosen from `calendar_palette`) **and the server's colour** (`calendar_color_reset`), through `PreferenceIntent::SetCalendarColor`. | The colour lives where the calendar is, not in a dialog. |
| 10 | **Mail, Calendar, Contacts, Settings are pinned under the pane**, in that order on every desktop, the current one highlighted ([`folder-pane.md`](folder-pane.md), rule 21). Mail returns to the mail view last open. | The way back to mail must be where the way to the calendar was. |
| 11 | **Accessibility.** Every mini-month day is a button whose spoken label is the full date, plus `calendar_today` and `a11y_calendar_pane_day_shown` where they apply; arrow keys move between days. A calendar row is a checkbox named `a11y_calendar_pane_calendar` (calendar and account address). The list is named `calendar_pane_calendars`. | The mini month is otherwise a grid of bare numbers. |
| 12 | **The pane is pulled, never pushed**: on `Surface::Calendar`, on `Surface::Settings` (the week start), and when the grid comes to rest on other days, never per frame of a scroll. | It reads the same cache as the grid, under [`calendar.md`](calendar.md) §5's discipline. |

Not part of the pane, so nobody adds them: event dots under mini-month days (an unmaterialised month
would show none, which reads as "nothing on"); "Add calendar" (the core cannot create one); a
tri-state check on the group heading; "show only this calendar".

## Where each rule lives

| Rule | Core | Test |
|---|---|---|
| 2, 3, 4, 6 | `mini_month`, `followed_month`, `groups` (`mailcal-viewmodel/src/calendar/pane.rs`) | `pane_tests.rs` |
| 2 (week numbers) | `iso_week` (`calendar/days.rs`), also behind `GridDay::week` | `days_tests.rs` |
| 5, 7 | `App::calendar_seat`, `App::set_calendar_group_expanded` (`mailcal-app/src/calendar_pane.rs`); `Preferences::calendar_group_expanded` | `calendar_pane_tests.rs`, `preferences/calendar_pane.rs` |
| FFI | `calendar_pane`, `calendar_seat`, `calendar_groups` (`app_calendar_pane.rs`) | `records_calendar_pane_tests.rs` |

## Per-platform

| Platform | Pane | Mini month | Calendars | Mail destination | Desktop manager removed | Notice mark on the heading (7) |
|---|---|---|---|---|---|---|
| macOS | ✅ `sidebarColumn` → `CalendarPane.swift` | ✅ `CalendarPaneMonth.swift` | ✅ `CalendarPaneList` | ✅ `sidebarDestinations` | ✅ no menu item or toolbar entry; `CalendarManagerView` is the phone's | ✅ `AccountNoticeLine`, shared with the folder tree |
| iPadOS (regular width) | ✅ the same pane, in the split view's sidebar column | ✅ the same | ✅ the same | ✅ the same bar | ✅ at regular width; compact width keeps the manager | ✅ the same |
| Windows | ✅ the `NavigationView`'s pane footer, sized to the column, the menu's scroller collapsed (`Views/CalendarPane.xaml`) | ✅ `CalendarPane.xaml`, its state in `ViewModels/CalendarPaneModel.cs` | ✅ toggled on `Checked`/`Unchecked`, so UI Automation's toggle works (`CalendarPane.Calendars.cs`) | ✅ `FooterItems`: Mail, Calendar, Contacts, then the Settings gear; in mail the lit row is the open folder's, a `NavigationView` having one selection | ✅ `CalendarManagerDialog` and its button deleted | ✅ `Views/AccountMarkLine`, shared with the folder tree's account row |
| Linux | ✅ a `gtk::Stack` in the sidebar column (`side_pane.rs`) | ✅ a `gtk::Grid` of flat buttons (`side_pane_month.rs`) | ✅ navigation-sidebar rows, the menu in `side_pane_menu.rs` (`side_pane_list.rs`) | ✅ `DestinationBar` | ✅ `manager.rs` and its header button deleted | ✅ `wear_mark`, shared with the folder tree's account row (`folder_pane_rows.rs`) |
| iOS (iPhone) | — | — | — | — | — | — |
| Android | — | — | — | — | — | — |

A phone has no pane: its calendar manager stays, grouped by `calendar_groups`.

## Enforcement

1. Ask the core; never group calendars, number weeks or page months in a client.
2. Keep only the paged month of your own, and clear it when the grid's shown days change.
3. Apply a seat through the layout menu's path, never by setting the anchor alone.
4. Apply a change to every desktop that ships the pane and update the matrix above,
   [`folder-pane.md`](folder-pane.md)'s and [`capabilities.md`](capabilities.md) in the same change.
