# App-store listing: cross-platform contract

**Scope.** How the app describes itself in **every** app store: Microsoft Store (Windows), App Store
Connect (Apple: macOS, iOS, iPadOS), Google Play Console (Android) and a Linux software centre. One
product, one story: the description a user reads before installing must say the same thing on every
store and in every language the build ships. Store copy is **marketing derived from the product**,
not a place to invent capabilities: it must match what the app actually does today
([`capabilities.md`](capabilities.md)), the brand voice (clear, plain, anti-hype), and the
[`privacy-policy.md`](privacy-policy.md) it links to.

**The rules are here; the copy is not.** This file holds what may be claimed, which locales move
together, the stores' field limits and how the Linux metadata is generated. The copy itself, and
the runbooks for pushing it to a console, live beside the brand it belongs to, resolving exactly the
way [`branding.md`](branding.md)'s identity files do:

    branding/<brand>-listing.md   the product's own copy, when that file is present
    branding/default-listing.md   the neutral default, which is always present

A build reads whichever it resolves to, through `brand.listing_source()`. Removing the branded file
leaves a working unbranded listing rather than a hole, which is the same property that makes
`branding/default.env` the thing a fork inherits.

**Languages are the localisation catalog, not a per-store choice.** A branded listing publishes a
store translation for **exactly** the locales in
[`../project.inlang/settings.json`](../project.inlang/settings.json): today `en` · `nl` · `de` ·
`fr` · `es` · `it` · `pt`. Adding a locale to the app (a new `messages/<locale>.json`) means adding
its store translation in the same spirit; a store may never carry a language the app doesn't, and
vice versa. The neutral default is the deliberate exception, and says why in its own header.

