# The contacts contract

Contacts are one product across every client. This file is the **contract** they all keep: what the
core decides, what a client decides, and the handful of rules that are not cosmetic: the ones where
getting it wrong produces a screen that looks perfectly plausible and is quietly lying to the user.

Like [`docs/calendar.md`](calendar.md) and [`docs/search.md`](search.md), this is a
**cross-platform** document. Raise a rule on one platform and you raise it everywhere; any shortfall
goes under "Known gaps" rather than staying silent.

---

## 1. The core decides who is one person. The client discloses it.

An address book is not a list of people, it is a list of **cards**, and the same person is
routinely filed in several of them. So the core derives a **unified person** from the cards, and
every client renders people, never cards.

The join is deliberately narrow:

| Joins on | Never joins on |
|---|---|
| a **shared canonical email address** | a name |
| | a phone number |
| | an organisation |

**Names never join, and this is the load-bearing half of the rule.** Two different people commonly
share a name (every company has two of some name), and a merge is *destructive to the user's
understanding*: they filed two colleagues and now see one, with one set of details silently winning.
A missed merge is a cosmetic annoyance; a wrong merge hides a person. The asymmetry is why the join
is conservative rather than clever, and why no client may add matching of its own on top.

### Nothing is merged without saying so

A merged row **must** disclose that it is a merge, and the client must be able to answer "why?".
This is the counterpart of the calendar's "nothing is hidden without saying so":

- The list row shows **"In N accounts"** whenever the person came from more than one account.
- The detail view names them, under **"Also in"**, and tags each individual value (this address is
  in your work account, that one in both).

**Count accounts, not source cards.** A person filed in two address books of *one* account has two
source cards and one account; a row reading "In 2 accounts" would be false. The core's
`ContactRow::account_count` already collapses to distinct accounts; a client must not recount.

And **never show "In 1 accounts"**: it is noise on every ordinary contact, and ungrammatical noise.

### A retired person id still resolves

Merging retires ids. The core keeps the retired ones pointing at the surviving person, so a row a
client is still holding after a background sync merged it **still opens**. A client therefore need
not refresh its list before opening a row it already has, and `contact_detail` returning `None`
means the person is genuinely gone, never merely renumbered.

---

## 2. What the core owns, and what a client owns

### The core

- **The join** (§1), and the account count behind it.
- **Ordering.** Alphabetical by display name, case-insensitively, with the id as a tiebreak so the
  order is total: two people with the same name never swap places between rebuilds.
- **Matching.** The search query runs in the core over name, email, phone, organisation and title.
  A client passing the query down rather than filtering its loaded rows is what lets a person
  *beyond the page cap* still be found.
- **The section letter** each row files under, and the `#` bucket for anything not starting with a
  letter, so digits and symbols collect in one section rather than each minting their own.
  **An accented letter is a letter.** "Émile" files under **E** and sorts between "Emil" and
  "Emma"; "Ärzte" under **A**, "Øystein" under **O**. Treating only ASCII as alphabetic breaks this
  twice over: those names land in `#`, *and* their code points sort after every ASCII name, so the
  list reads `#` … A … Z … `#` with a second bucket past the end of the alphabet holding every
  Dutch, German, French, Scandinavian and Polish contact. The fold covers Latin; other scripts file
  under `#` rather than being folded to a letter they are not. The **monogram keeps the real
  character**: only the section and the sort key fold.
- **The monogram initials.**

### A client

- **All localised copy**, as everywhere: the core owns no locale facility. That includes the
  "(no name)" placeholder: a card may legitimately carry an address and no name, and the core emits
  an **empty** `display_name` for it, never a placeholder of its own. A placeholder in the core
  could only ever be English, and a client cannot substitute one it has no way to detect. Empty is
  the signal; the client supplies the string (`contacts_no_name` in the shared catalog).
- **Layout and the section-header treatment**, and an avatar's **shape**: circle, rounded
  square, diameter. What an avatar *contains* is not the client's: the letters, the colour and
  the photo are decided in the core by [`avatars.md`](avatars.md), so the same person cannot be
  `AL` on one screen and `A` on the other, and four clients cannot disagree about whether a
  white letter is legible on a mid-green fill.
- **Whether to offer tap-to-mail / tap-to-dial** on a value.

---

## 3. A write names a card, never a person

Contacts can be **created and edited**. Deleting them is not offered yet ("Known gaps").

Everything else on this screen is about people, and a write is not: the engine joins several
accounts' cards into one person (§1), so saving the values on screen without choosing one of those
cards would file the work account's details in the personal account's address book, and the result
would look perfectly right. So:

