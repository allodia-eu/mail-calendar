# Release notes / "What's new": cross-platform contract

**Scope.** The one source of truth for the user-facing release notes ("What's new") that Allodia
Mail & Calendar shows in **every** store: Microsoft Store (Windows), App Store Connect (Apple:
macOS, iOS, iPadOS) and Google Play (Android). One release, one story: the note a user reads before
updating says the same thing in every language we ship, and, where the change reached them, on
every store. The notes are **pasted into each console by hand** at release; there is no store-API
automation, matching how the app, the copy ([`store-listing.md`](store-listing.md)) and the version
([`versioning.md`](versioning.md)) already ship.

**Principle.** A user-facing change writes **one new file that no other change is writing**, and a
release **assembles** them. That is the whole design, and it is structural rather than procedural:

```
docs/changelog/unreleased/<slug>.md      one pending change: you write this
docs/changelog/unreleased/_summary.md    optional, at release time: see "When a store's cap cannot
                                         hold the release"
docs/changelog/released/<X.Y.Z>.md       what a release shipped: scripts/dev/release.py writes this
docs/changelog/announcements/<X.Y.Z>.md  the forum post: same script, same moment
docs/changelog.md                        this file: the rules, and the index of releases
```

This replaced a single 1,900-line file that every user-facing PR edited, at the top, in seven
languages. Two PRs in flight always conflicted, in the same place, every time, and because the note
was bound to a `/VERSION` bump they conflicted in `/VERSION`, `Cargo.toml` and
`clients/apple/project.yml` too. Nothing about *what* we write changed; only where, and how many
files a change has to touch to say it.

**Languages are the localisation catalog, not a per-store choice.** Every fragment carries a note
for **exactly** the locales in [`../project.inlang/settings.json`](../project.inlang/settings.json)
(today `en` · `nl` · `de` · `fr` · `es` · `it` · `pt`), the same rule as
[`store-listing.md`](store-listing.md), and it is **enforced**: a fragment missing a locale fails
the `store-copy` check. Adding a locale to the app means adding its note to every **pending**
fragment. A release already submitted is history and is left as it was shipped, which is why
`released/0.2.0.md` still carries `en` + `nl` alone.

---

## Writing a fragment

One file per user-facing change, in `docs/changelog/unreleased/`, named for the change
(`mac-remembers-pane-widths.md`, `signatures.md`). The name is yours to pick; the point is that it
is *yours*, so nobody else's PR touches it.

````markdown
# Attendee list on an event

Platforms: macos, ios, windows, android
Bump: minor

> Engineering commentary for reviewers: why the change is shaped this way, what it cost, what it
> deliberately leaves out. Never measured, never pasted into a store. It survives the release: it is
> carried into the assembled note's "Changes in this release" appendix.

**English**

```
See who is coming to a meeting: who called it, and who has accepted, declined, answered maybe or
not replied yet.
```

**Nederlands**

```
Zie wie er bij een afspraak komt: wie hem belegde, en wie accepteerde, afwees, misschien
antwoordde of nog niets liet weten.
```

(… Deutsch · Français · Español · Italiano · Português)
````

- **`Platforms:`**: `all`, or a comma-separated subset of `macos`, `ios`, `windows`, `android`,
  `linux`. It says where the change *landed*, and it decides two things: which stores get this
  bullet, and which character cap the note is held to. `ios` covers iPadOS: they share one App
  Store record, so they cannot be given different notes even in principle. The platform → store map
  lives in code, in
  [`scripts/ci/changelog_fragments.py`](../scripts/ci/changelog_fragments.py), so it cannot drift
  from what is enforced.
- **`Bump:`**: `minor` for a capability a user gains, `patch` for a fix. This is what decides the
  next version number: **minor if any pending fragment says minor, else patch**.
- The **notes** are one `**Language**` line and a fenced block per catalog locale, deliberately the
  same shape [`store-listing.md`](store-listing.md) uses, read by the same parser.

Internal-only work (a refactor, a test, a doc, tooling) changes nothing a user sees and writes no
fragment.

---

## The field this feeds, and the cap that applies

Each store has a "What's new" / release-notes field, and they cap it differently
([`store-listing.md`](store-listing.md) → "Field limits"):

