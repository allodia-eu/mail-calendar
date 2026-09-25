# Icons: cross-platform contract

**Scope.** Every glyph a client draws in its own chrome: which one each platform uses for a meaning,
and which icon set it comes from. The app icon and brand art are [`branding.md`](branding.md);
a person's avatar is [`avatars.md`](avatars.md).

**Principle.** A meaning is decided once; the artwork is each platform's own. A Lucide glyph beside
Segoe Fluent in a WinUI pane, or beside SF Symbols in a macOS sidebar, reads as a bug, so the
contract fixes semantic parity, never pixel parity
([`folder-pane.md`](folder-pane.md), rule 10).

## Rules

1. **Where a glyph comes from, in order:** the platform's own set; then the set that platform
   itself sanctions for what its own set lacks; then a drawing of our own on that platform's grid.
   Lucide is the brand's set for the web and design assets and is **never** drawn in platform
   chrome.

   | Platform | Its own set | The set it sanctions | Where the names live |
   |---|---|---|---|
   | Apple | SF Symbols, from the OS | none needed so far | string literals in `MailcalUI` |
   | Android | Material Symbols, copied into vector drawables (Apache-2.0) | none needed so far | `app/src/main/res/drawable/ic_*.xml` |
   | Windows | Segoe Fluent Icons through `SymbolThemeFontFamily`, and WinUI's `Symbol` | [Fluent UI System Icons](https://github.com/microsoft/fluentui-system-icons) (MIT); none shipped yet | code points in XAML and C# |
   | Linux | Adwaita, from the GNOME runtime | GNOME's [icon-development-kit](https://gitlab.gnome.org/Teams/Design/icon-development-kit) (CC0-1.0) | [`icons.rs`](../clients/linux/src/ui/icons.rs), and nowhere else |

2. **A vendored glyph is copied verbatim and keeps its own licence.** It is named in
   [`REUSE.toml`](../REUSE.toml) under that licence, so the GPL blanket cannot relabel it, and it
   says where it came from: an Android drawable in its header comment, an icon-development-kit
   file by the commit [`vendor-idk-icons.sh`](../scripts/dev/vendor-idk-icons.sh) pins.

3. **One glyph per meaning within a platform, and one meaning per navigation glyph.** Two Settings
   categories never share a glyph: a sidebar's icons are for finding a row without reading it.

4. **Linux draws every name from one registry, asked of Adwaita.** A name GTK cannot find is not an
   error anywhere in GTK: the widget draws the broken-image glyph and carries on. So every name is
   a constant in `icons.rs`, whose test asks **Adwaita** for each one. It never asks the display's
   theme, because that is whatever the desktop running the test chose, and Ubuntu's Yaru ships
   names Adwaita does not. A glyph Adwaita lacks is bundled and served as `mailcal-…-symbolic`, a
   name no theme ships, so it draws the same on every distribution.

5. **The platform's rendering decides the format, not the set's own.**
   - GTK recolours a symbolic icon by class, not by `fill` or `stroke`: a shape without
     `class="foreground-stroke"` is filled, so a raw stroke-drawn SVG (Lucide, Tabler) renders as a
     solid blob. The icon-development-kit's files carry the classes and draw as they are on the
     GTK 4.22 floor.
   - Adwaita's `legacy/` names are not used: the theme keeps them for older applications, outside
     its current set. Nor is a status glyph drawn at reduced opacity (Adwaita's `mail-read`, which
     means *already read*) used where it would read as a disabled row.
   - WinUI's `ImageIcon` ignores `Foreground`, so an SVG there never follows the theme. A Fluent UI
     System Icons glyph goes in as its font through `FontIcon`, or as path data through `PathIcon`.

## The mapping

A cell names what that platform draws, a lone dash that it draws nothing there, and a glyph the
platform's framework draws (a `NavigationView` chevron, a search box's own icon) is described in
words. On Linux, a `mailcal-` name is bundled: the icon-development-kit file it is served from is
noted beside it, and the two without one are this app's own drawings.