- **An edit carries `(person, account, card)`**, from `ContactDetail::editable_cards`. The person
  is there so a row held across a merge still resolves; the account and the card are what the write
  actually goes to.
- **The editor is seeded from that card**, through `contact_card`, never from the merged detail the
  user was just looking at. A merged person's addresses belong to different cards, and offering
  them all for saving into one is the same mistake in another form.
- **The client asks which card when there is more than one**, naming the account. One card opens
  straight into the form.

### Both affordances are conditional, and each answers a different question

| Affordance | Shown when | Because |
|---|---|---|
| Create | `contact_targets()` is non-empty | A destination is a **writable** address book. A directory, a suggested-contacts source and a shared book the account may only read are places contacts come *from*; offering one produces a save that fails on the server after the user has typed everything in. |
| Edit | `editable_cards` is non-empty | Same rule, per person. |

A person with **no** editable card says so ("This contact can't be edited here"), in as many words,
rather than leaving it to be inferred from an absence or, worse, from a disabled pencil the user
presses twice and then doubts.

### What is editable, and what an edit must not destroy

The editable set is exactly what the detail screen **shows**: given name, surname, organisation,
role, the email addresses and the phone numbers. Nothing is editable that the app will not display
back afterwards.

A card holds far more than that, and an edit must leave all of it alone. Two rules carry that:

- **A patch names only the fields that changed.** Every field a patch names is *replaced*, and a
  replacement built from a form loses whatever the form cannot show: an address's `TYPE=work`, an
  organisation's departments, whether a title was recorded as a `TITLE` or a `ROLE`. A changed field
  is therefore rebuilt **on top of** the property the card already carried, and an unchanged one is
  left out entirely. An edit that changed nothing sends nothing.
- **A structured name (`N`) is written only when there is a surname.** A card may legitimately carry
  a formatted name and no structured one, and the form seeds such a card by putting the whole name
  in the first-name field. Emitting components from that would file "Ada Lovelace" as a first name
  with no surname, in every other client the user owns, on a save that changed nothing.

The **order** of the addresses is the order on the card, because the first one is the person's
primary address: what the avatar and the list row are keyed on (`avatars.md`).

### A card must be findable again

A create is refused unless it names something to file the card under: a name, an organisation, or
an email address. The formatted name is derived from the first of those that is present, so a
company contact reads as its company rather than as a blank row.

### What the status means

`ContactWriteStatus` has the same shape as the calendar's, plus one value:

- **`Failed` does not mean "rejected".** A write whose server call succeeded but whose post-write
  reconcile came back busy or failed has **landed on the server**; only the local copy is briefly
  stale, and the next sync heals it. The client says "we could not confirm this saved".
- **`Invalid` is a different sentence**: the core refused the form *before* sending anything, so
  there is something to correct and retrying unchanged would be refused the same way. The client
  states it under the form the user is still looking at, and nowhere else.

Neither write is durable offline yet: a failed save stays failed rather than queueing, which is the
same shortfall the calendar writes have.

---

## 4. Autosuggest is a separate feature that happens to share the index

Composer recipient autosuggest draws on the same people index, but it is **not** gated on contacts:
the engine mines recipient history from each account's **Sent** mailbox during the ordinary mail
sync (plus a one-time backfill of already-stored mail). So:

- Autosuggest works on an account with **no address book at all**, which is most accounts.
- A suggestion that came only from sent mail is as valid as one from a saved card. A client may
  mark the two apart; it must **not** hide the history-only ones, which are usually the most useful.
- A blank query returns nothing. A dropdown of everyone the user has ever emailed, the moment the
  To field takes focus, is noise rather than help.

**The token, not the field.** To/Cc/Bcc hold a *list* in one string. Query the whole field and
nothing matches once a first recipient is entered; replace the whole field on selection and every
recipient already typed is silently destroyed. Both are real, both are silent, and every client
must complete only the text after the last comma.

Addresses are inserted **bare**, not as `Name <address>`: the core parses addresses, a display name
adds nothing it uses, and a name containing a comma would split into two invalid recipients, which
the user would not discover until the send failed.

**Finished recipients are drawn as pills; the one being typed stays text.** This is the same split
as the completion rule above, rendered, so what the user sees as one recipient and what the
completion treats as one recipient cannot disagree. It matters because a bare field shows
`a@x.com, b@y.com` as a wall of text whose only boundary is a comma the reader has to find, and
offers nothing to tap to remove a wrong address. The field's value stays **one comma-separated
string**: pills are a view of it, never a second source of truth, so nothing can be on screen that
would not be sent. Each pill carries a remove control, and the accessible label names the recipient
rather than repeating a bare "Remove".