| Store | "What's new" limit |
|---|---|
| Google Play | **500 characters** (per language) |
| Microsoft Store | 1,500 |
| App Store Connect | 4,000 |

**A note is measured against the tightest store its `Platforms:` actually reach.** An `android`
fragment gets Play's 500, the binding constraint, and the one to write to whenever a change ships
everywhere. A `macos`-only fragment gets Apple's 4,000, because Play is never going to see it.
A `linux`-only fragment reaches no store and is not measured: it is written down and shipped like
any other, it just has no console to be rejected by.

**The caps are measured, not trusted.** Every fragment and every released note is counted by
[`scripts/ci/check_store_copy_length.py`](../scripts/ci/check_store_copy_length.py) against the
"Field limits" table, in the `store-copy` CI job, on every push. It takes the *minimum* of the
applicable stores rather than a hard-coded 500, so if Play ever raises its ceiling the next store's
number takes over on its own. A 501-character note fails on the branch instead of in Partner Center,
after a build number has been spent.

---

## The announcement · one page for the whole release

A store note only ever describes the platform of the store it sits in, and a reader has to already
own that app to find it. So a release also gets **one page covering every client at once**, posted
to the support forum, where it can be linked to.

[`announcement.py`](../scripts/dev/announcement.py) assembles it from the same fragments, and
`release.py` writes it to `docs/changelog/announcements/<X.Y.Z>.md` in the same run. It has to
happen there and nowhere else: the released note keeps each change's headline and its engineering
commentary, but the **user-facing note survives only inside the per-platform sections**, already
merged into an authored summary wherever a store's cap could not hold the list. After the fragments
are deleted, that text cannot be recovered.

What it decides, so nobody re-decides it per release:

- **Grouped by reach, not by platform tuple.** One "Every app" section for changes that reached
  every *shipping* platform, then a section per platform for the rest, so a change that landed on
  two apps is listed under both. Grouping by distinct tuple instead produces headings like
  "macOS, iPhone & iPad and Android", which is a spec rather than something a reader scans.
- **"Shipping" is derived from the store map**, not listed here. A platform with no entry in
  `PLATFORM_STORES` cannot be installed, so its changes are reported under "in development, not yet
  released" rather than announced beside ones a reader can go and get. The day Linux gets a store it
  moves out of that section on its own: a hand-kept list would instead keep calling a shipped
  client unreleased, in the one document written for people who do not follow the repo.
- **New before Fixed**, labelled only when both are present: a "Fixed" heading with no "New" beside
  it announces the absence of the other kind.
- **English only.** It is one forum post, not a per-locale store field.

**It is a draft, and its lead paragraph says so in capitals.** Which two or three changes a release
is *about* is an editorial judgement no generator makes from a fragment list, and a placeholder
that does not admit it is one gets posted.

---

## Cutting a release

```sh
scripts/dev/release.py --dry-run     # see what would be assembled
scripts/dev/release.py               # version computed from the fragments' Bump: lines
```

It measures every section against the tightest store it is pasted into and **refuses before writing
anything** if one is over (see below). Otherwise it writes `docs/changelog/released/X.Y.Z.md` and
`docs/changelog/announcements/X.Y.Z.md`, deletes the fragments it consumed, adds a row to the index
below, runs
[`bump-version.sh`](../scripts/dev/bump-version.sh), which is the **only** thing that moves
[`/VERSION`](../VERSION), and then runs the store-copy check over what it wrote.

The assembled note carries **one section per distinct bullet set**, not one per store. When every
fragment is `all` (the common case), that is a single section with seven notes, exactly the
workload the one-file changelog had. Per-platform sections appear only when the content genuinely
differs, which is what stops a Mac-only fix appearing in the iPhone's note.

**What comes out is a draft.** Assembly is mechanical; the words are still yours to edit before the
note is pasted anywhere.

`release.py` does **not** commit and does **not** tag, the same deliberate rule as `bump-version.sh`:
a `vX.Y.Z` tag is what the release builds take as their input, so you tag when you mean to
release.

---

## When a store's cap cannot hold the release

A section is measured **before** anything is written, and `release.py` refuses the release rather
than leaving one half-cut: the fragments carrying the text you would need are the files it would
have just deleted.