### Folder pane

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Inbox, and All Inboxes | `tray` | `ic_inbox` | `E715` Mail | `mailcal-inbox-symbolic` (own) |
| Drafts | `square.and.pencil` | `ic_edit` | `E70F` Edit | `document-edit-symbolic` |
| Sent | `paperplane` | `ic_send` | `E724` Send | `mail-send-symbolic` |
| Archive | `archivebox` | `ic_archive` | `E7B8` Package | `mailcal-archive-symbolic` (own) |
| Junk | `xmark.bin` | `ic_warning` | `E733` Blocked | `mail-mark-junk-symbolic` |
| Trash | `trash` | `ic_delete` | `E74D` Delete | `user-trash-symbolic` |
| Any other folder | `folder` | `ic_folder` | `E8B7` Folder | `folder-symbolic` |
| All Mail | `tray.full` | — | — | — |
| An account | `person.crop.circle` | — | `E77B` Contact | `avatar-default-symbolic` |
| An account whose server is unreachable | `exclamationmark.triangle.fill` | `ic_warning` | `Symbol.Important` | `dialog-warning-symbolic` |
| Outbox | `tray.and.arrow.up` | `ic_outbox` | `E898` Upload | `document-send-symbolic` |
| Add an account | `plus.circle` | — | `E710` Add | `list-add-symbolic` |
| Open / closed tree | `chevron.down` / `chevron.right` | `ic_keyboard_arrow_down` / `ic_keyboard_arrow_right` | the `NavigationView`'s own | `pan-down-symbolic` / `pan-end-symbolic` |

### Navigation

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Mail | `tray.full` | `ic_mail` | the `NavigationView`'s own | `mail-unread-symbolic` |
| Calendar | `calendar` | `ic_calendar_month` | `E787` Calendar | `x-office-calendar-symbolic` |
| Contacts | `person.2` | `ic_contacts` | `E716` People | `x-office-address-book-symbolic` |
| Settings | `gearshape` | `ic_settings` | the `NavigationView`'s own | `preferences-system-symbolic` |
| New message | `square.and.pencil` | `ic_edit` | `E70F` Edit | `mail-message-new-symbolic` |
| Search | `magnifyingglass` | `ic_search` | the search box's own | — |
| Clear the search | `xmark.circle.fill` | `ic_close` | the search box's own | — |
| Show the folders (phone) | `line.3.horizontal` | `ic_menu` | — | — |
| Back | the system's own | `ic_arrow_back` | — | — |
| Jump to newly arrived mail | — | `ic_keyboard_arrow_up` | — | — |

### Reading

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Reply | `arrowshape.turn.up.left` | `ic_reply` | `E8CA` MailReply | `mail-reply-sender-symbolic` |
| Reply all | `arrowshape.turn.up.left.2` | `ic_reply_all` | `E8C2` MailReplyAll | `mail-reply-all-symbolic` |
| Forward | `arrowshape.turn.up.right` | `ic_forward` | `E72A` Forward | `mail-forward-symbolic` |
| Archive | `archivebox` | `ic_archive` | `E7B8` Package | `mailcal-archive-symbolic` |
| Move to Trash | `trash` | `ic_delete` | `E74D` Delete | `user-trash-symbolic` |
| More actions | `ellipsis` | `ic_more_vert` | `E712` More | `view-more-symbolic` |
| Save as a file | `square.and.arrow.down.on.square` | — | — | — |
| Open an attachment | `arrow.up.forward.app` | — | — | — |
| Save an attachment | `square.and.arrow.down` | — | — | — |
| The body failed to load | `exclamationmark.triangle` | — | `E7BA` Warning | — |
| Remote images are blocked | `photo` | — | — | — |
| Nothing selected | `envelope.open` | — | `E715` Mail | — |
| Several messages selected | `envelope.badge.fill` | — | `E8A9` ViewAll | `mail-unread-symbolic` |