**A pre-filled field has nothing in progress.** The rule above (the trailing token is what is being
typed) is an inference about the *user*, and it is wrong for a field the composer was **opened**
with: a reply's derived recipients, an assistant's draft, a `mailto:` link. Every address a caller
supplies is finished, so the field is normalised to that at the moment it is seeded, and each one
renders as its own pill. Left raw, a reply-all's `To` drew one pill beside a loose address and a
single-recipient `Cc` drew no pill at all: the fields looked like they had dropped the people they
were in fact holding, and the composer's first autosuggest query went out against half of someone's
address. Two things this binds: normalise the **seed**, not the field a frame later (a rewrite after
opening reads as an edit, and the "Discard draft?" prompt would then fire on every untouched reply),
and compare against the **normalised** opening value wherever a client asks whether anything was
typed.

**The caret goes to the end after any programmatic change.** Accepting a suggestion rewrites the
text; a field that keeps its old offset leaves the caret mid-address, and the next keystroke lands
inside the address just inserted. (With pills this is mostly automatic: the accepted address
becomes a pill and the input empties, but the rule is stated because a client that renders the
field differently still owes it.)

**The list floats over what follows; it never displaces it.** A popup, an overlay, a popover:
whatever the toolkit calls the thing that takes **no layout space**. Inline, the list is a sibling
of the input: it appears and disappears on every keystroke and takes a field's worth of height with
it, so Cc, Bcc, Subject, the editor and Send all jump down and back while the user is still typing
the first recipient. On a phone it reaches further than it looks: the composer's header is
*measured* and its height becomes the editor's top inset, so the message body moves too. Four
things this binds:

- The surface is **opaque and shadowed**. It now covers live content, and a translucent fill
  renders two texts on top of each other.
- It **does not take focus**, or the keyboard closes and the keystrokes it is completing stop
  arriving. (WinUI: a non-focusable `Popup`. GTK: `autohide(false)`. Compose:
  `PopupProperties(focusable = false)`. SwiftUI: a plain button takes no responder.)
- Only the **focused** field's list is on screen. Moving from To to Cc must close To's: harmless
  while it sat in the layout, and covering live content the moment it floats.
- It clears the **editor**, which is a web view on every platform. This is the half that does not
  come free: a hosted web view is a real platform view, and a toolkit will happily composite it
  above the toolkit's own drawing however the list is z-ordered. On Apple neither `zIndex` nor an
  overlay on the enclosing scroll view was enough: the list came out sliced off at the editor's
  top edge, over the fields and under the message, and the layer had to move to the composer's
  outermost view. Assert it against a list long enough to reach the editor: one short enough to
  stop above it passes while the bug is still there.

**The caret opens where the work starts.** A composer that is already addressed (a reply, a
reply-all, a forward's quoted original, a mail link, an assistant's draft) opens with the caret in
the **body**, and raises the keyboard there. Anything else opens with the caret in **To**: on a new
message that is the empty field the user has to fill, and leaving it unfocused costs a tap on every
message they write. Exactly one of the two is focused, so a client decides it with **one
predicate** over (mode, To) rather than two flags that can disagree: `opensInBody` /
`composerOpensInBody` / `opens_in_body` / `FocusesBody`. Two traps, both silent: the focus must be
taken **after** the editor's seed snapshot, or moving the caret reads as the user having typed and
the composer opens dirty; and a toolkit that has not finished presenting or mapping the field
**drops** the request without complaining, leaving a field that looks ready and a keyboard that
never came.

---

## 5. Privacy

Contacts are the most identifying data the app holds. Two rules, both inherited rather than new:

- **Never log content.** The diagnostic log carries counts, ids and durations, never a name,
  address, phone number or organisation ([`docs/logging.md`](logging.md)). Address-book *ids* are
  not content and may be logged; the cards in them may not.
- **Nothing new is sent anywhere.** Contacts sync to and from the user's own configured accounts and
  nowhere else. Nothing about them enters the analytics payload, which is a closed enum of labels
  and structurally cannot carry them ([`docs/analytics.md`](analytics.md)).

The stored-data categories are declared in [`docs/privacy-policy.md`](privacy-policy.md), which this
change updates in every catalog locale.

---

## 6. Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Contacts list: unified people, A–Z, section headers | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cross-account dedup + the "In N accounts" disclosure | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Contact detail: emails / phones / org / title, with per-value provenance | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Contacts search (matched in the core) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Create a contact, into a chosen writable address book | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Edit one **source card** of a person, chosen when there are several | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Both write affordances hidden where no write could land | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Delete a contact | — | — | — | — | — | — |
| Composer recipient autosuggest (contacts **+** sent-mail history) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Composer recipients as **pills**, with per-recipient removal | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Suggestions **float**: the form below them does not move | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| The caret opens in **To**, or in the body when already addressed | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| CardDAV contact sources (one adapter per address book) | ✅ | — | — | — | — | — |
| JMAP contact sources (account-global adapter) | ✅ | — | — | — | — | — |

