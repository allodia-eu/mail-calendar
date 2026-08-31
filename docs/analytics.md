# Consented product analytics: cross-platform contract

**Scope.** How every Allodia Mail & Calendar client reports product usage. We want to know whether
people get an account connected, whether sync is healthy, which surfaces get used, and whether
anyone comes back, and we want to know it **without ever being able to say who**. This is the
**single bar** all clients meet: Apple (macOS, iOS, iPadOS), Windows, Android, Linux, and any future
platform. Adding the surface on a new platform means meeting every cell of the matrix below.

**Principle.** **Consent is the gate, and its absence is a refusal.** Nothing is minted, built, or
sent until the user actively opts in. The payload carries no content, no addresses, and no raw
device model, not by convention but by construction: there is no type that could carry one.

---

## The law, in one paragraph, because it drives the whole design

**ePrivacy Directive Art. 5(3)** is not a data-protection rule; it is a **device-integrity** rule.
It is triggered by the *act* of writing to or reading from the user's device, and the CJEU held in
*Planet49* (C-673/17, ¶70) that it applies **"regardless of whether or not it is personal data."**
So "we anonymised it" is not a defence; it is the wrong axis entirely. Writing a random install id
into our own settings file **is** "storing information in terminal equipment" (EDPB Guidelines
2/2023 ¶36, which expressly covers *"customised software, regardless of who created or installed
the software"*), and reading the OS version and sending it **is** "gaining access to information
already stored" (¶11, ¶39). Neither ePrivacy exemption applies: analytics is not "strictly
necessary" for an email client, and CNIL is explicit that *commercial* necessity is not *strict*
necessity. Some member states (NL, Telecommunicatiewet 11.7a(3)(b), plus FR, ES, IT) have national
analytics carve-outs, **but Germany's §25 TDDDG has none, and neither does Denmark**. We ship
EU-wide, so the strictest member state sets the design: **opt-in consent, everywhere, one
behaviour.** Consent cannot be bundled into terms and conditions (GDPR Art. 7(2), Recital 43; EDPB
Guidelines 05/2020: bundling is presumed *not* freely given).

**Sovereignty scope.** Telemetry dispatches **only** to Allodia's own self-hosted relay, which is
`eu-native` by construction in every jurisdiction mode, so there is nothing for the
`JurisdictionGate` to decide and it would be a no-op. This is a deliberate, ratified carve-out in
the style of the 2026-07-02 account-connection one; see [`../AGENTS.md`](../AGENTS.md). A build with
no relay baked in (every local build) sends nothing at all, and an air-gapped or self-hosted
deployment behaves identically to a connected one.

---

## The port (shared) · `crates/mailcal-app/src/telemetry.rs` + `crates/mailcal-telemetry`

The core owns consent, the install id, and the payload. `mailcal-telemetry` is the only thing that
can put an event on a network, which is why the demo, the showcase, and **every test** run with no
sink at all and therefore *cannot* phone home.

- **The consent gate is one function.** Every emit path goes through `App::track`, which returns
  early unless consent is live. There is no second way to send.
- **The install id is minted at consent, not at first launch.** A user who never opts in has
  *nothing* on disk for analytics, so the Art. 5(3) storage event only ever happens after they say
  yes. Withdrawal deletes the id locally **and** asks the relay to erase everything held under it
  (GDPR Art. 17), which is only possible *because* the id is stable.
- **A stale notice re-asks.** Consent is recorded against a `NOTICE_VERSION` (**currently 2**,
  bumped from 1 when the Google provider added `has_google` to the account-mix, widening the
  payload). Materially widening what we send bumps it, which reads back as *unasked*, so a new
  payload can never inherit a consent that was given for less. A **decline**, by contrast,
  survives a bump: we asked once and were told no.
- **The payload cannot carry content.** Every property value is the label of a closed enum, a
  bucket, or a string that has been through a reducer. There is no variant that could carry a
  subject, an address, a folder name, a search query, or a hostname. Widening what we send means
  adding an enum variant, which forces a key into `PROPERTY_KEYS` **and** into the relay's ingest
  whitelist: two deliberate edits in two repositories.
- **The core coarsens; the client does not.** A client reports its raw OS version and locale; the
  core reduces them (`15.4.1` → `15`, `nl-NL` → `nl`). One tested rule, six client targets, and no client
  can widen the payload by reporting something more precise than we asked for.
- **Delivery is best-effort and bounded.** Never blocks, bounded in-memory queue, drops on overflow,
  **never persists to disk**, one attempt then done. No retry storm on a flaky network; no analytics
  data at rest to leak.

### What we send

| Field | Value | Why |
|---|---|---|
| `install_id` | random 128-bit, minted **at consent** | the only stable identifier; the sole reason the consent gate exists |
| `platform` | `macos` \| `ios` \| `ipados` \| `windows` \| `android` \| `linux` | n/a |
| `os_version` | **major only**: `15`, `11` | a build number is a strong identifier and answers nothing a major does not |
| `device_class` | `iphone` \| `ipad` \| `mac-laptop` \| `mac-desktop` \| `pc` \| `android-phone` \| `android-tablet` \| `linux-desktop` | **a class, never a model string** (see below) |
| `app_version` | `1.4.0` | tells us when an old version can be dropped |
| `locale` | `en` \| `nl` \| `other` | do we need more languages |
| `account_count` | `0` \| `1` \| `2` \| `3-5` \| `6+` | a raw count is high-entropy at the tail |
| `protocols` | **unordered** `has_imap` / `has_jmap` / `has_graph` / `has_google` | never an ordered per-account tuple |

**Why no raw device model.** `MacBookPro18,3` or `SM-G991B` is the strongest identifier an otherwise
low-entropy payload could carry: with a few thousand installs, a rare model paired with a rare
account mix is plausibly one identifiable person. The **app stores already report exact models to us
for free**, at higher fidelity than a consented subset would give, so we lose nothing. A class is
what actually drives a decision ("does the tablet layout matter?"); the model string never did.

### Events

`app_opened` · `settings_snapshot` · `setup_started` / `setup_completed` / `setup_failed` (per
protocol) · `feature_used` · `sync_completed` / `sync_failed` (per protocol).

**Never sent:** anything from the mailbox: subjects, senders, recipients, folder names, message
counts, search queries, calendar titles, attachment names. The relay's closed key whitelist enforces
this at the boundary, not just by convention, and `tests_telemetry.rs::no_content_reaches_the_wire`
drives a consented app through the intents that *carry* that content and asserts none of it reaches
the wire.

---

## The shared bar

Every client meets all of these:

- **Opt-in, default OFF.** No pre-ticked box, no implied consent, no "by continuing you agree".
- **Unbundled.** Never part of accepting terms, creating an account, or completing setup.
- **Asked once, on the welcome screen at first boot**: the first thing a new user sees, *ahead* of
  account setup, and **before** the OS notification prompt so no two asks stack. A decline is
  remembered; we do not ask again. Asking before setup rather than after keeps the question
  structurally unbundled: the screen it lives on accepts no terms and creates no account.
  The gate is `analytics_consent().asked`, which also covers a **returning** user upgrading into a
  build that asks: they have accounts already, so no setup transition would ever fire for them.
- **Nothing is written until the user leaves the screen.** Moving the switch does not persist
  anything; the decision is taken once, on "Get started". Under Art. 5(3) the *act* of storing the
  install id is what needs consent, so it may not precede the affirmative action.
- **"See exactly what we send"**: the consent screen shows the literal JSON from
  `analytics_payload_preview()`, built from the *same type* the sink serializes, so what the user
  reads is what actually goes on the wire. **This holds in a build with no relay too**
  (`Telemetry::unsent`, not `Telemetry::off`): a build that cannot send still knows its device, and
  a hollow preview (`os_version: "0"`, `device_class: "unknown"`) would be a lie told on the one
  screen whose whole purpose is not to tell one.
- **Refusal costs nothing.** The app is fully functional; "Get started" is always enabled and is the
  only way forward, so the switch is genuinely optional rather than a toll gate.
- **Withdrawable in one click**, in Settings → Privacy, as easily as it was given (GDPR Art. 7(3)).
  Withdrawal deletes the install id *and* the consent timestamp from the device, and asks the relay
  to erase what it holds (Art. 17).
- **EN + NL from day one**, via the shared `mailcal-l10n` catalog, including the privacy-policy URL,
  so all clients point at one place and a localised policy page can diverge later.
- **Never the word "anonymous."** It is *pseudonymous* usage data. Mislabelling it is itself an
  Art. 5(1)(a) transparency problem. Say "usage data" or "privacy-preserving usage statistics".
  Pinned by a test in both locales (`WelcomeScreenTest`), because it is exactly the word a
  well-meaning edit reaches for.

---

## Boot-time prompts: the pattern this sets (and the rule that binds it)

The welcome screen is the first thing we show a user *because a stored answer says we owe them a
question*. There will be more of these: a privacy-policy change that needs re-consent, a "what's
new" screen worth one boot. Write down the pattern while there is exactly one consumer, and the
rule that keeps the next one legal.

**How an existing install experiences this feature.** An upgrade is not exempt from the ask: a
returning user's `preferences.toml` has no `analytics_consent` key, an absent key reads back as
*unasked*, and the welcome screen shows **once** on the first boot after upgrading. That is the
design, not an accident: the screen answers "is this question settled?", never "is this install
fresh?", and for an existing user the consent question genuinely is unsettled. Any answer settles it
for good.

**Re-consent is already built.** Consent is recorded against `NOTICE_VERSION` (see "A stale notice
re-asks" above): bump it and every stored *yes* reads back as unasked, while a *no* stays a no. A
privacy-notice change that materially widens what we send is a `NOTICE_VERSION` bump, not a new
mechanism.

**The pattern for the next boot-time prompt.** One per-concern key in the core's `preferences.toml`
(versioned if the content can change, e.g. `whats_new_seen_version: Option<u32>`), one core-side
decision function that reduces it to an `asked`-style flag, and every client pulls that flag at boot,
exactly the shape of `analytics_consent().asked`. The **decision lives in the core** so clients
cannot drift on when to show a screen, and a client that forgets to ask fails *closed* (for
consent: nothing is sent). Do **not** build a generic prompt framework until a second consumer
exists; the second consumer's shape should design it.

**The rule that binds it: a consent screen shares its boot slot with nothing.** A consent ask is a
*gate*: it must accept no terms, create no account, promote no feature, and cost nothing to refuse
(Art. 7(2): bundled consent is presumed not freely given, the same reason the welcome screen sits
*ahead* of account setup). A feature promo is dismissible marketing. They may share the persistence
pattern above; they may **never** share a screen. If a future release owes the user both a re-consent
and a what's-new, show them as separate steps with the consent first, alone.

---

## Per-platform implementation matrix

| Aspect | Apple · `DeviceFacts.swift` | Windows · `Services/DeviceFacts.cs` | Android · `DeviceFacts.kt` | Linux · `boot.rs` |
|---|---|---|---|---|
| Device facts | `ProcessInfo` + `UIDevice.userInterfaceIdiom`; laptop/desktop from **whether there is an internal battery** (`IOKit.ps`); **no model string is read at all** | `Environment.OSVersion` (build ≥ 22000 → `11`); no model read at all | `Build.VERSION.RELEASE` + `smallestScreenWidthDp` ≥ 600 → tablet; **no** `Build.MODEL` | ✅ `/etc/os-release` `VERSION_ID`; fixed desktop class; **no model string is read** |
| Reported at | `MailcalApp.newAccounts` / `newBackgroundWorker` | `MailcalApp.NewAccounts` | `MailcalApp.newAccounts` / `newBackgroundWorker` | ✅ `MailcalApp::new_accounts` |
| Welcome / consent screen | ✅ `WelcomeView.swift` (shared macOS + iOS + iPadOS) | ✅ `Views/WelcomeView.xaml` | ✅ `WelcomeScreen.kt` | ✅ `ui/welcome.rs`, ahead of setup |
| Settings → Privacy toggle | ✅ `AnalyticsSettings.swift`, mounted by `SettingsView` (macOS) and `SyncSettingsView` (iOS) | ✅ `Dialogs/SettingsDialog.Privacy.cs` | ✅ `AnalyticsConsentUi.kt`, mounted by `SettingsScreen.kt` | ✅ `ui/settings/pages.rs` |
| Payload preview panel | ✅ `AnalyticsPayloadPanel` (shared by both surfaces) | ✅ inline reveal (a nested `ContentDialog` is not allowed) | ✅ `AnalyticsPayloadPanel` | ✅ modal, shared core JSON |
| Launch reported | ✅ `reportAppOpened()` in `MailcalModel.connect` | ✅ `ReportAppOpened()` in `ConnectAsync` | ✅ `reportAppOpened()` in `connect()` | ✅ after stored or first-run choice |
| Consent storage | core `preferences.toml` (shared, no per-client store) | same | same | same |
| Driven end to end | ✅ macOS (AX API) · iOS + iPadOS (`idb`, simulator) | ✅ Windows 11 arm64 (UIA) | ✅ emulator + 60 JVM tests | ✅ GNOME 50 runtime (AT-SPI) |

Every shipping platform has been driven through the whole lifecycle against the Stalwart harness:
unasked → payload preview → opt in (mints the id) → withdraw (deletes it), and the payload reports
the right platform and class (`macos`/`mac-laptop`, `ios`/`iphone`, `ipados`/`ipad`,
`windows`/`pc`, `android`/`android-phone`). The Art. 5(3) property was checked directly rather than
inferred: with the switch **on** but "Get started" not yet pressed, `preferences.toml` still holds
**zero** analytics keys. Linux's AT-SPI run proves the same lifecycle, including the install id
appearing only after "Get started" and disappearing again on withdrawal.

**Why the laptop/desktop split is not `hw.model`.** The obvious test (does the model identifier
start with `MacBook`) is **broken on every Apple Silicon Mac**: they report `Mac14,15`, `Mac15,3`
and so on, and only the Intel models were ever named `MacBookPro18,3`. It silently classified an M2
MacBook Air as a desktop. A battery is what "laptop" has actually meant all along, and reading for
one means no model string is read at all, a strictly better privacy posture than the rule it
replaced.

**The gate is in the core, not in the UI.** A client that forgets to show the screen sends nothing,
because `App::track` returns early unless consent is live. The demo and the showcase have no
preferences store at all, so they report the question *settled* and never raise the screen, which
is also why no screenshot or UI-automation run has to click past it.

---

## The store privacy declarations

Every store asks the same questions about this payload: Apple's **App Privacy**, Google Play's
**Data Safety**, and Partner Center's privacy fields. They get the **same answers**, because they
are describing one payload. Recorded here rather than re-derived per store, and rather than left in
a console where the person editing `PROPERTY_KEYS` will never see it.

⚠️ **This payload is not the whole label.** A build carrying an account sign-in has a second thing
the user can send, it is **linked** to them where all of this is not, and it has to be declared
too. Answering a store from this table alone under-declares such a build. The rows below are the
analytics rows; the listing a publisher submits from holds both sets and is what a submission is
checked against ([`store-listing.md`](store-listing.md)).

**Apple: App Store Connect → App Privacy**, as this payload alone would be answered:

| Question | Answer |
|---|---|
| Do you collect data from this app? | **Yes** |
| Data types | **Identifiers → User ID**; **Usage Data → Product Interaction**; **Diagnostics → Performance Data**; **Diagnostics → Other Diagnostic Data** |
| Purpose (all four) | **Analytics** only |
| Linked to the user's identity (all four) | **No** |
| Used for tracking (all four) | **No** |

⚠️ **The submitted `User ID` row says *linked*, and that is not a contradiction.** Apple takes one
linked answer per data *type*, and since 0.6.0 that type carries the Allodia account id as well as
this install id. A type holding both can only be declared at the stricter of the two, so the row
goes out as linked and as **App Functionality**, not Analytics; the submitted set and the
reasoning are recorded with the listing that was submitted. Nothing below changes: the install id
is still not linked to anyone, and the day the account rows leave the label this table is again the
whole answer.

Which maps onto the wire payload as: `install_id` → User ID; `app_opened` / `feature_used` /
`setup_*` / `settings_snapshot` / the account mix → Product Interaction; `sync_completed.duration`
→ Performance Data; `setup_failed` / `sync_failed` counts and the `os_version` / `app_version`
context → Other Diagnostic Data.

Four judgment calls, so nobody has to make them twice:

- **User ID, not Device ID.** The install id is minted at consent, is deliberately not derived from
  the device, and does not survive withdrawal or reinstall. "Device ID" would claim a device-level
  identifier we specifically do not have; see `DeviceClass`'s comment on why no model string is
  read.
- **Analytics, not App Functionality**, even though Apple's App Functionality wording includes
  "improve scalability and performance". Nothing here serves the app's operation: `App::track`
  returns early unless consent is live, so the app behaves identically for every user who declines.
  Claiming App Functionality would assert the payload is needed to make the app work, which is both
  untrue and in tension with the opt-in model.
- **Not linked to identity, even though §5 of the privacy policy calls the install id personal
  data.** These answer different questions. GDPR's bar is *identifiability*, which a persistent
  pseudonymous id clears by singling-out alone; Apple's bar is whether **we** can tie the record to
  a person, and we cannot; the policy says so itself in §11: *"an install id doesn't tell us who
  you are, so we can't look yours up from your name or email."* Apple's label has a "Data Not Linked
  to You → Identifiers" bucket precisely for this case. **Do not "fix" the policy to match** by
  calling the id anonymous: that would be wrong under GDPR (Recital 26) and would trade a correct
  legal posture for a cosmetic consistency. §5 is the more cautious statement of the two; it stays.
- **No tracking, and no IDFA.** There is no `NSUserTrackingUsageDescription`, no
  `AppTrackingTransparency` import, and no `advertisingIdentifier` / `ASIdentifierManager` reference
  anywhere under `clients/apple/`, so no ATT prompt, and no Device-ID-for-tracking declaration is
  owed. Adding any of those would make this label false.

The resulting product-page label is **Data Not Linked to You: Identifiers, Usage Data,
Diagnostics**.

**The declaration was answered forward-looking, and the build has now caught up with it.** It was
written while `ALLODIA_TELEMETRY_URL` was baked into no build, so the label described collection the
binary could not perform: over-declaring is not a violation, under-declaring is, and it meant the
relay could ship without a store-metadata round trip. A build we ship now carries the endpoint or
fails to compile, so the label and the artifact agree. Nothing here needs changing; it is recorded
because "the declaration is ahead of the build" was true for a release cycle and should not be
re-derived as a discrepancy.

**When this needs revisiting:** adding a `PROPERTY_KEYS` entry that introduces a *new data
category* (anything beyond usage/diagnostic labels: a free-form field, a location, an address)
changes the answers above, not just `NOTICE_VERSION`. App Privacy is editable in App Store Connect
at any time, so the fix is cheap, but it has to land **before** the build that widens the payload,
not after.

---

## Known gaps / follow-ups

- **The relay's key whitelist is unverified from here.** The relay is live, and a build we ship
  now carries its endpoint or does not compile (`MAILCAL_REQUIRE_INJECTED_CONFIG`, `BUILDING.md`).
  What that proves is delivery: a batch was accepted, and a withdrawal erased what was held. It does
  **not** prove the boundary check, because a relay with no whitelist at all answers a good payload
  identically; the enforcement is only observable by sending a key that should be refused, which
  this side structurally cannot emit. Its ingest whitelist must include `has_google` (added here
  with the Google provider, `NOTICE_VERSION` 2), the second of the two-repo edits called out under
  "The port". A build with no endpoint stays silent and supported; that is every from-source
  checkout.
- **Setup and sync failures are counted but not classified.** We report *that* a JMAP setup failed,
  not *why*. The engine classifies provider failures with a `FailureClass`, but that type lives in
  `engine-core` and the product core consumes the engine only through the `engine-api` facade
  ([`../AGENTS.md`](../AGENTS.md)); `ApiError` cannot tell a revoked credential from a dead DNS
  lookup. Deriving a class from the error *string* would be brittle and would put us one engine
  message change away from leaking the user's host or username. **Follow-up: re-export
  `FailureClass` from `engine-api`**: that is what unlocks the *why*, which is the most valuable
  thing this feature could tell us.
- **Swipe adoption and attachment-opens are not counted.** Only the client knows they happened: a
  swipe reaches the core as a plain Delete/Archive/Flag, and opening a received attachment never
  reaches the core at all. Rather than widen the FFI so clients can report them, we take the proxy
  we have (the settings snapshot says which swipe actions were *configured*). Revisit if the proxy
  proves insufficient.
- **Crash-free rate is not reported.** A crash reporter is a different pipeline: the app is dead,
  there is nothing to `track()`. The app-store dashboards give us crash data for free today.
- **The store dashboards are not wired up.** App Store Connect, Play Console and Partner Center
  report installs, active devices, retention, OS version, **device model** and crashes for free, with
  no code and no consent needed from us. They are the intended source for everything the consented
  payload deliberately omits. Turn them on.
- **A privacy policy is legally mandatory; it is written but not yet live.** GDPR Art. 12–14, and
  all three app stores block a release without one. [`privacy-policy.md`](privacy-policy.md) is
  final (v1.4), and the welcome screen links to `https://allodia.eu/privacy/mail-calendar` (from
  the l10n catalog); that page goes live when the website repo's vendored copy is synced. It must
  resolve before any telemetry ships.
- **The Apple welcome screen's "Get started" is the default action** (⏎ dismisses it). That is the
  safe direction: a stray Return can only ever *decline*, never opt in, since the switch is off
  unless deliberately moved, and the choice is reversible in Settings → Privacy. Worth revisiting if
  it proves to cost real opt-ins.

---

## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change analytics:

1. Update this document (the rule **and** the matrix) **and** the capability matrix in the
   same change.
2. Keep the policy identical across every existing platform: opt-in, default off, unbundled,
   one-click withdrawal, the payload preview, and the never-send-content rule.
3. **Widening what is sent bumps `NOTICE_VERSION`**, which re-asks. A payload may never grow under a
   consent that was given for less. Add the key to `PROPERTY_KEYS` **and** the relay whitelist, or it
   is rejected at ingest.
4. A new platform ships the consent screen, the Settings toggle, and the payload preview **before**
   it ships to users; any shortfall goes under "Known gaps" with a follow-up, never left silent.
5. **A new data *category* re-opens the store declarations** ("The store privacy declarations"
   above): Apple's App Privacy, Play's Data Safety, Partner Center. Update them in the same change
   and land the edit **before** the build that widens the payload, not after.