### The mail list and its selection

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Flagged | `flag.fill` | `ic_star` | `E7C1` Flag | `starred-symbolic` |
| Has an attachment | `paperclip` | `ic_attachment` | `E723` Attach | `mail-attachment-symbolic` |
| Conversation open / closed | `chevron.down` / `chevron.right` | — | `E70D` ChevronDown / `E76C` ChevronRight | the expander row's own |
| A row's actions | a context menu | `ic_more_vert` | a context menu | `view-more-symbolic` |
| Open | `envelope.open` | — | — | `go-next-symbolic` |
| Open in a window | `macwindow.on.rectangle` | — | — | — |
| Mark read | `envelope.open` | — | `E715` Mail | `mail-read-symbolic` |
| Mark unread | `envelope.badge`, `envelope` in the row menu | — | `E715` Mail | `mail-unread-symbolic` |
| Flag | `flag` | — | `E7C1` Flag | `starred-symbolic` |
| Remove the flag | `flag.slash` | — | `E7C1` Flag | `non-starred-symbolic` |
| Delete permanently | `trash.slash` | — | `E74D` Delete, in the critical colour | `edit-delete-symbolic` |
| Select all | `checklist` | — | `E8B3` SelectAll | `edit-select-all-symbolic` |
| Clear the selection | `xmark` | `ic_close` | `E711` Cancel | `window-close-symbolic` |
| Sync | `arrow.clockwise` | — | `E72C` Refresh | `view-refresh-symbolic` |
| Swipe: archive | `archivebox` | `ic_archive` | `Symbol.MoveToFolder` | — |
| Swipe: star | `star` | `ic_star` | `Symbol.Favorite` | — |
| Swipe: delete | `trash` | `ic_delete` | `Symbol.Delete` | — |

### Why a list is empty

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| The folder holds no mail | `tray` | — | — | `mailcal-inbox-symbolic` |
| Its mail is outside the sync depth | `clock.arrow.circlepath` | — | — | `document-open-recent-symbolic` |
| The Outbox is empty | `tray.and.arrow.up` | — | — | — |

### Outbox

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Waiting | `clock` | — | `E823` Recent | `document-open-recent-symbolic` |
| Sending | `arrow.up.circle` | — | `E724` Send | `mail-send-symbolic` |
| Unconfirmed | `exclamationmark.triangle` | — | `E7BA` Warning | `dialog-warning-symbolic` |
| A queued message's actions | — | `ic_more_vert` | — | `view-more-symbolic` |

### Composer

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Send | `paperplane` | `ic_send` | text | text |
| Discard | `trash` | `ic_close` | text | text |
| Attach | `paperclip` | `ic_attachment` | text | text |
| Choose a signature | `signature` | `ic_signature` | text | text |
| The chosen signature | — | `ic_check` | — | — |
| Show / hide Cc and Bcc | `chevron.down`, turned | `ic_keyboard_arrow_down`, turned | `E70D` ChevronDown / `E70E` ChevronUp | `pan-down-symbolic` / `pan-up-symbolic` |
| Remove an attachment | `xmark.circle` | — | — | `user-trash-symbolic` |
| Remove a recipient | `xmark.circle.fill` | `ic_close` | `E711` Cancel | `window-close-symbolic` |

### Writing style and drafted replies

The contract is [`ai.md`](ai.md).

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Draft a reply | `text.quote` | `ic_stylus_note` | text | text |
| A draft that could not be made | `exclamationmark.circle` | — | — | — |
| A style in the library, and opening it | `text.quote`, `chevron.right` | — | — | `go-next-symbolic` |
| A placeholder to fill in | `character.cursor.ibeam` | `ic_text_fields` | `E70F` Edit | `document-edit-symbolic` |
| A file to attach | `paperclip` | `ic_attachment` | `E723` Attach | `mail-attachment-symbolic` |
| Something to do elsewhere | `checklist` | `ic_task_alt` | `E9D5` CheckList | `view-list-bullet-symbolic` |
| An item done / not done | `checkmark.square.fill` / `square` | the framework's checkbox | the framework's checkbox | the framework's check button |
| The reveal: register | `slider.horizontal.3` | `ic_tune` | `E9E9` Equalizer | — |
| The reveal: structure | `arrowshape.turn.up.left` | `ic_reply` | `E8CA` MailReply | — |
| The reveal: how they decline and chase | `bubble.left.and.bubble.right` | `ic_forum` | `E8F2` ChatBubbles | — |
| Rate a draft: up / down, filled once chosen | `hand.thumbsup` / `hand.thumbsdown` | — | — | — |