---

## 7. Known gaps

- **Linux's merge disclosure is proven against the harness, not against real accounts.** The rest
  of its matrix row was driven on the developer's **own accounts on 2026-08-19**, but "In N
  accounts" needs one person filed in two *different* accounts, which that data did not contain.
  The harness does: its `shared-*.vcf` cards file one person in alice's book and bob's, and
  `stalwart-multi` connects both at once, so
  [`test-linux-ui.sh`](../scripts/dev/test-linux-ui.sh) now ends by reading that row and its
  disclosure off the screen. The detail's **"Also in"** stays on its widget test: a contacts row
  exposes no AT-SPI action, so nothing can open a person from the driver.
- **Contact photos are fetched.** The engine's photo cache is wired up and a person's face
  reaches the mail row, the reading header **and** the contacts row, from one resolution shared
  across all three ([`avatars.md`](avatars.md)), so the same person cannot show a photo on one
  screen and a monogram on another. The client matrix lives in `avatars.md`.
- **The floating list and the opening caret are unverified on Linux.** Both rules were driven
  against the harness on **2026-08-23**: macOS, iPhone and iPad (the caret opens in To, a reply
  opens in the body, the list covers the editor's web view without moving anything under it, the
  signature editor still opens focused) and an **Android emulator** (caret in To with the keyboard
  up, the list hanging under the input over the Subject field, accepting a suggestion, and the
  event editor's title). **Windows** followed on **2026-08-24**, held by
  `uitests/ComposerFocus.Tests.ps1`: five cases, each watched failing against the rule broken.
  Linux's half is host code this repo cannot build on a Mac: an AT-SPI run is owed, with a list
  long enough to reach the editor, per the rule above.

  **A dataset can be too small to test this, and it fails green.** The Windows suite runs against
  the harness rather than the showcase for one reason: the showcase people index holds two
  addresses, and a two-item list is shorter than the gap below every recipient field: at its
  closest, from the lowest field, it stops 12px above the editor. It clears the web view entirely,
  so a showcase run would have reported the one rule it cannot see as holding. The suite therefore
  asserts the match count before the geometry, so a thinned fixture and a broken rule cannot report
  the same way.

  **Where the list lands needs its own assertion, and on Android it needs a device.** The
  screen-level test ("nothing below the list moved") passes just as well when the list is drawn on
  *top* of the input it is completing, which is what Compose's `Alignment.TopStart` did: a popup
  aligns inside its parent's bounds, not where it is written. Compose's test framework cannot see
  the difference either, because a popup owns its own root and reports its bounds as (0, 0). So the
  Android half asserts the position *decision* (`RecipientPopupPositionTest`) and the rendering was
  confirmed on an emulator.
- **The Linux UI suite drives the editor up to the write and no further.**
  [`test-linux-ui.sh`](../scripts/dev/test-linux-ui.sh) runs against the shared harness and the app
  offers no delete, so an actual create would leave a card behind and accumulate one per run. It
  asserts the half a screenshot cannot: that the create affordance is offered at all (it is hidden
  unless the core reports a writable address book, so its presence *is* the write-destination
  binding working), that the form opens, and that Save on an empty form refuses rather than closing
  over a card nobody could find again. The write itself is gated in `mailcal-app`'s contacts suite
  against a real engine, and was driven by hand against the harness (above).
- **Contacts cannot be deleted.** The engine supports it through the same outbox the create and the
  edit ride, so this is scope rather than a missing capability: a delete is the one contacts write
  a user cannot undo, and it wants a confirmation and an account-naming step of its own, on the
  same terms the calendar's delete already has.
- **The write paths were driven on Linux and Android against the harness on 2026-09-02**, over
  JMAP: create and edit, each read back off the server. The **Apple** and **Windows** halves are
  written to the same contract and are unverified against a running client. What is gated on every
  platform is the pure layer (which card an edit names, what a form is refused for, which fields a
  patch carries), which is where the silent failures are.

  **Read a JMAP write back over JMAP.** Stalwart renders the same stored card for both protocols
  and the two disagree after a `ContactCard/set`: over CardDAV the card comes back holding `FN` and
  a `JSPROP` line, with `ORG`, `TITLE` and `EMAIL` gone, while `ContactCard/get` shows it intact.
  Verifying through the wrong door therefore reads as exactly the data loss a contacts edit is
  most likely to cause (`.agents/skills/mail-harness`).
- **A JMAP account writes into one address book.** The adapter is account-global and its bound book
  decides where a write lands, so the connect binds it to the account's default writable book (else
  the first) and offers that one destination. CardDAV, whose adapters are already one per book,
  offers each of them. A JMAP account with several books can therefore read them all and write to
  one.
