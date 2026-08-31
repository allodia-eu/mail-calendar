# Agent Instructions: Allodia Mail & Calendar

## What this repo is

The whole application: the Rust **product core** and every native client that sits on it. The core
holds product logic, presentation and view-model state machines, host-service ports and the
UniFFI/C-ABI bindings. It sits **above** the product-neutral PIM sync engine, a separate public
repository ([`email-calendar-sync-engine`](https://github.com/allodia-eu/email-calendar-sync-engine)),
and **below** the clients in [`clients/`](clients): Apple (macOS, iOS, iPadOS), Windows, Android and
Linux. Layering, crate map and the dispatch → snapshot loop:
[`docs/architecture.md`](docs/architecture.md).

[`allodia_license/`](allodia_license) is the source-available half, holding the Allodia account and
the paid-capability surfaces. It is excluded from the default build, and the free application never
asks it anything.

## How we write

Docs and comments are read far more often than they are written, and every line costs the reader
attention, human or agent alike. Write the fact, not the story around it.

- **Be brief and concrete.** State the rule, then stop. No build-up, no restatement, no reassurance
  that the rule is a good one.
- **No dash as punctuation.** Not an em dash (—), not an en dash (–), not a spaced hyphen standing
  in for one. Rewrite with a comma, a colon, a semicolon, parentheses or a full stop; which one it
  is, is a judgement about the clause, so no bulk replace does this correctly.

  ```
  ✗ The gate is a no-op — a build without the registration never reaches it.
  ✓ The gate is a no-op: a build without the registration never reaches it.
  ```

  A dash that is not punctuation stays: a range (`A–Z`, `15–120 min`), a minus sign, the cell a
  capability matrix uses for *not applicable*, and every character inside text we quote rather than
  write. A string a test asserts on moves when its test does, not before.
- **Write for a European reader.** Understated, professional, quiet. No superlatives, no drama, no
  pitch, and none of the American editorial register: the one-word paragraph for emphasis, the
  rhetorical question answered on the next line, *incredibly*, *hugely*, *a game changer*. State
  the thing at its true size and let the reader weigh it.
- **British English**, in prose, comments and user-facing copy alike. `-ise` rather than `-ize`
  (*organise*, *authorise*, *recognise*, *normalise*), *behaviour*, *colour*, *centre*,
  *cancelled*, *travelling*.

  Two traps. First, **some pairs split by part of speech**: the noun is *licence* and *practice*,
  the verb is *license* and *practise*, so "the Allodia Licence" but "licensed under the GPL", while
  *licensing*, *licensor*, *relicense* and *sublicense* all keep the s. Second, **identifiers are
  not prose**: an SPDX id, the `LICENSE` file REUSE requires, Cargo's `license` field, the HTTP
  `Authorization` header, iCalendar's `ORGANIZER`, serde's `serialize`, a catalog key like
  `invitation_organizer`. All keep the spelling their spec or tool gave them, as does anything
  quoted from upstream. The rule reaches what we write, not what we have to name.

  ```
  ✗ "Organizer" in a message value          ✓ "Organiser"  (the *key* stays `invitation_organizer`)
  ✗ the license text                        ✓ the licence text
  ✗ licenced under GPL-3.0                  ✓ licensed under GPL-3.0
  ```

  Three words stay American because each is the thing's actual name: **dialog** for the UI element,
  **artifact** for a build output, **catalog** for the inlang message catalog.

  ⚠️ **A doc reference is an identifier wearing prose clothes.** C#'s `<see cref="Maximized"/>`,
  Kotlin's `[authorizationUrl]` and a Rust intra-doc link all name a symbol from inside a comment,
  and renaming one there compiles fine and resolves to nothing. Rust and Swift backtick theirs so
  the eye catches it; C# and Kotlin do not.

  Both rules have checkers (`check_dash_hygiene.py`, `check_british_english.py`) that carry the
  exempt paths, so the tree is clean and an existing spelling or dash is a mistake, not precedent.
  [`docs/privacy-policy.md`](docs/privacy-policy.md) is exempt from both: it is a published
  contract whose text does not move without a version bump, both locales and the website mirror
  travelling together.
- **The code is the source of truth about the code.** Describe what it does *now*. Do not narrate
  what it used to do, which bug changed it, or which PR or issue landed it: git and the tracker hold
  that, and a comment about the past goes stale the moment the code moves again.

  ```rust
  // ✗ `add_account` connected first and registered afterwards, so the first rotation of a newly
  //   added account was dropped (#1234). The headless boot did the same and killed a real grant
  //   twice (#1281).
  // ✓ Every path that connects an account registers its token sink first; a rotation arriving
  //   before registration has no entry to land in and is dropped.
  ```

  History earns a clause only when a reader needs it to act correctly: an external constraint we
  found by hitting it, a trap that will bite again. State it as a present fact.
- **Never name a plan's phases.** `Phase <letter>`, `the <name> wave`, a plan's step ids: none
  belong in code, a comment, a contract doc or a PR description. A phase name is a pointer into a
  document the reader does not have, and it goes stale by design, because a phase ships, is renamed
  or is abandoned and the comment is left describing a future that never arrived.

  ```rust
  // ✗ Display-only in v1 (editable in a later phase of the plan).
  // ✓ Read-only: no client offers to change a reminder yet.
  ```

  `check-public-hygiene.sh` catches the shapes a grep can decide, and the ✗ above is not one of
  them, which is the usual relationship between a rule and its checker. The domain's own word is
  untouched: a gesture's propagation phase and an Xcode build phase are lowercase.
- **Comment sparingly.** A comment earns its place when it carries what the code cannot: a
  non-obvious invariant, an outside constraint (a protocol, a platform bug, a server's behaviour),
  or a "this looks wrong but isn't". Anything a reader gets from the signature and body is noise.
- **A log line is product surface, not a comment.** It describes the user's mail, never our source
  tree: no repo path, doc reference, issue number or internal jargon
  ([`docs/logging.md`](docs/logging.md); `check_log_hygiene.py` catches the machine-decidable part).
- **Say it once.** A rule belongs in exactly one place (this file, a doc under [`docs/`](docs), or
  the code) and everything else links to it. Restating a doc's rules creates two copies that drift.
- **Write it in the repo, never only in an agent memory.** A memory lives on one machine, in one
  tool, for one person, invisible to every other contributor and to the next session. When you catch
  yourself writing "remember that…", ask which file should have told you, and put it there.

## Non-negotiables

- **Sovereignty.** Every external dispatch (AI and model calls, voice, conferencing, connectors)
  must pass an in-process jurisdiction check **before data leaves**: never a perimeter it can route
  around, never air-gapped-exempt. ⚠️ `JurisdictionGate` is the named seam and **is not yet in
  code**, so the rule binds design rather than a call site: nothing new may ship a dispatch that
  would have to route around it. Four carve-outs exist, each stated with the condition that ends it
  in the doc
  that owns it: connecting a mail account
  ([`docs/provider-oauth.md`](docs/provider-oauth.md)), consented analytics
  ([`docs/analytics.md`](docs/analytics.md)), signing in to an Allodia account
  ([`allodia_license/entitlement.md`](allodia_license/entitlement.md)) and the local MCP server
  ([`docs/mcp.md`](docs/mcp.md)), each under "Sovereignty scope". Adding a fifth is a decision, not
  a refactor.
- **Voice.** User-facing copy is clear, plain and anti-hype, and the product is named
  "Allodia Mail & Calendar", never "Allodia" alone.
- **Security.** Encryption at rest, data classification, MFA for privileged access, secrets in the
  platform keystore.

## Product hard rules

- Files stay **under 500 lines**, CI-enforced for every language this repository is written in.
  Split by responsibility; the extensions and the traps are under "Toolchain, lints and CI".
- **Newtypes** for identities; no raw strings where a type prevents mixing ids.
- **Test-first** for behaviour changes. No speculative features, knobs or provider shortcuts.
- **Bug fixes get a regression test** at the lowest shared layer that observes the contract, so
  prefer `mailcal-app` or `mailcal-bindings` for client-visible behaviour. If none is practical, say
  why and record the manual verification path.
- **Consume the engine only through its `engine-api` facade.** Never reach into lower engine crates.
- **Protocol knowledge belongs in the engine: check before you parse.** About to read or write
  protocol bytes here (an iCalendar property, a MIME part, a JMAP or DAV payload, an RFC status
  code)? Search the engine first. A parser here is a second implementation that *will* disagree with
  the one that ships: the engine's is exercised against real servers by live suites, a copy here
  only by its own unit tests, and the copy is what decides what the user sees. The tell is a doc
  comment in this repo citing an RFC section. What *does* belong here is the product decision the
  protocol enables: the engine says `ReplyDelivery::Failed`, this repo decides whether to prompt,
  what to say and what to remember.
- **Debug a live CalDAV server with the engine's `dav-cli`, not a new script.** It drives the real
  adapter, so what it prints is what the core would get. Add a profile for this repo's harness
  rather than expecting the engine to know it: the two harnesses are separate compose projects on
  different ports.
- **The public engine repo stays product-neutral.** No Allodia branding, strategy or roadmap in it.
- **The command/use-case layer is the single surface**; UI, the shipped MCP server and AI
  orchestration are adapters over it. Intents and results stay serde-serializable and
  schema-describable. The schema half is discharged for the **MCP surface only**, whose
  request/result types derive `JsonSchema` beside the deserializer so a published schema and the
  parser cannot drift. It stays open for `Intent`, deliberately serde-free so the FFI enum's shape
  is not pinned by a wire format no client speaks.
- **Keep the README capability matrix current.** It is the at-a-glance truth for what each client
  ships, so a change that shifts a capability's reach updates it too. Internal refactors do not.

## Cross-platform contracts

**A contract is decided once, in its doc, and binds every client.** Each doc states its rules and
carries a per-platform matrix. Add or change a surface a contract covers and you MUST, in the same
change: (1) update that doc's rule **and** its matrix, (2) apply it to **every** platform that ships
the surface (a new platform may not ship the surface until it meets every gate in the contract),
(3) update the README capability matrix if a capability's reach shifted, and (4) write a changelog
fragment if a user could notice. A shortfall goes under that doc's **"Known gaps"**, never left
silent. Two couplings apply to everything user-facing: copy may not out-run the README matrix
(anti-hype), and every privacy claim must match [`docs/privacy-policy.md`](docs/privacy-policy.md).

| Contract | What it decides |
|---|---|
| [`rendering-security.md`](docs/rendering-security.md) · [`composer-security.md`](docs/composer-security.md) | Every gate on untrusted content and on composer hosting: sanitisation, remote-content blocking, script/navigation lockdown, paste handling, bridge and network controls. Raising a gate anywhere raises it everywhere. |
| [`calendar.md`](docs/calendar.md) | Grid semantics: the core emits **unit-free** geometry and a client only multiplies; a page is a **week** and day/3-day/week are zoom levels of one grid; alignment is deliberate, never a side-effect of a zoom; `is_materialized: false` means "we have not looked", not "no events". |
| [`sending.md`](docs/sending.md) | Delivering a message and keeping the sender's copy are two operations, never one transaction: a send is never repeated, the filing alone is retried, and a copy that could not be filed becomes a **standing** question the user can answer, never a silent success. |
| [`search.md`](docs/search.md) | Newest-first ordering (never relevance); default scope is every account and folder except Trash; the scope filter mirrors the mailbox list; leaving search restores the view it opened from. |
| [`folder-pane.md`](docs/folder-pane.md) | Every account's folders on screen at once; expansion is independent of selection, lives in the core and survives a restart; the unread count is the **server's** and counts messages; no badge at zero; All Inboxes sums the inboxes only; icons come from the folder's **role**, in each platform's native set. |
| [`avatars.md`](docs/avatars.md) | The circle beside a person: what it is *of* (the canonical **address**, never the name), monogram then photo but never blank, the colour from a stable hash of the address, hidden from assistive technology, and the raster-only sniff a path must pass before a client sees it. |
| [`reporting.md`](docs/reporting.md) | Marking spam is a **report to the provider**, not a folder move: the report files the message itself, so a client never moves it as well. Which verdicts exist is read from `Capabilities::mail_report` (Gmail has no phishing verdict), and no client may claim the provider acted unless its evidence is `Acknowledged`. A provider that cannot report still gets the message filed. |
| [`contacts.md`](docs/contacts.md) | One person = a shared canonical email, **never** a name; a merged row says it is a merge and names the accounts; read-only this release. |
| [`signatures.md`](docs/signatures.md) | Standalone reusable entities in a named library; two independent slots per account; re-resolved when From changes; sanitised on store *and* submit; `data:` images rewritten to `cid:` on send. |
| [`timestamps.md`](docs/timestamps.md) | Relative label on the list row, full absolute date in the reading header; formatted client-side because the core is tzdata-free; bucket selection is unit-tested. |
| [`logging.md`](docs/logging.md) | The shared `Logger` port: a rotating, size-capped, privacy-safe local log. Counts, ids, durations and events only, **never** mail content, addresses or credentials. |
| [`analytics.md`](docs/analytics.md) | Opt-in, default off, EU-wide; the install id is minted **at consent**; the payload is closed-enum labels so it structurally cannot carry content; withdrawal erases locally and at the backend. |
| [`settings.md`](docs/settings.md) | Which Settings categories exist, in which order, under which names, and what lives in each, so "Settings → Reading" is true on every platform. |
| [`background-sync.md`](docs/background-sync.md) | The `run_background_sync` port: a bounded one-shot pass, the persisted per-account high-water-mark (inbound Inbox only, first-run seeded), and the notification content policy. |
| [`sync-progress.md`](docs/sync-progress.md) | The two things a client may say about mail arriving. A pass the user **awaits** gets the bar, in its own row **below** the list; a pass nobody started gets a **hint** inside a status line the client already draws, never a row of its own, and reaches it only once it has actually committed mail. The same rule decides the **reading spinner**: a client draws it only when the core says an open has outlasted its threshold, never merely because no snapshot has arrived. |
| [`onboarding.md`](docs/onboarding.md) | The first screen that adds a mail account: the Allodia-account recommendation, the sign-in line, the divider and the email-address field, **in that order**. Skipping is one action on that screen; a build with no registration loses items 1 to 3 **together**; the copy may not out-run the README matrix (phone and desktop, never web); the card claims the account list, never mail and never a password. |
| [`account-autodetect.md`](docs/account-autodetect.md) | The strategy set raced in priority order, plus two rules binding every platform: an **untrusted** (non-HTTPS) result needs explicit user approval before a credential is sent, and the **email address never appears in a URL**, only the domain. |
| [`mcp.md`](docs/mcp.md) | Writes go through the same door the user does; reads must not be an `Intent`. Off by default, empty allow list, direct send its own toggle, the known-recipient guard, bodies only from `get_message`, no irreversible primitive. |
| [`branding.md`](docs/branding.md) | The app's name and application id are **injected**, never written in a client: `branding/default.env` is the unbranded default and `branding/allodia.env` overrides it, so removing that one file un-brands every build. Everything named after the id (OAuth redirect schemes, keychain and app groups, the data directory) follows it. |
| [`entitlement.md`](allodia_license/entitlement.md) | What a paid capability may draw, and how a client behaves when it cannot ask: the check is never on a path anyone waits for; a stored answer grants for **30 days** of outage; an explicit *not entitled* takes effect **immediately** while an unreachable service changes nothing; everything else degrades to the free app, never to an error. Governs `allodia_license/` only: the free application has no entitlement and never asks. |
| [`versioning.md`](docs/versioning.md) | One version, from [`VERSION`](VERSION); per-store build numbers are derived, never hand-edited. |
| [`changelog.md`](docs/changelog.md) | Release notes: one fragment per change, per catalog locale, with `Platforms:` and `Bump:`; `release.py` assembles them. |
| [`store-listing.md`](docs/store-listing.md) | The stores' field limits and the rules the copy obeys: one description body written once and reused verbatim across all three stores, only genuinely per-store fields differing, pushed by script and never retyped in a console. The copy itself resolves like the identity files, `branding/<brand>-listing.md` over `branding/default-listing.md`, so an unbranded build still describes itself in a software centre. |

Four obligations that reach outside this repo, or outside the doc:

- **The privacy policy is a published contract.** [`docs/privacy-policy.md`](docs/privacy-policy.md)
  is authoritative, and the page at `allodia.eu/privacy/mail-calendar` renders a **vendored mirror**
  in the website's own repository; never edit it there first. Any change to what the app stores,
  sends or shares bumps the version and date line, and **every catalog locale moves in the same
  change**, because a lagging translation is a *different policy shown to Dutch users*.
  ⚠️ This repo cannot update the mirror, so a PR touching the policy MUST say
  *"⚠️ publish: allodia.eu/privacy needs the matching update"* and is not done until the copy
  matches.
- **[`VERSION`](VERSION) holds the *last released* version, and only a release PR moves it.** A
  feature PR never touches `VERSION`, `Cargo.toml` or `clients/apple/project.yml`, so two PRs in
  flight cannot conflict over it. Cut a release with `scripts/dev/release.py`.
- **A user-facing change writes its changelog fragment in the same change**:
  `docs/changelog/unreleased/<slug>.md`, every catalog locale, with `Platforms:` and `Bump: minor`
  or `patch`, so the semver decision is made while the change is in your head. One sentence, because
  a release assembles every pending fragment into one store field and each note is measured against
  the tightest store its `Platforms:` reach. `store-copy` rejects a bad tag, a missing locale or an
  over-cap note; **nothing can detect that you wrote no fragment.** Internal-only work writes none.
- **User-doc images are in neither repo's git.** `scripts/dev/docs_publish.py` uploads them to the
  website's content-addressed store, and **publishing precedes shipping**: run `--check` before a
  page goes out. The gate cannot, by design, because it needs no network.

## Building & verifying

**Chained work goes in a stack, not in parallel PRs off `main`.** Set the dependent PR's base to
the branch below it (`gh pr create --base <the-branch-below>`), then register the chain in the same
step: `gh stack link <bottom> <next> … <top>`, trunk-most first.
[`.agents/skills/gh-stack`](.agents/skills/gh-stack) has the rest.

⚠️ **A correct `--base` is not a stack.** GitHub does not infer the chain from base pointers, so
until it is told the PRs are separate. The failure is invisible until the first merge: B's diff
silently carries A's commits, both read fine, CI is green on both, then A merges by rebase or
squash and every PR above it conflicts at once in changes their authors never touched. The tell,
before you open it: a "files changed" list holding work that belongs to a different review.

**Google and Microsoft sign-in are build-time injected, and a build without them is legitimate.**
The registrations come from the environment or a gitignored `.env` ([`BUILDING.md`](BUILDING.md));
a build given none drops those two routes from the setup wizard rather than failing. So a missing
`.env` looks exactly like a regression in whatever you are working on: check `oauth_routes()`
before chasing one.

**Run `scripts/dev/gate.sh` before the first push of a branch**: `--clients` adds every client this
host can actually build. CI costs real money and real minutes (macOS runners bill at **10×**), so a
PR is where you *confirm* a green build, not where you discover one.

**On Windows the shell is Git Bash, a prerequisite rather than a preference.** The gate and half
the checks are bash scripts, and Actions' `shell: bash` on `windows-*` runners is Git Bash too, so
the dev box and CI run the same interpreter. Set `MAILCAL_BASH` if yours lives somewhere unusual.

⚠️ **On Windows, a bare tool name is a name Windows also has**, and `System32` answers first.
`bash.exe` there is *WSL's* launcher: `CreateProcess` searches `System32` before `PATH`, so
`shutil.which("bash")` can answer Git Bash while `subprocess.run(["bash", …])` in the same
interpreter gets WSL's, failing in a way that reads as the script under test misbehaving. Resolve
an absolute path through [`scripts/dev/bashtools.py`](scripts/dev/bashtools.py). `convert.exe` is
the FAT-to-NTFS volume converter, and it is worse: `command -v` finds it and `convert -version`
**exits 0**, printing "Invalid drive specification", so neither presence nor exit code is evidence.
[`imagemagick_bin`](scripts/dev/lib.sh) accepts it only when `-version` says ImageMagick.

**Identify a tool by what it says, not by its name.** A probe that stops at `command -v` passes on
Windows and fails on the first real call, several steps later, in whatever it was meant to produce.

**Run the script, never a list assembled from memory**: a half-run gate drops a step, and it is
never a random one. `scripts/dev/gate.sh --list` prints the order, `--keep-going` runs everything
and summarises. The order is cheapest-first rather than CI's, so the step most likely to fail on
your change fails first.

Nothing in it needs a device, simulator, emulator or Docker: the suites that do stay out
(`clients/windows/uitests`, `scripts/dev/test-linux-ui.sh`, `scripts/dev/test-android-native-fault.sh`,
anything behind `scripts/dev/boot.sh`),
because a gate that cannot run is a gate people stop running. `--clients` adds only the headless,
host-appropriate ones and **says what it skipped and why**: a skip that reads like a pass is the
failure this file keeps warning about.

**A skip and a missing tool are different answers, and the gate says which.** A *skip* is a
question this host cannot answer, and the rest of the run is still complete. A tool that is simply
**not installed** is not that: `reuse` and `bun` each guard something nothing else watches, so
their absence means a check silently stopped running and the pipeline finds out instead. The gate
reports those as **NEED**, names the install command, and goes red.

On **Windows**, also run the client's own gates: the workspace gate compiles no C# and draws no
window:

```powershell
clients/windows/build-and-run.ps1 -NoRun        # cdylib -> bindings -> headless gate -> Mailcal.Tests -> app
clients/windows/uitests/run-ui-tests.ps1        # UI Automation assertions against the RUNNING app
```

On **Linux**, with GTK 4.14+ and libadwaita 1.5+ dev packages; other hosts exclude the crate.
[`clients/linux/README.md`](clients/linux/README.md) has the commands and the one-time GNOME
runtime install.

⚠️ **A host build compiles against the distribution's GTK, not the runtime's**, so it proves the
code compiles and its logic holds, not that the toolkit the user gets behaves. What runs against
the shipped runtime is `test-linux-ui.sh` and `build-and-run.sh`, both defaulting to it via
[`scripts/dev/sdk.sh`](scripts/dev/sdk.sh); pass `--host` to the latter for the faster loop.
`cargo fmt` stays on the host either way, because the SDK's Rust is stable and stable rustfmt
silently ignores every nightly option in [`rustfmt.toml`](rustfmt.toml).

`clients/linux/package.sh` is the packaging gate: it builds `--release` in the GNOME SDK, generates
the desktop entry and AppStream metainfo from the resolved listing and [`VERSION`](VERSION), and
validates both against the installed tree. CI runs it on **both** architectures at a tag, never on
a push.

### Gates that stay green over a broken build

If a check cannot fail, it is not a check. Before calling a branch green, ask of each gate: *if this
were broken right now, would this tell me?*

- **`cargo doc` is its own gate.** `cargo test` and `cargo clippy` never invoke rustdoc, and rustdoc
  warnings are denied: a doc link to a **private** item fails while both stay green. A warm
  `cargo doc --no-deps` over this workspace is under six seconds.
- **The composer editor is built from TypeScript, and its bundle is the one generated file we
  commit.** Sources are ESM modules in [`clients/composer/src`](clients/composer/src), which
  `bun run build` inlines into `dist/editor.html`. It is committed because all four hosts load that
  single file, so generating it per build would make bun a prerequisite of cargo, MSBuild **and**
  Gradle. `bun run check` re-derives it and fails when it is stale, which is what buys back the
  guarantee committing it otherwise costs, so never skip that step. Each client's `build-and-run`
  rebuilds it first via [`composer-bundle.sh`](scripts/dev/composer-bundle.sh). Editing
  `dist/editor.html` by hand is always wrong; the next build silently reverts you.
- **Generated bindings are built, never committed**, so a stale local copy after any Rust enum or
  FFI record change, *a merge included*, fails the build for reasons that look nothing like the
  cause.
  Android regenerates via Gradle (`generateUniffiBindings` / `generateL10n`, which is why the android
  CI job installs Rust); other clients via their build scripts. By hand:
  `cargo run --bin uniffi-bindgen -- generate target/debug/libmailcal_bindings.dylib
  --language <swift|kotlin> --out-dir <…>`, then the `mailcal-l10n` generator. C# is a **separate
  generator**: `cargo run -p mailcal-bindgen-cs -- --library <cdylib> --out-dir <…>`, which still
  needs its `--library` flag (UniFFI's own binary auto-detects a library and ignores it).
- **Android tests run on JDK 17**, pinned by `kotlin { jvmToolchain(17) }`. Unpinned, Gradle uses the
  daemon's JDK and Robolectric reads the *host* JDK's locale data: Dutch July is `jul` on 21 and
  `jul.` on 17. Assert that copy is *Dutch*, not *which* Dutch.
- **`swift build` is not the app build.** SwiftPM compiles the module together, so a file missing an
  `import` still builds when a sibling imports the same symbol; xcodebuild's Debug
  `-enable-batch-mode` compiles in isolated batches and fails with `cannot find 'X' in scope`. Verify
  Apple with `clients/apple/Scripts/build-and-run.sh --macos --no-run` (add `--iphone`).
- **A `#![cfg(unix)]` test file reports `running 0 tests ... ok` on Windows**, which reads exactly
  like a pass. Any `cfg(windows)` / `cfg(unix)` branch needs a test *per branch*, and a
  cross-platform test count is not coverage: read the per-file `running N tests` lines.
- **`Mailcal.Tests` cannot link WinUI types** (plain `net10.0`), so an unwired binding, an
  unassigned property and a control opening in the wrong state are all invisible to it. State a rule
  there when it can be stated without WinUI, and in `uitests/` when the thing under test is what the
  user sees. Read [`run-ui-tests.ps1`](clients/windows/uitests/run-ui-tests.ps1)'s header first: the
  dataset choice decides whether your test proves anything.
- **Build on the host's *own* architecture.** Every dual-arch script defaults to it, so passing
  `-Arch` explicitly, copying the CI job's line, is the only way to get this wrong: it recompiles
  the cdylib for a target `cargo test` did not build and **silently drops the gates that must
  execute**. Both arches are needed only for the release `.msixupload`.
- **A store upload is a remote gate**, and a rejection burns a build number and a slow round-trip,
  so assert its invariants locally first. [`package.sh`](clients/apple/Scripts/package.sh) signs
  every nested item and gates on signature consistency;
  [`check-nested-bundles.sh`](clients/apple/Scripts/check-nested-bundles.sh) checks the payload's
  **shape**, not only its signatures, because Xcode creates the directories of a build phase's
  declared `outputFiles` even when the phase exits early, so an empty helper `.app` can ship inside
  the `.ipa` past every signature gate. Both scripts carry the rejection codes they prevent.

### Toolchain, lints and CI

- **Warnings are hard errors, clippy included.** `[workspace.lints]` in the root
  [`Cargo.toml`](Cargo.toml) sets `unsafe_code = forbid`, denies every rustc **and** rustdoc warning
  and denies clippy `pedantic`; the few allowed lints carry their justification there. Write
  `` [`X`] `` doc links only to **public** items. `mailcal-bindings` opts out entirely, because
  UniFFI emits unsafe, undocumented items.
- **The Rust toolchain is pinned** in [`rust-toolchain.toml`](rust-toolchain.toml), which CI parses
  too. Bumping it is a **standalone PR**: a new stable that adds a default-warn lint turns the build
  red, and that PR is where it gets fixed.
- **Format on the pinned nightly**, which [`gate.sh`](scripts/dev/gate.sh) selects for you.
  [`rustfmt.toml`](rustfmt.toml) uses nightly-only options that stable *warns about and ignores*
  rather than failing on, so a floating `+nightly` goes green locally and red in CI. The date lives
  in one file, [`rust-nightly.toml`](rust-nightly.toml), which the gate and
  [`ci.yml`](.github/workflows/ci.yml) both read. Bump the pin and re-run fmt together.
  ⚠️ `cargo +<pin>` auto-installs a missing toolchain **without** rustfmt, then fails with
  `'cargo-fmt' is not installed`, which reads like a bad pin. The gate installs it for you.
- **The 500-line limit** is enforced by
  [`check-file-length.sh`](scripts/ci/check-file-length.sh) in its own always-run job, because no
  linter in these languages has a per-file length lint. It covers `*.rs`, `*.cs`, `*.ts`, `*.js`,
  `*.html`, `*.swift` and `*.kt`. It reads `git ls-files`, so **an unstaged new file is invisible
  to it**: inspect untracked files before calling a change green, and use `git grep --untracked` in
  any new contract check. Generated files are invisible for the same reason and correctly so, which
  is why the 20,000-line UniFFI Kotlin bindings need no exemption. The one `EXCLUDED` entry is the
  committed composer bundle; "it is hard to split" is never a reason to add another.
- **CI builds only what a change can break, and only `main` writes caches.** Jobs gate on
  [`changed-areas.sh`](scripts/ci/changed-areas.sh), which maps the change set onto
  `rust` / `apple` / `windows` / `android` / `linux` and **fails open**, so an unrecognised path
  turns on every area. Add a path to that `case` when you add a directory. Mark **`CI OK`** required
  in the branch ruleset, never the individual jobs: a skipped job reports *no* status, so a
  docs-only PR would wait forever, and `ci-ok` counts a skip as success. Caches save only on `main`,
  since a PR-ref cache is readable by nothing else and evicts main's. **Coverage gap:** the `apple`
  job never links `aarch64-apple-ios`, so a device-only break surfaces at
  [`device.sh`](scripts/dev/device.sh) or at release.
- **The Android and Linux release builds run on a `v*` tag, not on every push**, being the two
  longest steps and each proving a property of the shipped artifact. What that costs between tags:
  nothing in CI reads `proguard-rules.pro`, because the JVM suite runs unminified, so a keep rule
  broken by a dependency bump waits for the tag; and the only remaining check on a dev fixture
  escaping a `cfg` arm is textual
  ([`check-dev-account.sh`](scripts/ci/check-dev-account.sh)), so a leaking *call site* compiles
  fine in debug. Build a release variant by hand before tagging if a change touched shrinker rules
  or the debug-only fixtures.

## Client conventions

- **Localisation is client-side.** The core has no runtime locale facility: it emits
  machine-readable data and owns validation plus the security gates, while each client assembles
  localised copy and formats dates. `mailcal-l10n` is **build-time** codegen from the inlang catalog
  (`messages/<locale>.json`), which the core consumes none of. Localised text baked into a message
  body, such as a reply attribution, is assembled in the client.
- **The catalog is the single source of the language list.** Shipping **en, nl, de, fr, es, it, pt**
  (European pt-PT / es-ES; German formal *Sie*). No client hand-keeps a locale list: `mailcal-l10n`
  emits `L10n.locales` / `LOCALES` / `Locales` / `active_locale` plus `languageName(code)`, and the
  settings pickers, the `MAILCAL_SHOWCASE` check, the Windows `LanguageStore` and `AppCulture` date
  mapping all read those. Adding a language:
  1. `messages/<loc>.json` and the locale in `project.inlang/settings.json`.
  2. A `ShowcaseLocale` variant plus `showcase_data/<loc>.rs` and `showcase_bodies/<loc>.rs` seeds.
  3. The README capability matrix.
  4. Its store translation in the branded listing (`branding/<brand>-listing.md`) and a note in every
     **pending** changelog fragment (released notes are history).
  5. The three screenshot-capture lists, which are deliberately not catalog-driven (a locale is
     admissible only once the showcase **seeds** it): `ALL_LOCALES` in
     [`showcase.sh`](scripts/dev/showcase.sh), `showcase_marker_for` in
     [`lib.sh`](scripts/dev/lib.sh), the `-Locale` `ValidateSet` in
     [`showcase.ps1`](clients/windows/showcase.ps1).

  No **shipped** client code changes. Four guards hold it: codegen fails the build on a missing
  `settings_language_<loc>` endonym ("Deutsch", never "German"); `mailcal-bindings` tests assert
  every seed carries English's keys, folders and bodies **and** that no two locales share a calendar
  event-title list, since parity alone passes a seed copied from English; `AppCulture` resolves
  against the emitted list, so a picker language cannot render its own words over the host's dates;
  and [`check-showcase-flag.sh`](scripts/ci/check-showcase-flag.sh) compares the three lists in step
  5. **The Windows locale list stays its own file** (`L10nLocales.cs`) because `L10n.cs` needs a
  Windows TFM that `Mailcal.Tests` does not have.
- **Android is modern-only.** `minSdk` is **31**; prefer modern APIs, no legacy back-compat shims
  unless asked.
- **The Android client has a JVM test suite: extend it.** `clients/android/app/src/test/` runs under
  Robolectric with Compose's test rule, so `./gradlew :app:test` needs no emulator and gates every
  PR. Nothing there loads the cdylib. Client-side state machines (swipe/undo), the composer seed JSON
  and anything reading the l10n catalog belong here: put the logic in a plain class rather than a
  knot of `remember`s so it can be tested without composing a screen.
- **Platform traps live in [`docs/client-traps.md`](docs/client-traps.md).** Android's WebView
  viewport, an iPad column's own size class, a modal that will not follow the appearance setting,
  the Linux portal launchers, libadwaita markup and focus, and reading a GLib critical. Each is a
  behaviour that looks like our bug and is not, or looks correct and is not. Read it before
  debugging a client; the obvious test does not see most of them.

## Running & debugging a client

**Debug against the local seeded Stalwart server by default, not personal accounts**: it is
deterministic and shared. Skills live in [`.agents/skills`](.agents/skills)
([`.claude/skills`](.claude/skills) is a compatibility symlink), each wrapping a `scripts/dev/*`
script that is human-runnable too. Read the skill before reaching for the script; each carries the
traps that produce a false pass.

| Skill | Reach for it when |
|---|---|
| `mail-harness` | You need a deterministic mailbox. Start, seed, reset, inspect (`harness.sh up [--bulk]`, needs Docker). |
| `debug-app` | Booting, watching or driving a client (`boot.sh`, then `logs`/`screenshot`/`control.sh`). |
| `inspect-mail-store` | The UI and your mental model disagree, and you need to know whether the engine or the view-model is wrong, before reading either. |
| `verify-windows-ui` | Asserting on the running WinUI client. Several automation traps hand you a green assertion for something not on screen. |
| `diagnose-linux-gtk` | A GLib/GTK critical or a stall. Reasoning from the message finds the wrong file; get the backtrace. |
| `ios-device-bgsync` | Background sync or notifications, which a simulator cannot run. |
| `device-interaction` | "It feels laggy", or before claiming any performance win. Needs a physical Android device. |

⚠️ **This repo's harness is a different compose project** (`mailcal-core-harness`, ports
`12xxx`/`28080`) from the engine repo's, because Compose keys a project on its `name:` and both
repos ship a `docker/stalwart/docker-compose.yml`. Sharing one meant `up` adopted the other's
container *and volumes*. When a seed you just added is missing, check `docker ps` for the other
project.

Accounts, debug builds only, via `MAILCAL_DEV_ACCOUNT`, each into an **isolated store**: `stalwart`
(default) is the harness over JMAP; `stalwart-imap` is IMAP for full mail-action and IDLE fidelity
**with the SMTP and CalDAV halves beside it**, which is the split-server shape meeting invitations
break on ([`docs/invitations.md`](docs/invitations.md)); `personal` is the developer's real
accounts. Full loop and per-platform paths: [`docs/debugging.md`](docs/debugging.md).

## Where things live

| Area | Location |
|---|---|
| Building, and the OAuth client registrations a build needs | [`BUILDING.md`](BUILDING.md) |
| Architecture map (layering, crates, ports) | [`docs/architecture.md`](docs/architecture.md) |
| Cross-cutting product contracts | [`docs/`](docs), see the table above |
| Debugging a client against the local mail server | [`docs/debugging.md`](docs/debugging.md) |
| Repo-local agent/debug skills | [`.agents/skills`](.agents/skills) |
| The PIM sync engine | [`allodia-eu/email-calendar-sync-engine`](https://github.com/allodia-eu/email-calendar-sync-engine), pinned by commit in [`Cargo.toml`](Cargo.toml); its own `AGENTS.md` governs engine work |
| Native clients | [`clients/`](clients): `apple`, `windows`, `android`, `linux` |
| Paid capabilities and the Allodia account | [`allodia_license/`](allodia_license), governed by [`allodia_license/entitlement.md`](allodia_license/entitlement.md) |
| Machine-specific build tuning | `AGENTS.local.md`, untracked; one developer's disk and toolchain are not a repo fact |