**Why one body, not three.** Three stores, seven languages, and the ambition to "keep them in sync"
is exactly how copy drifts: someone edits Partner Center, forgets Play, and the sovereignty promise
now reads differently depending on where you installed. So the **description body is written once**
(the listing file's "Shared description" section) and reused verbatim on every store; only the
fields the stores **structurally** differ on (Apple's Subtitle/Keywords, Play's Short description,
each store's feature list, and the one word for the platform keystore) live in the per-store
section. Edit the shared body and it changes everywhere by construction.

---

## The rule

1. **The shared description body is edited in one place** (the listing file's "Shared description"
   section) and used **verbatim** in every store. Do not hand-edit the body inside a store console.
   On the Microsoft Store it is not retyped at all:
   [`scripts/dev/msstore_listing.py`](../scripts/dev/msstore_listing.py) pushes the listing file
   into the submission.
2. **Cover every store, in every catalog locale.** A change to what the app does that touches the
   copy updates **all three** stores and **every** catalog language, in the same change.
3. **The copy may not out-run the matrix.** A capability appears in a store's listing only if the
   capability matrix marks it shipped (✅) for **that store's platform(s)**. A feature that is
   🚧 or ⬜ on the store's platform is **left out**, never soft-promised: the brand's anti-hype rule
   is load-bearing here. This is the tie to the capability matrix in place of a matrix row: the
   listing is checked *against* it. A listing file records what it is holding back, and why, under
   its own "Deliberately left out".
4. **No em dashes in shipped copy.** Every fenced block in a listing file is text a store renders,
   and none of it uses `—`. A bracketed aside takes parentheses, and a dash that introduces what
   follows takes a colon; in French the colon keeps the space before it that French sets. This is
   house style for copy a customer reads, the same rule [`pledge.md`](pledge.md) already follows,
   and it binds the fenced blocks rather than the prose around them.
5. **Consistent with the privacy policy.** Any privacy claim in the copy ("nothing routes through
   the vendor", "analytics is off by default", "credentials in the keystore") must be true per
   [`privacy-policy.md`](privacy-policy.md). If the policy changes what is stored, sent or shared,
   revisit the copy in the same change.
6. **Respect each store's field limits** (the "Field limits" table below); `store-copy` in CI
   measures this, so it fails on a branch rather than at submission. Where the shared body exceeds a
   field, trim from the tail (the feature bullets), never the first two paragraphs: the sovereignty
   framing is the point, and it is also the only part the Linux generator takes. **Trim every
   locale, not only the ones that failed:** the body is written once and reused, so a cut applied to
   French alone would leave the stores telling different stories. Tightening a *translation's*
   wording without dropping a clause is not a trim in this sense and is always allowed, which is how
   the framing paragraphs can be shortened while rule 5 still protects them.

---

## Field limits (as of writing; confirm in each console before submitting)

These are the stores' own published caps, not anyone's copy, which is why they stay here rather than
moving with the listing: a contributor writing a changelog fragment is measured against them, and
that has to work in a checkout with no branded listing at all.

| Store | Name/Title | Subtitle | Short/Promo | Description | Feature list | Search terms | What's new |
|---|---|---|---|---|---|---|---|
| Microsoft Store | 256 | n/a | n/a | 10,000 | up to 20 × ~200 | up to 7 × 30 (21 words) | 1,500 |
| App Store Connect | 30 | 30 | 170 (Promo) | 4,000 | n/a | Keywords: 100 | 4,000 |
| Google Play | 30 | n/a | 80 (Short desc) | 4,000 | n/a | n/a | 500 (release notes) |

**On the Microsoft Store's 30.** Its own documentation states both **30** and **40** characters per
search term in different places. **40 is what the ingestion API enforces**: measured directly on
2026-08-03: a 40-character term is accepted, a 41-character one is refused with `The length of
Keyword must be 40 or less`. This table still says **30** deliberately: the console is a separate
validator from the API, a term that fits 30 is accepted under either reading, and nothing shipped is
close to either number.

The **21 words across all seven terms of one language** is the constraint that actually binds: a
word budget, not a character one. It is a **total**, not a count of distinct words, and **a hyphen
splits a word**. Both of those were measured rather than read, and this table asserted the opposite
of each until the Store rejected copy that the gate had passed.

**The product name is measured against the name fields, and it is not written here.** It is the
injected `MAILCAL_APP_NAME` ([`branding.md`](branding.md)), so the check reads it from there: two
sources for one name is exactly the drift the branding overlay exists to prevent, and the 30
characters Apple and Play allow is the tightest cap any identity has to fit.

**These numbers are enforced, not merely stated.**
[`scripts/ci/check_store_copy_length.py`](../scripts/ci/check_store_copy_length.py) **parses this
table** and measures every field of the resolved listing against it: descriptions (once per store,
as they would be pasted), the product name, the Microsoft search terms (count, length **and** the
word budget), the Microsoft feature lists (count *and* per-item length), Apple's Subtitle /
Promotional / Keywords, Play's Short description, and every changelog fragment and released note
under `docs/changelog/`, each against the tightest store its own `Platforms:` reach
([`changelog.md`](changelog.md)). It runs on **every** push (the `store-copy` CI job, deliberately
not behind change-area gating (a docs-only PR is exactly the kind that edits copy). So the table is
load-bearing: change a number here and you change what is enforced; the two cannot drift. If a
console really does change a limit, edit the cell.

**A listing field the resolved file does not carry is not measured**, because the neutral default
deliberately carries only the two the Linux build needs. What that costs is nothing a store would
have caught: a *push* names each section it wants and refuses without it, in the console-specific
tooling that is the only thing which needs them.

---

## Flathub (Linux): generated, not drafted

Linux is the one store whose listing is **assembled at build time** rather than pasted: an AppStream
metainfo file is what a software centre reads, and
[`scripts/dev/flatpak_metadata.py`](../scripts/dev/flatpak_metadata.py) emits it (and the `.desktop`
entry's `Comment`) during the Flatpak build. This is not a second copy of anything; it records which
fields the generator takes and, more importantly, which it deliberately leaves empty.

| AppStream / desktop field | Taken from |
|---|---|
| `<name>` | the injected `MAILCAL_APP_NAME` ([`branding.md`](branding.md)), never the listing |
| `<summary>`, `Comment` | the listing's "Flathub — Summary", per locale, minus its trailing full stop (AppStream's validator rejects one) |
| `<description>` | the **first two paragraphs** of the listing's shared body, per locale |
| `<release version date>` | `/VERSION` + that release's note ([`versioning.md`](versioning.md)) |

Those two fields are what every listing file must carry, branded or not, because this is the one
store that cannot be typed by hand: a build with no metainfo has no entry in a software centre at
all.

**The summary is the one field Flathub does not share with another store.** Its quality guidelines
cap it at **35 characters**, against Play's 80, and ask for something a non-technical reader
understands: no protocol names, sentence case, no full stop, not starting with an article. One line
cannot serve both, so the listing carries a "Flathub — Summary" section of its own and
`flatpak_metadata.py` refuses one over the cap. Nothing downstream would catch it: a guideline is
not a gate, and the cost of breaking it is a volunteer reviewer's comment and a round trip.

**The `.desktop` `Categories` names one main category, and that is deliberate.** `Network` and
`Office` are both main categories, and an entry naming two is filed under both, so the app appears
twice in the application menu. `Email` and `Calendar` are additional categories that `Office`
satisfies, so `Office;Email;Calendar;` says the same thing about the app and places it once.
`desktop-file-validate` reports the two-main case as a hint rather than an error, which is how it
survived until `flatpak-builder-lint` was run against a real submission. **English is required and every other locale is optional**: AppStream falls a reader back to
the untagged paragraph directly above, which is what lets the neutral default be English-only.

**The feature bullets are still left out, and that is rule 3 doing its job.** A branded body's
bullets describe search, contacts, signatures, swipe actions, mail actions, threading, background
delivery and invitations. Linux now ships every one of those but **configurable swipe actions**,
which is ⬜ in the capability matrix. The body is one block of copy reused verbatim, so a listing
that took it would claim a capability this client does not have, which is precisely the copy
out-running the matrix that rule 3 forbids.

The gap is much narrower than it was, and closing it is a **deliberate edit here** rather than
something to infer from the matrix: either the shared body gains a Linux-shaped variant, or the
swipe-actions row lands and the bullets go in whole.

The first two paragraphs are the sovereignty framing, which describes the *product*, makes no
capability claim, and is the part rule 5 already names as never trimmed. So the generator can take
them for any client at any maturity, while the bullets wait for the matrix.

**The bullets a client may claim are a deliberate edit to the generator**, not a silent consequence
of a cell flipping, which is the only way rule 3 can be enforced by a machine rather than remembered
by a person. Cells flipping to ✅ without this section changing is that rule working, not an
oversight.

---

## Known gaps

- **The Flathub gallery has no published images yet.** `scripts/dev/showcase.sh linux` captures all
  six screens (seven captures, since the mailbox list is shot light and dark) but an AppStream
  `<screenshots>` block addresses images by **URL**, and nothing has been uploaded. Until
  `clients/linux/flatpak/screenshots.json` exists, the generator emits a comment in place of the
  gallery rather than an empty element (invalid) or invented URLs (valid, and broken in front of a
  user). The images belong in the same content-addressed store the user guides use
  (by the publisher's own tooling), so **publishing precedes shipping**: a
  listing naming an image the store cannot fetch is a broken gallery. Flathub also wants one showing
  what the app does; two of the six capturable screens (`settings`, `add-account`) are chrome, so
  this gap does not close on its own; it closes when Linux reaches more of the matrix.
- **A branded listing keeps its own "Known gaps"**, for the consoles it actually pushes to. They are
  operational rather than contractual, so they are not restated here.

---

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change store copy or ship a
capability that changes what would be advertised:

1. Edit the **shared body once**, in the resolved listing file; never inside a store console.
2. Update **all three** stores and **every catalog locale** in the same change.
3. Check the copy against the capability matrix: a feature appears only where its store's
   platform is ✅; anything else goes under that listing's "Deliberately left out".
4. Keep every privacy claim consistent with [`privacy-policy.md`](privacy-policy.md).
5. Run the length check before you push: `python3 scripts/ci/check_store_copy_length.py`. It is
   the one part of this contract a machine can hold, and it is the part whose failure otherwise
   arrives from a store console.
6. When the change reaches a live listing, push all three stores from that file rather than typing
   any of them: `scripts/dev/msstore_listing.py`, `scripts/dev/appstore_listing.py` and
   `scripts/dev/publish_play.py`, each with its own runbook beside the copy. Read the plan first,
   then `--apply` / `--commit`; pressing Submit stays a human's job in every console.
