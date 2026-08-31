# Sender avatars: cross-platform contract

**Scope.** The small circle every Allodia Mail & Calendar client draws beside a person: on a
mail **list row**, in the **reading header**, on a **contacts row**, and on the contact that row
opens. It covers what the avatar is *of*, what is drawn, which colour it sits on, and where a
photograph may come from.

**Principle.** A mailbox of fifty rows is fifty identical envelope glyphs, and the eye has no
way to find a person without reading text. Every mainstream client answers that with a
coloured circle, and almost never with a photograph, because for most correspondents no
provider has one. **The monogram is the feature; the colour is what does the work.** A wall of
identical grey circles would be no better than the envelopes.

**Why so much of this is decided in the core.** Contrast is the reason, and it is the same one
[`calendar.md`](calendar.md) gives: resolved per client, four clients disagree about whether a
white letter is legible on a mid-green fill. The letters are here for a plainer reason: the
contacts list and the mail list deriving them separately would make the same person `AL` on one
screen and `A` on the other. What stays with the client is the **shape**: circle, rounded
square, diameter, and where it sits in the row. That is genuinely platform-native.

> This supersedes [`contacts.md`](contacts.md) §2's assignment of "layout, avatars, and the
> section-header treatment" wholly to the client. Layout and section headers stay there; the
> avatar's content and colour are decided here.

## What an avatar is of

**The canonical email address of the person the row names**: never the display name, never the
person id.

- **Not the name**, because two people share one, and they would then share an identity.
- **Not the person id**, because it does not exist before contacts sync, and a later merge
  would recolour a sender under the user while they were looking at it.

Every surface falls out of that single rule with no per-folder special case: a flat row names
its sender, a thread row the latest sender, a Sent row the sender too, a contacts row the
person. Canonicalization is the engine's `CanonicalEmail`.

## What is drawn

| | Rule |
|---|---|
| **Preference** | Photo, else monogram. **Never blank, never a silhouette.** |
| **Letters** | The first character of the first and last whitespace-separated words of the name, uppercased. `Ada Lovelace` → `AL`, `GitHub` → `G`, `The Google Workspace Team` → `TT`. Characters, not bytes. |
| **No name** | The address supplies the letter (`ada@example.test` → `A`). |
| **Neither** | Empty initials. The client draws **its own** platform person glyph: the core invents no placeholder, because any word it chose would be untranslatable English shown verbatim by every client. |
| **Colour** | A slot in the shared [`color`] palette, from a stable FNV-1a hash of the lowercased address. |
| **Accessibility** | **Hidden from assistive technology, everywhere.** |

The letters come from the same derivation the contacts list uses. Outlook renders "The Google
Workspace Team" as *TG* where this gives *TT*; agreeing with our own contacts list is worth more
than matching an undocumented rule of someone else's.

**The palette is the calendar's**, so the two surfaces read as one system rather than two
unrelated colour schemes, and the orange band stays reserved for actions.

**Never `DefaultHasher`.** Its output is not guaranteed across Rust releases, so a toolchain
bump would silently recolour every sender in the user's mailbox: perfectly consistent within
any one build, and wrong across two. The palette slots are pinned to literal values in a test
for exactly this reason.

**The address is lowercased for the colour only.** `CanonicalEmail` case-folds the domain and
keeps the local part exact, because two mailboxes differing in case may be two people. That is
right for identity and wrong for colour, where one person appearing in two colours depending on
how a header spelled them just looks broken.

**The avatar is decoration.** The row already announces the sender's name; an avatar that is
not hidden makes Narrator and TalkBack read a letter before every sender.

## Where a photo may come from

Only address books the user already syncs, over connections they configured. Three hard noes,
each closing a channel that would leak who the user is reading mail from:

- **No Gravatar or Libravatar.** It sends a hash of every correspondent's address, plus the
  user's IP and the time they opened the message, to a third party, a read-receipt side channel
  aimed at exactly what [`privacy-policy.md`](privacy-policy.md) promises the app blocks. The
  hashes are not protective: address space is far too small to resist a dictionary, whether the
  spec says MD5 or SHA-256.
- **No BIMI.** The record may only be an SVG, which is script-capable and fetched from an
  arbitrary domain; and it is scoped to an organizational domain, so it is a brand logo rather
  than a person.
- **No sender-domain favicons.** Same arbitrary-host fetch, same open-tracking channel.

No jurisdiction-gate call site is needed: connecting a mail account is a ratified carve-out
([`provider-oauth.md`](provider-oauth.md) → "Sovereignty scope"), and nothing here reaches a
destination the user did not choose.

**Who actually gets a photo, stated honestly:**

| Account | Photo for whom |
|---|---|
| Microsoft 365 (work/school) | Anyone in the tenant directory (the set Outlook shows) plus saved Outlook contacts |
| Personal Microsoft | Saved Outlook contacts only; consumer accounts have no directory at all |
| Google Workspace | Saved contacts, collected "other contacts", and same-domain colleagues |
| Personal Gmail | Saved and other contacts; no directory |
| JMAP | Contacts in the user's address books |
| CardDAV | Contacts with a vCard `PHOTO` |
| IMAP-only, or any external sender | Monogram. Every provider returns nothing for a stranger. |

## Bytes never cross the FFI; a path does