### Calendar

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Previous / next week | `chevron.left` / `chevron.right` | — | `E76B` ChevronLeft / `E76C` ChevronRight | `go-previous-symbolic` / `go-next-symbolic` |
| New event | `calendar.badge.plus` | `ic_add` | `E787` Calendar | — |
| The view menu | `ellipsis.circle` | `ic_more_vert` | — | — |
| Manage calendars | — | — | `E8FD` BulletedList | — |
| The chosen mode or calendar | `checkmark` | `ic_check` | — | — |
| An event in the agenda | `calendar` | `ic_calendar_month` | `E787` Calendar | — |
| Open an event | — | — | — | `go-next-symbolic` |
| Edit an event | `pencil` | `ic_edit` | — | — |
| Delete an event | `trash` | `ic_delete` | `E74D` Delete | `user-trash-symbolic` |
| A write was saved | `checkmark.circle.fill` | `ic_check` | `E73E` CheckMark | — |
| A write is unconfirmed | `exclamationmark.triangle.fill` | `ic_warning` | `E7BA` Warning | — |
| Repeat count down / up | — | `ic_keyboard_arrow_down` / `ic_keyboard_arrow_up` | — | — |
| Choose a time zone | — | `ic_arrow_drop_down` | — | — |

### Invitations

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Asks for a reply | `calendar.badge.clock` | `ic_calendar_month` | `E787` Calendar | — |
| Cancelled | `calendar.badge.minus` | `ic_calendar_month` | `E711` Cancel | — |
| For information | `calendar` | `ic_calendar_month` | `E946` Info | — |
| Superseded | `calendar.badge.exclamationmark` | `ic_calendar_month` | `E7BA` Warning | — |
| Repeats | `repeat` | — | — | — |
| Answered: accepted | `checkmark.circle` | — | — | — |
| Answered: declined | `xmark.circle` | — | — | — |
| Answered: tentative | `questionmark.circle` | — | — | — |
| Answered: delegated | `arrowshape.turn.up.right.circle` | — | — | — |
| Not answered yet | `clock` | — | — | — |
| The reply is being sent | `arrow.triangle.2.circlepath` | — | — | — |
| The reply failed | `exclamationmark.triangle` | — | — | — |

### Contacts

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Search | `magnifyingglass` | `ic_search` | — | — |
| Clear the search | `xmark.circle.fill` | `ic_close` | `E711` Cancel | — |
| New contact | `plus` | `ic_add` | `E710` Add | `list-add-symbolic` |
| Refresh the address books | — | — | — | `view-refresh-symbolic` |
| Remove a value | `minus.circle.fill` | `ic_close` | `E74D` Delete | `user-trash-symbolic` |
| Nobody picked, and a person with no photo, name or address | `person.crop.circle` | `ic_account_circle` | `E77B` Contact | `avatar-default-symbolic` |
| No contacts | — | — | — | `x-office-address-book-symbolic` |

### Settings categories

The order and names are [`settings.md`](settings.md). Windows draws none (Known gaps).

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Allodia account | `person.crop.circle` | `ic_account_circle` | — | `avatar-default-symbolic` |
| General | `gearshape` | `ic_settings` | — | `mailcal-cogged-wheel-symbolic` (`cogged-wheel`) |
| Calendar | `calendar` | `ic_calendar_month` | — | `x-office-calendar-symbolic` |
| Reading | `envelope.open` | `ic_mail` | — | `mailcal-mail-read-symbolic` (`mail-read`) |
| Composing | `square.and.pencil` | `ic_edit` | — | `document-edit-symbolic` |
| Signatures | `signature` | `ic_signature` | — | `mailcal-signature-symbolic` (`signature`) |
| Writing style | `text.quote` | `ic_stylus_note` | — | `format-text-rich-symbolic` |
| Notifications | `bell` | `ic_notifications` | — | `mailcal-bell-symbolic` (`bell`) |
| Privacy | `hand.raised` | `ic_lock` | — | `channel-secure-symbolic` |
| Accounts | `tray.2` | `ic_inbox` | — | `mailcal-inbox-symbolic` |
| Advanced | `wrench.and.screwdriver` | `ic_build` | — | `mailcal-wrench-symbolic` (`wrench`) |
| Diagnostics | `stethoscope` | `ic_troubleshoot` | — | `mailcal-stethoscope-symbolic` (`stethoscope`) |
| About | `info.circle` | `ic_info` | — | `help-about-symbolic` |