Most of the time a section fits and there is nothing to do. What does not fit is the **big** release.
0.3.0 consumed twenty-one fragments, fourteen of which reached Android: **500 ÷ 14 is thirty-five
characters a change**, which no trim reaches. That is not an editorial failure: a Play note for a
release that size is structurally a *summary*, and the fragment format has no way to say one, because
a fragment is written months earlier by someone who cannot know what it will ship beside.

So such a section is **written, not assembled**, in `docs/changelog/unreleased/_summary.md`:

````markdown
## android

**English**

```
New: signatures, a Contacts tab, meeting invitations you can answer from the email, and search
that returns newest first. Fixed: replies keep the original, back steps back, …
```

(… every other catalog locale)
````

- **Keyed by the section's platform tuple**, exactly as `release.py` heads it, so a summary written
  for `android` cannot silently attach to a differently-grouped section in a later release. `all`
  means what `## macos, ios, windows, android, linux` means; the heading is read by the same parser.
- **Every catalog locale**, exactly as a fragment carries them. A summary replaces the assembled
  notes wholesale, so a gap is a language that ships nothing.
- **It is measured too.** An over-long summary fails the same gate: the escape hatch is not a way to
  switch the cap off.
- **It is consumed and deleted with the fragments**, because it describes *this* release's change set.
  An ordinary release writes none, and the file's absence is the normal state.
- The assembled section **says it was written rather than assembled**, and every fragment it stands
  in for is still named in the appendix, so a reader comparing the two reads a summary, not an
  omission.

Underscore-prefixed because it is **not** a fragment: it carries no `Platforms:` or `Bump:`, nobody's
feature PR writes one, and `load_fragments` skips it rather than failing on a malformed change.

Prefer trimming to summarising. A section that fits names each change in the user's own terms, which
is better than a paragraph that generalises them; reach for a summary only when the arithmetic
genuinely does not work.

---

## The rule

1. **One note per change, per locale: written once, pasted verbatim.** Do not hand-write release
   notes inside a store console; write a fragment here and copy the assembled note out.
2. **Every catalog locale.** A user-facing change adds a fragment carrying **all** of them (today
   `en` · `nl` · `de` · `fr` · `es` · `it` · `pt`), in the same change. This is enforced.
3. **Fit the cap the fragment's platforms imply**: Play's 500 whenever the change ships to Android,
   which is most of the time, so that one body serves all three stores. **Write it short**: a
   release assembles every pending fragment into one field, so a note's real budget is that cap
   divided by however many changes ship beside it. One sentence is the target, not the ceiling.
4. **The note may not out-run the matrix: same anti-hype coupling as the listing.** A note
   describes only capabilities the capability matrix marks ✅ for the platform of the store it
   is pasted into. `Platforms:` is how you say that: tag the change with where it landed, and a
   platform that did not get it never sees the bullet. Where one body covers several platforms and
   one of them lacks a detail, scope that sentence in the text ("On Mac and Windows the calendar now
   scrolls…") or leave it out, never soft-promise it (see [`store-listing.md`](store-listing.md) →
   "Deliberately left out"). The brand voice is the listing's: clear, plain, anti-hype,
   non-technical.
5. **Consistent with the privacy policy.** Any privacy claim in a note must be true per
   [`privacy-policy.md`](privacy-policy.md), the same as the listing copy.

---

## Releases

[`/VERSION`](../VERSION) holds the **last released** version: the number users currently have, not
the one we are building. Every version listed here has a note; `check-version-sync.sh` proves that
`/VERSION` does, and that no note claims a version above it.