The engine writes photo bytes to a content-addressed file and the core hands the client **that
path**. Nothing is copied, the name changes when the content does (so a client may cache
against it indefinitely), and every platform loads it with a built-in decoder and **no new
image-loading dependency**.

**The core sniffs magic bytes before any path reaches a client** and accepts only PNG, JPEG,
GIF and WebP. A provider's `media_type` is remote content describing itself and is *not* the
check. **SVG cannot pass by construction** (it has no magic number), which is the point:
keeping it out needs no rule that someone could later relax, and nothing in
[`rendering-security.md`](rendering-security.md) permits it near a client surface. Anything
over 2 MB falls back to the monogram.

## Resolution never blocks a row

A monogram costs nothing; a photo costs a store read and, the first time, a provider fetch.
Rows are projected inside the snapshot rebuild, which does store reads only, so what a row
draws is read from an in-memory map, and everything the map cannot answer is resolved by a
bounded background pass that publishes **one** further snapshot when it finishes. One, not one
per photo: a publish per face would signal every client dozens of times for a single screenful.

"Nobody has a photo for this person" is recorded as an answer, or every pass would re-ask the
provider about the same strangers forever. It is given up when contacts sync, because that
replaces the index the answer was derived from.

**Opening something never regresses to the monogram.** A reading header and a contact detail are
each built one at a time rather than as a list, so each must read the same resolved map instead
of projecting a fresh avatar: otherwise the list draws a photograph and tapping it draws
initials. Neither queues a lookup: both are reached from a row that is already on screen behind
them, so there is nothing left to ask.

## The unread dot

The read-state glyph is retired. On **desktop layouts** (wherever the list sits beside a
reading pane) the row reads *dot · avatar · text*. On the **compact phone list** there is no
dot: bold subject and sender already carry unread, and the width is better spent on the text.

## Per-platform

| Platform | List row | Reading header | Contacts row + detail | Dot rule | Hidden from AT | Where |
|---|:---:|:---:|:---:|:---:|:---:|---|
| Core | ✅ | ✅ | ✅ | n/a | n/a | `mailcal-viewmodel/src/avatar.rs`, `mailcal-app/src/avatars.rs` |
| macOS / iOS / iPadOS | ✅ | ✅ | ✅ | ✅ | ✅ | `AvatarView.swift`, `Mailcal.MailRows.swift`, `ReadingView.swift`, `Contacts/` |
| Android | ✅ | ✅ | ✅ | ✅ | ✅ | `AvatarView.kt`, `MailRows.kt`, `ReadingScreen.kt`, `ThreadReading.kt`, `ContactsScreen.kt`, `ContactDetailSheet.kt` |
| Windows | ✅ | ✅ | ✅ | ✅ | ✅ | `Controls/AvatarView.cs`, `ViewModels/AvatarItem.cs`, `MailListView.xaml`, `ReadingView.xaml`, `ContactsView.xaml` |
| Linux | ✅ | ✅ | ✅ | ✅ | ✅ | `clients/linux/src/ui/avatar.rs` |

"Dot rule" is the row below: a dot where the list sits beside a reading pane, none on a compact
phone list. It is a column because it is the one thing here a client can draw *plausibly* wrong:
a dot on every layout looks fine until you notice the phone's subjects are truncating.

## Known gaps

- **Nothing on Windows asserts that the circle is on screen.** It is decoration, so it is hidden
  from assistive technology, and UI Automation is the only machine that looks at the running
  client, so being correctly hidden is exactly what puts it out of reach. `Accessibility.Tests.ps1`
  holds the half that can be checked (that it is *not* announced), the unit rules hold the
  projection, and what it draws is verified by eye against the seeded harness.
- **No photo reaches a client over the JMAP contacts path.** Against the harness, the same cards
  on the same server give six faces through CardDAV (`--account stalwart-imap`) and none through
  JMAP (`--account stalwart`). Stalwart's `ContactCard/get` does return the picture (a `data:`
  URI carrying the bytes, with no `blobId` beside it), so what a client is handed is decided in
  the engine's JMAP contact mapping, and closing it is engine work, not product work. Until it
  closes, the CardDAV account is the one to debug a photo against.
- **Nothing evicts the photo blob area.** It is bounded in practice by how many correspondents a
  user actually looks at, but it only grows.
- **No stacked multi-participant avatar** on a thread row, none in composer recipient pills or
  the account switcher, and no writing or deleting a contact photo.
- **A face arrives a beat after the list does**, by design. A cold or offline boot draws every
  cached row immediately and syncs behind it, so the first paint is monograms and photos land
  when contacts have synced. Measured against the harness: rows at `…43.084`, contacts synced
  at `…43.706`, the photo on the row at `…43.708`.

## Enforcement

When you change what an avatar shows:

1. Keep the preference order (photo, then monogram, never blank) and apply any change to
   **every** platform that ships the surface, or record the shortfall under Known gaps.
2. Derive the letters from the shared `initials`, never a second copy, and take the colour from
   the shared palette. Two surfaces disagreeing about a person is the failure this prevents.
3. Keep the avatar hidden from assistive technology. On Windows this is what
   `uitests/Accessibility.Tests.ps1` will fail on.
4. Never widen the accepted image formats without reading
   [`rendering-security.md`](rendering-security.md) first.

[`color`]: ../crates/mailcal-viewmodel/src/color.rs
