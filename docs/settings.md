# Settings taxonomy: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client organises its Settings surface: which
categories exist, in which order, under which names, and what lives in each. The point is
**support**: an answer like "Settings → Reading → Swipe actions" must be true on every platform,
so a helper never has to ask "which device are you on?" before naming a path. Presentation is
per-platform (a desktop shows a sidebar beside a detail panel; a phone shows a hub of category
rows, each opening its own screen); the **taxonomy is not**.

**Principle.** One taxonomy, every client. A setting's category is decided **once, here**, never
per client. Adding a category, renaming one, or moving a setting between categories is a change to
this contract and lands on **every** platform in the same change (or under Known gaps, never
silently).

## The categories, in order

The names are the shared catalog's `settings_category_*` keys (`messages/<locale>.json`), so they
cannot drift per client. The order below is the display order everywhere.

| # | Category | What lives in it | Notes |
|---|---|---|---|
| 1 | **Allodia account** | Sign in · Create an account · the plan the account is on · Manage account · Sign out | Present **only** in a build carrying the registration, on the same `allodia_sign_in_available()` question the setup screen asks ([`onboarding.md`](onboarding.md)); a build from source has no such category and General is first. It is not a mail account and never appears among them: no mailbox, no switcher entry, and its token cannot touch mail. First because it is the reader's own account with us, which is where every platform's own settings put it, and because what it will carry next is the plan. **Manage account** opens the service's own page in the same in-app browser tab the sign-in uses, so it opens already signed in; **Sign out** erases the local grant first and ends the session second, and forgets what this device had synced: the record ids belong to the account that is leaving, and a device that carries them into the next sign-in claims nothing and is offered back the mail accounts it is already running. The person's own per-account choices (on / paused / off) are not part of that and stay. Full contract: [`entitlement.md`](../allodia_license/entitlement.md). |
| 2 | **General** | Language · Appearance (light/dark) · Time zone · Time format (12/24-hour) · Default mail app | In that order. Both the appearance and the clock sit here, **not** under Calendar or Reading: each spans the whole app, and one app must not disagree with itself. Appearance defaults to following the host and keeps following it live; Light and Dark are an explicit override. **Default mail app** is last, and is the permanent way back from the one-time offer: it appears **only where the build can act on it**, so it is absent on Linux and Android, where nothing in the platform can be asked, and in a sandboxed Mac App Store build, where the call is refused. Full contract: [`os-integration.md`](os-integration.md). |
| 3 | **Calendar** | First day of the week · Default zoom (horizon) · Default calendar | All three persisted in the core (see [`calendar.md`](calendar.md)). The default calendar lists only calendars that can be **written** to, and the core resolves the stored choice against what exists, so a client shows `is_default` rather than keeping a fallback rule. |
| 4 | **Reading** | Conversation grouping · Swipe actions | Swipe actions configure the mail list, which is read-side, even though clients also honour them elsewhere. |
| 5 | **Composing** | Quote style (+ per-message opt-in) · Default send account | |
| 6 | **Signatures** | The signature library (write / edit / delete) · per-account **For new messages** and **For replies or forwards** | Its own category, not a sub-screen of Composing: a signature is a standalone entity reused across accounts, and "Settings → Signatures" is the path people already look for (Outlook's arrangement). Library first, then the per-account defaults: an account picker with nothing to pick says nothing. Full contract: [`signatures.md`](signatures.md). |
| 7 | **Notifications** | New-mail notification toggle · (Android) battery-exemption card | Present where local new-mail notifications exist: Android, iOS/iPadOS and Linux. macOS and Windows retain this slot for when their notification host adapters land ([`background-sync.md`](background-sync.md) known gap). The category appears only where the feature exists; it keeps this slot when a platform gains it. |
| 8 | **Privacy** | Usage-statistics opt-in / withdrawal | The consent surface of [`analytics.md`](analytics.md); withdrawal is one tap (GDPR Art. 7(3)). |
| 9 | **Accounts** | **What the person's other devices hold** (offers to set up, accounts changed or removed elsewhere) · **how each account is shared with the other devices (on / paused / off)** · expired password/API-token replacement · per-account fetch depth · per-account message size · push/poll strategy · watched folders · poll interval · **Remove account** (destructive, confirmed) | Mail accounts only: the Allodia account is category 1 and is not one of these. What the other devices hold sits **above** the per-account cards and is drawn only once a sync pass has run and found something: before that there is nothing on screen at all, because a heading with an empty list under it claims the other devices hold no accounts, which this device does not yet know ([`onboarding.md`](onboarding.md) is the first-run half of the same feature). Each account's own card opens with **one three-position control** (on, paused, off), first because it decides whether anything below it is anybody else's business. It is one control and not a switch beside a button: the two questions underneath it (*is this account on my other devices*, *does this device exchange changes about it*) are not independent in any way a person can act on, and splitting them shipped a screen where turning the switch off changed nothing anybody could see. **Off is the only position that reaches the other devices**: it removes the record, so they are asked whether to drop the account too; paused is this device's business alone. **Every route that adds an account runs a pass**, the OAuth ones included: they reach the core by their own path rather than through the manual add, and a route that skips the pass leaves the account on that device until the next launch *and* draws its card with no sharing control, because the control is read in the same call. None of the three touches a mailbox or any mail, and each platform draws it with its own single-choice control (segmented on Apple and Android, `RadioButtons` on Windows, a linked toggle box on Linux). The credential field appears only while a stored-secret account needs repair; a valid credential stays out of ordinary settings. One card per account; the remove sits **last** in the card, below the things you might adjust. Fetch depth and message size sit together at the top of the card: both answer "how much of this account do I keep on this device", one in time and one in bytes, and both default per **form factor** rather than per product: a phone caps the warm at 2 MB where a computer keeps every size. Raising the size cap downloads what the lower one skipped; lowering it forgets those cached copies but never the mail, so the list and body search are unchanged. It is a *second* route, not the only one: the mailbox sidebar's per-account context menu keeps its own. A context menu cannot be the only copy: on a touch device it is a long press on a row that gives no sign it holds one, which is why App Review asks where the control is. |
| 10 | **Advanced** | Reset local database (destructive, confirmed) · AI assistant access (MCP server) | The MCP panel is **desktop-only**, the mirror of Notifications being mobile-only: mobile OSes suspend the app, so a server that is asleep when a client connects is worse than none ([`mcp.md`](mcp.md)). The category keeps this slot everywhere. Both entries share the category's *character* (powerful, expert-facing, off by default, capable of damage), which is what the Advanced row is for, rather than the one line it used to describe. |
| 11 | **Diagnostics** | Diagnostic log viewer · share / export · log size + copy path · DEBUG-detail toggle | The in-app view of the rotating file log ([`logging.md`](logging.md)): privacy-safe (counts / ids / durations, never content). On Android the hub row opens a full-screen log viewer rather than an inline detail. |
| 12 | **About** | The app version · the support forum · attributions | The one category that holds no setting: it is what a support answer needs quoted back at it. Its content is the **core's** (`about_info`), not each client's: a version that differs per platform is worse than none, and an attribution shown on one client and not another is a notice we have not given. Each client passes its own platform so the toolkit it actually links is named. Last, because it is the thing you go looking for rather than adjust. |

## Per-platform presentation

| Platform | Presentation | Where |
|---|---|---|
| macOS | Sidebar (category list + icons) beside a detail panel, in a sheet | `clients/apple/.../SettingsView.swift` |
| Windows | Source-list beside a detail panel, in a `ContentDialog` | `clients/windows/Mailcal/Dialogs/SettingsDialog*.cs` |
| Android | **Hub-and-spoke**: a list of category rows (icon · name · one-line summary), each opening its own screen; back (arrow and system) steps detail → hub → mailbox | `clients/android/.../SettingsCategory.kt` + `SettingsScreen.kt` |
| iOS / iPadOS | **iPad**: two-pane split (category list beside a detail panel, like macOS). **iPhone**: hub-and-spoke (category rows with a one-line summary, each pushing its own screen), matching Android. Presented full-screen (like the composer) so the iPad split has a regular-width container: a form sheet is compact and collapses it to the iPhone hub | `clients/apple/.../SettingsCategory.swift` + `SettingsCategoryDetail.swift` + `SettingsHubView.swift` |
| Linux | Sidebar beside a detail panel, in a modal window | `clients/linux/src/ui/settings.rs` + `clients/linux/src/ui/settings/` |

Category icons are the platform's native set (SF Symbols on Apple, Material on Android; the
brand's Lucide is for web/design assets, not platform chrome), matched by meaning:
gear/calendar/envelope/pencil/bell/hand-or-lock/person/wrench/stethoscope/info.

The Android hub rows also carry a one-line **summary** per category (the
`settings_category_*_summary` catalog keys) so a first-time user can find a setting without
opening every category; desktops don't need them (the sidebar and detail are visible at once), but
if another platform grows summaries it uses the same keys.

## Known gaps

- **Nothing presses the account category's browser hand-off.** Manage, Delete and the end-session
  hop each open a URL in the system browser, and no suite on any platform follows it: Windows
  ([`SettingsAllodia.Tests.ps1`](../clients/windows/uitests/SettingsAllodia.Tests.ps1), 2026-08-27)
  asserts the category's place, both signed-out routes and that Accounts no longer carries the card
  against the running client, and the Linux widget suite asserts each button reaches its own input.
  Both stop at the launch. What the pages themselves show is unverified from here.

- **Expired stored-secret replacement is wired on Linux only, and runtime-unverified there.** The
  Apple, Windows, and Android prompt says to use Settings, but Accounts exposes no credential editor,
  so their only remedy remains removing and re-adding the account. The shared
  `replace_account_secret` operation already preserves the account and validates before persistence;
  the missing work is client UI.

- **Two surfaces stay light whatever the appearance says: the composer editor and a message's own
  body.** Both are HTML in a web view: the shared editor bundle
  ([`composer-security.md`](composer-security.md)) and the sanitised message
  ([`rendering-security.md`](rendering-security.md)), and neither carries a dark palette, so in a
  dark app they read as a white panel inside it. This predates the setting (it was already true on a
  dark desktop), and the message body is the harder half: restyling a sender's mail is a rendering
  decision, not a theming one. It applies on every platform. The two halves separate: the composer
  editor is our own HTML and the easier one.

  The store set now shows this: its dark capture is the mailbox list
  ([`store-screenshots.md`](store-screenshots.md)), and on the
  three-pane clients that screen includes the reading pane. Phones have none, so their frame is dark
  throughout.

- **Android's Diagnostics row changed icon.** It borrowed the "info" symbol, which About now
  carries; Diagnostics uses Material's "troubleshoot". Apple already distinguished the two
  (stethoscope / info circle) and Windows draws no category icons.
- **"Remove account" in the Accounts category is missing on Android and Windows**: Apple (macOS +
  iOS/iPadOS, which share one target) and Linux both offer it there. Android and Windows still
  offer removal *only* from the mailbox sidebar's
  per-account context menu, which works, and whose confirmation already carries a Cancel on both
  (Android's `AlertDialog` has a `dismissButton`, Windows' `ContentDialog` a `CloseButtonText`), so
  neither has the defect this entry was added for. What they lack is the discoverable second route.
  It is client wiring on both: the core call (`removeAccount`) and the confirmation copy
  (`remove_account_title` / `remove_account_message`) are already shared and already used.

- **Linux's Accounts category opens with an "Add account" button**, above the per-account cards. No
  other client puts one there: Apple's and Windows' live in the mailbox sidebar, Android's on the
  mailbox screen. A Linux-only extra, not a taxonomy change: nothing has decided whether the others
  follow.

- **The Calendar category's default calendar ships on Android and Linux.** The setting itself is in
  the core (`Preferences::default_calendar`, resolved onto `CalendarRow::is_default`), so Apple and
  Windows need client wiring only: list the writable calendars **grouped by account** (an
  account id is `address@provider-host`, so it belongs above a group, not beside every row), show
  the row the core marked, and call `set_default_calendar`. Until they do, those clients keep filing
  new events on the first writable calendar, which is what `is_default` already resolves to when
  nobody has chosen.
- **A shortcut into a category ships on Android only.** The calendar's overflow menu offers
  **Calendar settings**, which opens Settings on the Calendar category rather than the hub: the
  settings governing that screen are otherwise reached by leaving it. Back still unwinds through the
  hub, so arriving deep does not make the first press leave Settings. The other clients reach the
  category through the hub only.

- **macOS and Windows omit Notifications.** Android, iOS/iPadOS, and Linux show it; Linux posts
  through the desktop portal. The two shipped desktops leave that slot out, blocked on their
  notification host adapters ([`background-sync.md`](background-sync.md)); the category keeps its
  position for when they gain it.
- **The Signatures category is on every platform.** Its body editor renders where each toolkit
  allows: **inside** the Settings dialog's detail panel on Windows (WinUI forbids a nested
  `ContentDialog`, the same reason the destructive database reset confirms in place), and in a
  window or screen of its own everywhere else.
- **Notification *content* is still English-only.** The settings copy is catalog-backed on Android
  and iOS, and both settings and notification copy are catalog-backed on Linux. The Android
  notification channel name and notification texts ("N new messages", "+N more") remain hardcoded:
  they render in a background context where the per-app locale needs separate handling
  ([`background-sync.md`](background-sync.md) known gap).

## Enforcement

When you add, rename, move, or re-order a setting or category:

1. Update the table above **in the same change**, and apply the change to **every** platform that
   has the surface (or record the shortfall under Known gaps, never silently).
2. Category names and Android summaries come from the shared catalog (`settings_category_*`);
   never hardcode one in a client.
3. The Android JVM suite pins the taxonomy and its order
   (`clients/android/app/src/test/.../SettingsHubTest.kt`); update it deliberately, with this doc.