| Version | Date | What shipped |
|---|---|---|
| [0.7.1](changelog/released/0.7.1.md) | 2026-09-01 | The date in a quoted reply |
| [0.7.0](changelog/released/0.7.0.md) | 2026-08-31 | Allodia Mail & Calendar on Linux |
| [0.6.0](changelog/released/0.6.0.md) | 2026-08-28 | A new account on a computer syncs more history · A repeating event says exactly how it repeats · An Allodia account of your own · …54 more |
| [0.5.0](changelog/released/0.5.0.md) | 2026-08-20 | A quiet note when mail is catching up · The composer opens on the fields a message usually needs · Answering an invitation works on an account whose calendar server will not answer for you · …60 more |
| [0.4.0](changelog/released/0.4.0.md) | 2026-08-04 | Removing an account is somewhere you would look for it · Account setup is laid out for the screen it is on · Attach files opens the picker again on iPhone and iPad · …6 more |
| [0.3.0](changelog/released/0.3.0.md) | 2026-08-03 | An event tells you who is coming · Answering an invitation, from the mail it arrived in · Contacts, and recipient suggestions while you type · …18 more |
| [0.2.2](changelog/released/0.2.2.md) | 2026-07-22 | Dutch dates on Mac, iPhone and iPad |
| [0.2.1](changelog/released/0.2.1.md) | 2026-07-21 | Easier sign-in for JMAP accounts |
| [0.2.0](changelog/released/0.2.0.md) | 2026-07-20 | First submission to the three stores |

---

## Known gaps

- **The announcement is posted by hand.** Nothing here talks to the forum, so the assembled page is
  copied into a new topic at release, and nothing checks that it was. The same shape as the store
  consoles below, for the same reason: one post per release is not worth an API integration.
- **No console automation.** Every release note is pasted into Partner Center / App Store Connect /
  Play Console by hand at release, a deliberate decision, matching the all-manual store uploads and
  the "What's new" paste noted in [`store-listing.md`](store-listing.md).
- **No git tag is created.** `release.py` prints the `git tag vX.Y.Z` command rather than running it,
  because the tag triggers a Windows Store build. Mirrors the same gap in
  [`versioning.md`](versioning.md).
- **Rule 4 is not machine-checked.** `Platforms:` makes "a note describes only what is ✅ on that
  platform" *checkable* in principle, but parsing the capability matrix is separate work.
  Until then it is a reviewer's duty, as it always was.
- **The date is the day the release is cut**, not the day a store approves it. Nothing reads store
  review status, so a note dated the 5th may reach users on the 8th.
- **A summary is prose, and nothing can check it against the changes it stands for.** The gate proves
  an authored section *fits*; only a reviewer can see that it does not claim something no fragment
  shipped, or quietly drop the one change a user was waiting for. Rule 4 applies to it exactly as it
  applies to a fragment, and the appendix beside it is what makes the omissions readable.
- **A cap cannot tell prose from padding.** 0.3.0's `mac-remembers-pane-widths` fragment carried 401
  literal `x` characters as its English note (a placeholder nobody filled in), and every gate passed
  it, because 401 is comfortably under 4,000. It was caught by reading the notes, which is still the
  only thing that catches it.
- **The migrated fragments' commentary still argues about version numbers that never shipped.** The
  twenty-one fragments 0.3.0 consumed were converted 1:1 from the old file's `0.3.0` … `0.15.1`
  entries, and several of their blockquotes reason about which of those numbers to take ("0.13.0
  rather than 0.11.0", "two releases cannot share a marketing version"). None of those versions ever
  reached a store: that collision is precisely what this rework removes, since a fragment claims no
  version at all. The commentary is kept **verbatim** in `released/0.3.0.md`'s appendix because it is
  the engineering record of the change, not a claim about the release; read the version bickering as
  an artefact of the format it was written in.

---

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you ship a user-facing change:

1. Add a fragment under `docs/changelog/unreleased/`, in **every** catalog locale, with the
   `Platforms:` it reached and the `Bump:` it deserves; never write the note first inside a store
   console, and never bump [`/VERSION`](../VERSION) yourself.
2. Keep each note within the cap its platforms imply (Play's **500** whenever Android is one of
   them) so one body serves every store it goes to.
3. Check the note against the capability matrix (rule 4) and against
   [`privacy-policy.md`](privacy-policy.md), the same two couplings as
   [`store-listing.md`](store-listing.md).

The machine half is the `store-copy` job
([`check_store_copy_length.py`](../scripts/ci/check_store_copy_length.py)): unknown platform tag,
bad `Bump:`, missing locale, over-cap note. The `version-sync` job
([`check-version-sync.sh`](../scripts/ci/check-version-sync.sh)) proves `/VERSION` names a release
that has a note. Neither can check whether the note is *true*: that is rule 4, and it is yours.