- **Postal addresses, notes, anniversaries and photos are not editable**, and are carried through an
  edit untouched (§3). Adding them means a wider form on four platforms and, for addresses, a
  structured one; the field set stops where the detail screen does deliberately.
- **A contact's kind is always `individual` on create.** A company contact is filed as an
  individual carrying an organisation, which is what its `FN` reads as. Offering the kind is a
  picker nobody has asked for yet; the engine models every kind and preserves the one a card
  already has.
- **Microsoft Graph and Google contacts do not sync yet: the scope half is done, the binding is
  not.** Sign-in now requests the contact scopes for both families (`provider.rs`), but the core
  still binds no contacts adapter for either (`contact_providers: Vec::new()` on every
  Graph/Google connect path), so nothing is read yet. The engine side is *done*: `provider-graph`
  and `provider-google` both implement `ContactsProvider`, so what remains is purely the binding.
  A connected account says so in the log rather than looking broken:
  `connection_info: … account_type=graph contacts_sources=0`.

  **The Google scope tier was recorded wrongly here and is worth correcting**, because it changed
  what the work costs: Google People is *not* a restricted scope. `mail.google.com` and `calendar`
  are restricted, and those are what tie the app to the security assessment it is already waiting
  on; `contacts`, `contacts.other.readonly` and `directory.readonly` are **sensitive**, which is a
  declaration, a justification and a demo video, not a second assessment. Adding them does not
  deepen the gate the app is already behind.
- **A JMAP account signed in with OAuth now asks for contacts too.** `WANTED_CAPABILITIES` used to
  omit the `contacts` scope, so the token was never granted it and the card sync was refused: a
  logged, tolerated skip, but one that left Fastmail-over-OAuth showing an empty Contacts list
  while the *same* account added with an app password showed a full one. It is now requested,
  which closes that asymmetry. A server advertising no `contacts` scope is unaffected: the
  selection only ever asks for what the metadata offers. Already-connected JMAP accounts keep
  working on their existing grant and pick the scope up on their next reconnect.
- **Contacts opened in the first second of a cold launch shows an empty list, and does not retry.**
  Account connect, and with it the binding of each account's contact sources, completes
  *asynchronously* after the core is built, so a `RefreshContacts` dispatched before it lands logs
  `contacts[aN]: skipped; no contact sources bound` and rebuilds zero rows. Nothing re-runs it, so
  the screen reads "No contacts yet" until the user leaves the surface and returns. This is not
  per-client: every client dispatches the refresh on entering the surface, and the fix belongs in
  the core, which should rebuild when a contacts provider binds rather than only when a host asks.
  Deliberately **not** worked around with a per-client heuristic: a retry invented in one client is
  exactly the divergence this document exists to prevent.
- **Android opens a contact's detail on the UI thread.** `contact_detail` and
  `recipient_suggestions` are network-free but not free: each blocks on the core's runtime and
  lands on the store's connection thread, so a call made while a sync holds that connection waits
  for it. The Apple and Windows clients await both off the UI thread; Android reads the detail
  inline in the tap handler, so opening a person during a large sync can stutter. Autosuggest is
  already off the UI thread everywhere.
- **Contact groups are not shown.** The engine derives a person from every card kind, including
  `KIND:GROUP`; the view-model filters those out, because a group has no address and would render
  as a row with a blank second line that opens onto nothing. Organisations are **not** filtered: a
  company with an address is a contact. Showing groups properly (as expandable lists of members) is
  a feature, not a filter change.
- **Only Latin names get a section letter.** The A–Z fold covers Latin-1 Supplement and Latin
  Extended-A: the diacritics a European address book holds. A Greek, Cyrillic, Hebrew, Arabic or
  CJK name files under `#`, alongside the digits and symbols. Folding those to a Latin letter would
  be worse than saying nothing; giving each script its own sections (and ordering them) is a
  feature, not a wider table.
- **The list is capped at 200 people** with no "show more". Search narrows in the core, so a person
  beyond the cap is findable, but not scrollable-to. A directory-sized address book needs paging.
- **No per-account or per-address-book filter.** The engine's `PeopleQuery` supports both; no client
  surfaces them yet.