### Inside Settings and setup

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Open a category (phone) | — | `ic_keyboard_arrow_right` | — | — |
| Open a list of choices | — | `ic_arrow_drop_down` | — | — |
| Add a signature | `plus` | `ic_add` | — | — |
| A signature in the library | `signature` | — | — | — |
| Delete a signature, or remove an account | `trash` | `ic_delete` | — | — |
| Insert an image into a signature | `photo` | `ic_image` | — | — |
| Save / cancel the signature editor | — | `ic_check` / `ic_close` | — | — |
| Jump to the end of the log | `arrow.down.to.line` | — | — | — |
| The analytics preview, open / closed | `chevron.down` / `chevron.right` | — | — | — |
| Setup: the mail account | `envelope` | — | — | — |
| Setup: the calendar | `calendar` | — | — | — |
| Setup: advanced server settings | `slider.horizontal.3` | — | — | — |

### Banners and notifications

| Meaning | Apple | Android | Windows | Linux |
|---|---|---|---|---|
| Offline | `wifi.slash` | `ic_warning` | the `InfoBar`'s own | — |
| Sign in again, or grant what is missing | `exclamationmark.triangle.fill` | `ic_warning` | the `InfoBar`'s own | — |
| Not connected | — | `ic_warning` | `E7BA` Warning | — |
| Sent | `checkmark.circle.fill` | — | — | — |
| Queued in the Outbox | `clock.fill` | — | — | — |
| Sending failed | `exclamationmark.triangle.fill` | — | — | — |
| A Sent copy could not be filed | `exclamationmark.circle.fill` | — | — | — |
| New mail, in the status bar | the app icon | `ic_stat_mail` | the app icon | the app icon |

## Vendored sets

| Set | Where | Licence | How it moves |
|---|---|---|---|
| Material Symbols (and the older Material Icons for `ic_stat_mail`) | `clients/android/app/src/main/res/drawable/ic_*.xml` | Apache-2.0 | by hand, one drawable at a time; its header names the upstream icon and style |
| GNOME icon-development-kit | `clients/linux/icons/idk/` | CC0-1.0 | [`vendor-idk-icons.sh`](../scripts/dev/vendor-idk-icons.sh) at its pinned commit; the gresource aliases each file to its served name |
| This app's own | `clients/linux/icons/scalable/actions/` | GPL-3.0-only | drawn on Adwaita's 16 px grid |

## Enforcement

[`cargo xtask check-icons`](../xtask/src/icons.rs) holds the tables above to the tree: the Linux
column equals the registry, no icon name is written outside it, and the bundle serves exactly its
`mailcal-` names; the Android column equals the drawables on disk; the Windows column equals the
code points and `Symbol` values in the app. Apple's column is checked one way, that every symbol
it names is drawn somewhere, because an SF Symbol in Swift is a string like any other. The Linux
widget suite asks Adwaita for every name in the registry.

Adding or changing a glyph:

1. Take it from the set rule 1 names for that platform, matching the meaning the other platforms
   already draw here.
2. On Linux, add the constant to `icons.rs`. If Adwaita lacks it, look in the icon-development-kit
   (its Icon Library app, `org.gnome.design.IconLibrary`, browses it), fetch it with
   `vendor-idk-icons.sh <name>`, and serve it from the gresource as `mailcal-<name>-symbolic`.
3. Update the row here, in the same change.

## Known gaps

- **Windows' Settings draws no category icons.** Segoe Fluent covers most of the meanings, so this
  is wiring rather than a missing set.
- **A few meanings differ in kind, not only in artwork.** Android marks a flagged message with a
  star where the others draw a flag, and draws no glyph for an Outbox state or an invitation's
  answer, which Apple does.
- **Linux screenshots photograph the capturing desktop's icon theme.** `showcase.sh linux` runs the
  host build on the host's settings, so a capture taken on an Ubuntu desktop shows Yaru rather than
  the Adwaita the Flatpak draws.
- **Windows' `E7B8` is Segoe's "Package"**, drawn for archive because the set has no archive glyph
  of its own.
