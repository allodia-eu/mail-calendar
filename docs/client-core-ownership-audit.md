# Client-to-core ownership audit

**Date:** 2026-08-02

**Status:** research only; no migration has been implemented

**Scope:** the shared Rust crates, UniFFI/C-ABI boundary, Apple, Android, Windows and Linux clients,
the shared composer asset, client tests, and the cross-platform contracts under `docs/`

## Executive result

The repository's intended architecture is clear: product logic and presentation state machines live
in Rust, while clients render native UI and provide OS services. The core already does this well for
large parts of mail, calendar projection, search, contacts, signatures, analytics, background sync,
and content security. Recent client growth has nevertheless created several substantial islands of
duplicated domain logic.

The best migration candidates are not pixel layout or platform API wrappers. They are coarse,
testable semantic units that currently have copies in three or four clients:

1. Make credential stores mandatory before interactive boot and let Rust coordinate account
   connect/persist/remove transactions.
2. Make autodetection approval an enforceable Rust capability rather than a Boolean interpreted by
   every client.
3. Give the reading surface a complete account-scoped session identity and latest-request-wins state
   machine.
4. Move the calendar event editor's validation, defaults, normalisation, and command construction to
   Rust.
5. Centralize recipient-field transformations and final composer-envelope validation.
6. Introduce a per-composer Rust session for From-account and signature semantics, then replace
   hand-built quote/signature seed JSON with typed builders.
7. Move or share the swipe/undo reducer, semantic calendar navigation, and asynchronous search
   coordination, while leaving gestures, timers, pixels and input mechanics native.

Several include concrete correctness or security exposure, not just code duplication. In particular,
an OAuth token can currently rotate while the interactive app still has empty credential sinks, an
untrusted autodetection result can reach the raw setup path without approval provenance, and a
reading body is matched by provider key without its account even though that key is only
account-scoped.

This audit recommends moving **semantic state and transactions**, not increasing per-frame FFI
traffic. The target shape is a Rust session/reducer that returns small immutable snapshots and typed
effects; the client retains native controls, accessibility, localisation, scheduling, browser and
keystore APIs, WebViews, and frame-sensitive gesture/rendering state.

## Method and coverage

The audit began from the ownership rules in [architecture](architecture.md) and the feature
contracts under `docs/`. It then traced client
behaviour back through generated-API call sites to `mailcal-bindings`, `mailcal-app`,
`mailcal-viewmodel`, `mailcal-account`, `mailcal-autodetect`, `mailcal-oauth`, and
`mailcal-composer`.

The production client inventory reviewed, **as of the audit date above**, was approximately:

| Area | Production files | Approximate source lines |
|---|---:|---:|
| Android Kotlin/Java | 84 | 18,152 |
| Apple Swift | 94 | 15,260 |
| Windows C#/XAML | 155 | 24,853 |
| Linux Rust | 28 | 6,125 |
| Shared composer page | 1 | 1,108 |

All 12 top-level crates, all five client roots, relevant unit/UI tests, host-service ports,
and the governing feature documents were included. Generated bindings, build output, vendored
dependencies, packaging boilerplate, and store copy were inventoried but not treated as candidates
for ownership migration. This was a static ownership audit: no app behaviour or implementation code
was changed; this report is the only added artifact.

Candidates were ranked using five questions:

- Is the behaviour a product/security invariant rather than a rendering choice?
- Does it have multiple independent implementations or client tests?
- Can drift cause lost data, leaked credentials, incorrect dispatch, or silent UI disagreement?
- Can it cross FFI at event/session granularity rather than once per frame or keystroke?
- Would Rust become the authoritative state, rather than merely adding another helper beside client
  state?

The repeated files alone are sizeable: the four event-editor implementations total 1,143 lines, the
three swipe/undo reducers 512, the three recipient helpers 309, the two DNS codecs 375, the three
calendar paging helpers 714, and the three invitation presenters 863. Raw line count is not the
reason to migrate them, but it corroborates that these are maintained subsystems rather than a few
native formatting branches.

## Priority map

| Priority | Candidate | Main reason | Recommended Rust home | Migration risk |
|---|---|---|---|---|
| P0 | Credential-store injection at boot | Rotated OAuth token can race empty sinks | `mailcal-bindings` / `mailcal-account` | Medium |
| P0 | Detected-setup security session | Approval is advisory and provenance is lost | `mailcal-autodetect` + bindings | Medium-high |
| P0 | Account credential transaction | Connect/persist/remove are split across UI and core | `mailcal-account` + host port | Medium-high |
| P0 | Account-scoped reading session | Wrong/stale body can match a same-key message in another account | `mailcal-viewmodel` / `mailcal-app` | Medium |
| P1 | Event-editor domain model | Four copies of validation and time semantics | `mailcal-viewmodel::calendar` | Medium-high |
| P1 | Recipient reducer + envelope validation | Three copies drift from the core send parser | `mailcal-composer` / `mailcal-app` | Low-medium |
| P1 | Composer session and seed builders | From/signature/quote rules are repeated and schema-stringly | `mailcal-composer` / `mailcal-app` | Medium-high |
| P1 | Swipe/undo reducer | Three copies of a correctness-sensitive optimistic transaction | `mailcal-viewmodel` / `mailcal-app` | Medium-high |
| P1 | Calendar semantic navigation | Three copies of load-bearing civil-date rules | `mailcal-viewmodel::calendar` | Medium |
| P1 | Latest-wins async coordination | Search and probes can complete out of order | `mailcal-app` | Medium-high |
| P2 | DNS wire codec | Portable security parser duplicated in Swift/Kotlin | `mailcal-autodetect` | Medium |
| P2 | Enriched snapshots/catalogs | Client joins and mirrored query state can be incoherent | view-model + bindings | Low-medium |
| P2 | Semantic invitation/event summaries | Repeated classification, not native formatting | `mailcal-viewmodel` | Low |
| P2 | Stable typed row identity | Clients invent delimiter encodings | view-model + bindings | Low-medium |
| Contract review | Native log sink, overflow helpers, seed ownership | Useful sharing exists, but current contracts explicitly assign ownership | separate reusable helper | Varies |

## P0: correctness and security hardening

### 1. Require credential stores before interactive boot can connect

This is the most immediate concrete defect found by the audit.

`MailcalApp::new_accounts` constructs the app with empty credential stores in
[`app_accounts.rs`](../crates/mailcal-bindings/src/app_accounts.rs), while interactive boot starts
`retry_connections()` before the constructor returns in
[`boot.rs`](../crates/mailcal-bindings/src/boot.rs). The clients install their Microsoft, Google and
JMAP stores only afterward; Windows does so in
[`MailboxModel.Accounts.cs`](../clients/windows/Mailcal/Services/MailboxModel.Accounts.cs), with
equivalent post-construction setters in Apple and Android. By contrast, the background worker
already requires all credential stores up front in
[`background_sync.rs`](../crates/mailcal-bindings/src/background_sync.rs), precisely because losing
a rotated refresh token can invalidate an OAuth grant.

A boot-time refresh can therefore race an empty sink. This is not primarily a “thin client” concern:
it is a lifecycle invariant that the Rust constructor should make impossible to violate.

Recommended boundary:

```text
new_accounts(..., credential_store: Arc<dyn AccountCredentialStore>)
```

As an immediate hardening, make the existing stores required constructor arguments. As a follow-on,
replace the three provider-shaped but behaviorally identical store traits with one account/provider
keyed port. Keychain, Keystore, Credential Manager, encryption, chunking and access-control policy
stay native; Rust owns when a credential must be durably written.

### 2. Make detected-setup approval an enforceable Rust session

The autodetection core returns a recommendation and `is_trusted`, but the FFI contract in
[`autodetect.rs`](../crates/mailcal-bindings/src/autodetect.rs) says the host **must** enforce
approval. Each client then independently calculates trust, approval, readiness, CalDAV defaulting,
manual fallback, and the final setup payload:

- Android: [`AccountSetupDetect.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/AccountSetupDetect.kt)
- Apple: [`AccountSetupDetectView.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/AccountSetupDetectView.swift)
- Windows: [`AccountDetectForm.cs`](../clients/windows/Mailcal/Services/AccountDetectForm.cs)

The final bindings in [`setup.rs`](../crates/mailcal-bindings/src/setup.rs) accept a new raw
`AccountSetup`/`JmapSetup`; detection provenance and the approval decision are gone. A buggy or stale
host can therefore send a credential to an untrusted detected endpoint without the core being able
to tell. Client validation already differs at the edges: for example, JMAP email readiness is not
identical across Apple and Windows.

Recommended boundary:

```text
detect(email) -> DetectedSetupSession
session.set_approval(bool)
session.set_calendar_enabled(bool)
session.apply_allowed_overrides(...)
session.snapshot() -> DetectedSetupSnapshot
session.connect(secret) -> Result<AccountId, SetupError>
```

The session should bind approval to the exact detected fields and invalidate it when a
security-relevant field changes. It should own route selection, trusted/untrusted status, CalDAV opt-out,
normalisation, readiness and config construction. The native client still owns the warning UI,
checkbox, progress presentation, password field, system browser and manual-setup form.

The same session can later absorb the duplicated JMAP probe generation/stale-result logic, but the
security capability should not wait for a complete onboarding redesign.

### 3. Let Rust coordinate account connect, persist, rollback and removal

Every client manually sequences a connected account and its OS credential record. Apple performs
connect-then-Keychain writes across `MailcalModel.swift` and provider extensions, Windows does so in
[`MailboxModel.Accounts.cs`](../clients/windows/Mailcal/Services/MailboxModel.Accounts.cs), and
Android does so in `MainActivityCore.kt`/`SecureStore.kt`. Removal similarly makes separate core and
vault calls. The account API explicitly delegates persistence back to the host.

This creates failure windows in both directions: an account may be live without durable credentials,
or a vault entry may survive a failed/removed account. Some native stores are intentionally
best-effort and cannot communicate enough outcome for a coordinator split across UI code.

Recommended Rust transaction:

```text
add_account(setup, secret) = validate -> connect -> persist -> publish
remove_account(id)         = disconnect/remove -> erase secret -> publish
refresh_token(id)          = rotate -> persist before old grant is discarded
```

The port should return meaningful success/failure so Rust can roll back or expose a recoverable
state. Initial OAuth credentials should not round-trip through view code merely so the view can put
them back into a store. This work naturally follows constructor injection, but is separable from the
autodetection session.

### 4. Make reading identity account-scoped and latest-request-wins

The current `ReadingSnapshot` in
[`reading.rs`](../crates/mailcal-viewmodel/src/reading.rs) and its binding in
[`records.rs`](../crates/mailcal-bindings/src/records.rs) contains a provider message key but not its
account. Provider keys are only unique within an account. Clients keep a separate `OpenedMessage`
header/session and match an arriving body to it by key alone:

- Android: `ReadingScreen.kt`, `MainActivityCore.kt`, and [`QuoteSeed.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/QuoteSeed.kt)
- Apple: `ReadingView.swift`, `MailcalModel.Actions.swift`, and [`QuoteSeed.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/QuoteSeed.swift)
- Windows: `MailboxModel.Reading.cs`, [`ReadingSelection.cs`](../clients/windows/Mailcal/Services/ReadingSelection.cs), and [`ReadingAdvance.cs`](../clients/windows/Mailcal/Services/ReadingAdvance.cs)
- Linux: `ui/model.rs`, `ui.rs`, and [`composer_model.rs`](../clients/linux/src/ui/composer_model.rs)

Opening two accounts whose providers use the same key can leave the key unchanged and accept the
wrong prior body or quote source. Independently, `dispatch` is fire-and-forget, so two rapid opens
can overlap and the older completion can publish after the newer request.

Recommended snapshot:

```text
ReadingSessionSnapshot {
    state: Closed | Loading | Ready | Failed,
    identity: MessageIdentity { account_id, provider_key },
    subject,
    sender,
    received_at_utc,
    body,
    remote_content_allowed,
    generation
}
```

Opening should publish `Loading` immediately with the complete identity and header, and only the
current generation should be allowed to become `Ready` or `Failed`. Native code still localises the
timestamp and sender presentation and hosts the hardened WebView. Desktop-only auto-advance can be a
separate opt-in pure helper over structured visible identities rather than forcing mobile navigation
into the shared state machine.

## P1: high-leverage shared domain models

### 5. Calendar event-editor draft and command builder

The same editor domain model exists four times:

- Android [`EventEditor.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/EventEditor.kt)
- Apple [`EventEditorState.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/Calendar/EventEditorState.swift)
- Windows [`EventEditorState.cs`](../clients/windows/Mailcal/Calendar/EventEditorState.cs)
- Linux [`editor.rs`](../clients/linux/src/ui/calendar/editor.rs)

These are not primarily widget code. They decide next-whole-hour defaults, create versus edit
editability, required title/range validation, all-day inclusive UI end versus exclusive stored end,
device-zone create versus event-zone edit, timed/all-day conversion, optional-field normalisation,
frozen account/calendar/zone fields on edit, reminder buckets, recurrence tokens, and raw create or
update intent construction. The FFI still exposes stringly timestamps and largely raw intent fields
in [`protocol.rs`](../crates/mailcal-bindings/src/protocol.rs).

Recommended API:

```text
EventEditorDraft::new(now_wall, device_zone, calendar_catalog)
EventEditorDraft::edit(EventDetail)
draft.apply(EventEditorChange)
draft.snapshot() -> fields + editability + validation
draft.build_command() -> Result<CreateOrUpdateEvent, EventEditorError>
```

Use typed civil date/time and recurrence/reminder enums at the boundary; require the host to inject
device “now” and the active zone. Native DatePickers, localised labels, timezone selection, dialog
lifecycle and pixel layout stay native. DST behaviour needs explicit tests, but the Linux copy proves
the rules themselves are portable Rust rather than inherently native UI logic.

### 6. Recipient-field reducer and authoritative envelope validation

The pure token transformations have three almost line-for-line implementations and three test
suites:

- Android [`RecipientAutosuggest.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/RecipientAutosuggest.kt)
- Apple [`RecipientTokens.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/RecipientTokens.swift)
- Windows [`RecipientTokens.cs`](../clients/windows/Mailcal/Services/RecipientTokens.cs)

They find the active token after the last comma, preserve committed recipients, accept a suggestion
without destroying earlier recipients, remove a pill, rebuild the field, and decide whether a
suggestion popup is meaningful. These are explicit cross-platform rules in
[`contacts.md`](contacts.md), not platform appearance.

There is a second, more important drift: client send-button readiness is not derived from the same
parser as submission. Android requires nonblank To and From; Apple and Windows primarily gate on To;
the Rust submit path accepts any nonempty parsed To/Cc/Bcc envelope. A Cc/Bcc-only message can thus
be valid in Rust but unreachable in a client, while shallow native checks can allow a later parse
failure.

Recommended split:

```text
project_recipient_field(text) -> { committed, active_token }
accept_recipient(text, bare_address) -> text
remove_recipient(text, index) -> text
validate_composer_envelope(from, to, cc, bcc)
    -> { can_send, reason: Option<EnvelopeError> }
```

The validator must call the same canonical address parser used by
[`mail_compose.rs`](../crates/mailcal-app/src/mail_compose.rs). Product policy must first decide
whether To is mandatory or any recipient field is sufficient; after that, Rust should be the only
authority. Caret placement, IME composition, pills, accessibility, focus and popup geometry remain
native. Calls can happen on token events or through a retained session, not necessarily on every
keystroke.

### 7. Per-composer From/signature session

Android, Apple and Windows independently own a small but consequential composer state machine:

- effective From account (`preferred -> stored default -> first account`);
- new-message versus reply/forward signature slot;
- inherited account choice versus explicit None versus explicit signature ID;
- automatic signature re-resolution when From changes;
- preservation of an explicit per-message choice across that change.

Representative implementations are
[`ComposerSignatures.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/ComposerSignatures.kt),
Apple's `RichComposerView.swift`, and
[`ComposerSignatures.cs`](../clients/windows/Mailcal/Services/ComposerSignatures.cs). Rust already
repeats the effective-account rule at final submission, so the sender displayed by a client and the
sender ultimately selected by the core have two authorities.

A `ComposerSession` should own mode, original message identity, effective account, recipient strings,
signature choice origin, and validation. `set_from` should re-resolve only an inherited signature.
The client should receive a typed seed/change result and retain the native editor, toolbar, account
picker, attachments and discard dialog.

This is a staged migration: begin with an authoritative `prepare_composer(...)` result and signature
choice reducer, then move additional session state only when it removes a second source of truth.

### 8. Typed quote and signature seed builders

All four clients manually serialize composer document blocks, including Rust enum spellings and
JSON field names. Examples are Android [`QuoteSeed.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/QuoteSeed.kt),
Apple [`QuoteSeed.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/QuoteSeed.swift),
Windows [`QuoteSeed.cs`](../clients/windows/Mailcal/Dialogs/QuoteSeed.cs), and Linux
[`composer_model.rs`](../clients/linux/src/ui/composer_model.rs). Signature block JSON is similarly
duplicated. Client tests exist specifically to detect schema drift after the fact.

Rust already owns the serde schema and sanitiser in `mailcal-composer`. It should construct a typed
`ComposerDocument` or canonical seed JSON from a structured input. The host supplies localised quote
attribution/header strings and its formatted date; localisation remains client-owned. Rust supplies
the safe schema, enum variants, body/identity checks and sanitisation.

The current Android testing convention explicitly names injected seed JSON as client-side logic, so
this migration requires a deliberate update to that convention. It is still a good ownership
candidate because the cross-language serialization is a schema/security seam, not localised UI.

### 9. Swipe/undo reducer with native effects and deadline

The production reducers in Android [`SwipeUndo.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/SwipeUndo.kt),
Apple [`SwipeUndo.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/SwipeUndo.swift), and
Windows [`SwipeUndo.cs`](../clients/windows/Mailcal/Services/SwipeUndo.cs) duplicate deferred
archive/delete, immediate flag/unflag, inverse actions, account-scoped identity, optimistic hidden
rows, supersession, stale completion rejection, commit, revert and a release grace state.

This is presentation state, but it is also an optimistic command transaction whose mistakes can
dispatch the wrong mail mutation. A per-scene Rust `UndoCoordinator` can accept `begin`, `commit`
and `undo` events and return typed dispatch/visibility effects. The host should continue to detect
the native swipe, render the snackbar, and schedule the four-second callback; passing a deadline
event is much cheaper and safer than asking Rust to own platform lifecycle timers.

`AGENTS.md` currently cites swipe/undo as a client-side state machine whose tests belong in the
Android suite. Moving it is therefore a contract/convention change, not just code motion. Given the
three independent machines and the product vision's stronger “presentation state machines in Rust”
rule, the ownership should be consciously resolved rather than left accidental.

### 10. Per-scene semantic calendar navigation

Android [`CalendarPaging.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/CalendarPaging.kt),
Apple [`CalendarPaging.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/Calendar/CalendarPaging.swift),
and Windows [`CalendarPaging.cs`](../clients/windows/Mailcal/Calendar/CalendarPaging.cs) repeat
civil-date semantics: page-to-anchor conversion, week/month stride, mode-to-column mapping, ties when
deriving mode from zoom, “change mode” versus “zoom,” Today behaviour, and deliberate week alignment.
These are load-bearing product rules in [`calendar.md`](calendar.md).

The same document explicitly says the client owns the current anchor and pulls several cached pages
synchronously. That is necessary for paging performance and multi-window independence. A suitable
migration therefore is **not** a global `App` calendar anchor and not per-frame FFI calls. It is a
small Rust value/session owned per native scene:

```text
CalendarNavigation::new(anchor, mode)
nav.apply(MenuMode | ZoomSettled | Step | Today | AlignWeek)
    -> { anchor, columns, reset_token, framing }
```

The native client keeps scroll offsets, page halos, pixel multiplication, gesture detection, live
pinch/fling math, hit testing and frame budgets. Only discrete settled semantic transitions cross
the boundary.

### 11. Generation-aware search and asynchronous probes

Mailbox search can take roughly a second. Android adds a 250 ms UI debounce, while Apple and Windows
dispatch more eagerly. FFI dispatch runs operations independently on a multithreaded runtime, so
debouncing alone does not establish latest-request-wins: an older query or account probe can still
finish after a newer one.

Rust should assign a generation, publish the query state immediately, coalesce work where useful,
and discard stale completions for mailbox search, contacts search and JMAP discovery probes. Clearing
search must remain immediate and reset scope atomically as required by [`search.md`](search.md).
Native clients retain text input, focus/back behaviour, filter controls and visual progress.

This also makes the client observer pattern simpler: clients render the authoritative query and
`is_searching` state rather than mirroring them solely to classify empty results.

## P2: targeted projection and boundary improvements

### 12. Move the portable DNS packet codec, not native resolution

Android [`DnsMessage.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/DnsMessage.kt) and
Apple [`DnsMessage.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/DnsMessage.swift)
duplicate raw query encoding, compressed-name parsing, AD-bit handling, bounds checks and
malformed-packet defence. This is a portable security parser with no UI dependency.

Move encode/decode to `mailcal-autodetect` and let a native `DnsTransport` return raw response bytes,
or export narrowly scoped helpers if transport constraints make that cleaner. Native APIs must
still select the OS resolver/network path (important for VPN and private-DNS behaviour), and Windows can
keep its structured `DnsQuery_W` adapter. Avoid a re-entrant or chatty FFI design.

### 13. Enrich connectivity snapshots

The connectivity snapshot exposes arrays of account IDs in
[`records_connectivity.rs`](../crates/mailcal-bindings/src/records_connectivity.rs). Each client then
joins those IDs to account email, technical detail, provider family and recovery action, often with
several additional FFI calls. See Windows
[`MailboxModel.Connectivity.cs`](../clients/windows/Mailcal/Services/MailboxModel.Connectivity.cs),
Android `MainActivityCore.kt`, and Apple `MailcalModel.Actions.swift`.

Emit coherent `AccountConnectivityIssue` records containing a typed kind and remedy. Clients still
localise banners and launch the correct browser or edit-credentials surface. This removes N+1 FFI
queries, provider-routing duplication and snapshot-consistency races.

### 14. Add aggregate settings/bootstrap snapshots

One settings invalidation currently makes clients pull many independent records; boot likewise
assembles state from a sequence of snapshots. Android `MainActivity.kt`, Apple
`MailcalModel.Reload.swift`, and Windows `MailboxModel.Projection.cs` repeat this fan-out and can
briefly combine values from different revisions.

Add a coherent aggregate settings snapshot and, where measurement confirms value, a bootstrap
snapshot. Preserve the successful observer design (signal then pull the latest immutable state), and
do not push large mailbox/calendar content through callbacks. This is FFI simplification rather
than moving business rules, but it makes thin clients substantially easier to keep coherent.

### 15. Add a canonical calendar catalog query

Windows pulls an arbitrary week merely to extract calendars; Linux merges calendars from page and
month data, deduplicates them, filters writable entries, and joins account labels. Editors then
repeat selection logic.

Expose a cached `CalendarCatalogSnapshot` with structured account/calendar identity, label data,
write capability, visibility, resolved colour and the default writable calendar. This is logically
global catalog data and should not depend on which week happens to be loaded. It is also a useful
input to the shared event-editor draft.

### 16. Emit semantic invitation and event-detail classifications

Android [`InvitationFormat.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/InvitationFormat.kt),
Apple [`InvitationFormat.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/InvitationFormat.swift),
and Windows [`InvitationFormat.cs`](../clients/windows/Mailcal/Calendar/InvitationFormat.cs) repeatedly
classify conflict count, attendee response buckets, pending/failed write state, awaiting response,
recurrence and reminder categories. They also derive unit-free invitation preview minute/hour bounds
that the core is well positioned to emit.

Return closed semantic enums such as `ConflictSummary`, `ReminderSummary`, `RecurrenceSummary`, and
`InvitationWritePresentation`, plus unit-free preview bounds. Clients must continue to assemble
localised/pluralized prose, format dates and convert units to pixels. Moving entire `InvitationFormat`
files would violate the localisation boundary; moving their classifications would not.

### 17. Include presentation-relevant state in contacts snapshots

The contacts snapshot contains only rows. Windows mirrors the query to distinguish “no contacts”
from “no results,” then explicitly clears both copies together. Expose `query_active` or a semantic
`Rows | Empty | NoResults` state from the same snapshot that owns the rows. Localised empty-state
copy and native section/list layout stay in the client.

### 18. Emit structured or opaque row identities

Android, Apple and Windows construct list IDs such as `m:<account>:<key>` and repeat the same encoding
in swipe/selection paths. A logical mail identity is already a typed account/key pair; delimiter
concatenation is unnecessary and can be ambiguous if an identifier contains the delimiter.

Expose a structured `RowIdentity` or a core-generated opaque stable ID for mail, thread, event and
calendar rows. Native list diffing and reconciliation remain native as the architecture requires.

### 19. Share desktop OAuth loopback protocol handling

macOS [`GoogleOAuth.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/GoogleOAuth.swift)
and Windows [`GoogleOAuth.cs`](../clients/windows/Mailcal/Services/GoogleOAuth.cs) independently bind
loopback, choose a port, parse one HTTP callback, return a closing page, match state, time out and
cancel. The portable one-shot HTTP/state protocol can live in `mailcal-oauth`; the client opens the
system browser. Mobile custom-scheme/`ASWebAuthenticationSession` rendezvous and all browser
presentation remain native. This is useful but lower priority than credential transaction safety.

### 20. Generate MCP configuration snippets from the authoritative protocol name

Apple `AgentComposerBridge.swift` and Windows
[`McpEndpoint.cs`](../clients/windows/Mailcal/Services/McpEndpoint.cs) hand-build the same JSON and
protocol key, while the authoritative server name already exists in `mailcal-mcp`. A Rust helper can
return the configuration snippet from a native-discovered relay path and endpoint. This is a small
drift-prevention cleanup, not a new MCP capability.

### 21. Enforce the signature inline-image cap in the core too

Native signature editors impose the current per-image size cap before constructing a `data:` URL.
That is good for immediate UX and memory usage, but an imported/bypassed document should not be able
to evade the limit before storage or submission. Enforce the same bound beside the existing Rust
signature sanitisation and `data:`-to-`cid:` rewrite. Native bounded reads and picker feedback stay.
The migration must decide whether over-limit existing content is rejected, stripped, or migrated;
that policy decision is why this is not a quick silent change.

### 22. Return typed FFI error information

Windows [`CoreError.cs`](../clients/windows/Mailcal/Services/CoreError.cs) strips a generated UniFFI
prefix from exception text before display. Other clients wrap raw strings differently. Replace
tuple-string leakage with `MailcalErrorInfo { kind, safe_detail }`; clients localise the surrounding copy
and keep technical details in privacy-safe diagnostics. This is a small boundary cleanup with broad
quality benefits.

## Candidates that require an explicit contract decision

These have shareable code, but moving them wholesale would contradict a current documented boundary.

### Rotating diagnostic log

The Android, Apple, Windows and Linux sinks separately implement the same 1 MB × three-backup,
best-effort, locked rotation policy (645 production lines across the four current implementations).
Linux already demonstrates a portable Rust implementation. However, [`logging.md`](logging.md)
explicitly says each client implements `Logger` over a **native rotating file sink**.

If that policy is revisited, a reusable Rust `RotatingDiagnosticLog` configured with a
native-selected path is plausible, while the host retains sandbox path selection, Logcat/`os_log`, sharing
and lifecycle metadata. Until then, keep it native and strengthen cross-platform conformance tests or
shared test vectors instead of treating it as an ordinary migration.

### Calendar all-day/month overflow projection

Clients duplicate the arithmetic that turns a host-computed lane/chip capacity into visible entries
and per-day hidden counts. A stateless Rust helper parameterized by capacity could protect the
“nothing is hidden without saying so” rule. But [`calendar.md`](calendar.md) correctly assigns pixel
capacity and the live pager to the host. Only discrete overflow projection is a candidate; measuring
capacity, layout, expansion animation, and per-frame scene construction stay native. The value may be
too small to justify an FFI call unless bundled with page projection.

### Settings taxonomy metadata

Category identifiers and order repeat across native clients, while titles come from the shared
catalog and platform availability/icons are native. Generated enum/order metadata would reduce
drift; a runtime FFI settings-navigation model is probably heavier than the problem. Prefer extending
existing code generation if this is addressed.

### Shell/navigation coordinator

Windows and Linux own duplicated destination transitions such as closing reading, clearing contacts
search and refreshing calendar. A Rust shell snapshot would align with the long-term architecture,
but dirty-composer guards, scene restoration, split-view behaviour and multi-window state make a full
migration high risk. First move the contained reading/search sessions; reassess whether enough pure
shell policy remains to justify a shared coordinator.

## Keep native

The following are deliberate host responsibilities, even where implementations look substantial:

- Locale, pluralization, user-facing sentence assembly, relative/absolute date formatting, timezone
  names, and capture of the device's current zone/time. Rust remains locale- and tzdata-free.
- SwiftUI/Compose/WinUI/GTK layout, accessibility, focus, native list diffing, icons, controls and
  platform navigation presentation.
- Calendar pixels, lane capacity, scroll offsets, per-frame hit testing, live zoom/fling/pinch
  physics, rendering caches and gesture arbitration. Rust should emit unit-free geometry and accept
  only settled semantic events.
- WKWebView/Android WebView/WebView2 hosting and native navigation/network/script barriers. Rust
  already owns sanitisation, CSP and canonical submit-time validation.
- [`editor.html`](../clients/composer/editor.html). This is already the correct shared, non-Rust DOM
  adapter; selection, caret, IME, paste, tables and DOM quote/signature manipulation belong beside
  the browser DOM. Only typed seed construction is a Rust candidate.
- OS keystore mechanics, system-browser presentation, mobile OAuth callback capture, file pickers,
  attachment open/share destinations, notification delivery, reachability monitors, background
  scheduling, native DNS transport, and platform log path/export UI.
- Multi-window/scene restoration and platform-specific discard-confirmation presentation.
- Tiny render mappings whose only output is an icon, colour or localised phrase. Crossing FFI merely
  to eliminate a four-line `switch` would make the architecture heavier, not thinner.

## Areas already correctly shared

The audit also found substantial evidence that the intended architecture works when followed:

- Mail mutations, durable outbox behaviour, search scope/order, account connection logic and mailbox
  projection are Rust-owned.
- Calendar overlap, recurrence materialization, conflict data, unit-free event geometry and colors
  are shared; clients multiply into pixels.
- Contacts canonicalization and merge-by-email, shared-account disclosure data, ordering and search
  run in Rust.
- Signature library persistence, sanitisation on store and submit, and inline `data:` image rewrite
  to `cid:` are shared.
- Rich document parsing, deterministic HTML/plain rendering, quoted-original re-sanitisation,
  attachment/CID manifests and final mail submission are shared.
- Analytics consent state, structurally closed payloads, install-ID lifecycle and relay dispatch are
  centralized.
- Background-sync bounds, inbox watermarks, first-run seeding and notification previews are shared;
  only OS scheduling/delivery is native.
- Account autodetection's strategy race, secured-link requirements and privacy rules are shared; the
  missing piece is carrying its security provenance through connection.

These are useful templates: a strong shared feature exposes typed semantics and a coarse snapshot,
then leaves only platform mechanics and rendering in the client.

## Suggested migration sequence

This is sequencing advice only, not authorisation to implement.

1. **Close boundary hazards:** require credential stores in the constructor; add account to reading
   identity; add generations/latest-wins; make detected approval unforgeable.
2. **Extract stateless primitives:** canonical recipient operations/envelope validation, typed seed
   builders, event reminder/recurrence classifications, structured errors and stable identities.
3. **Introduce contained sessions:** event editor, detected account setup, composer From/signature,
   and reading. Give each session one authoritative snapshot and typed effects.
4. **Move optimistic/async coordination:** swipe/undo and search/probe coalescing after lifecycle and
   multi-window semantics are written down.
5. **Reduce projection fan-out:** connectivity issues, calendar catalog, contacts state and aggregate
   settings/bootstrap snapshots.
6. **Reassess contract-review items:** calendar navigation/overflow, rotating logs and shell state
   only after measuring whether the proposed boundary reduces complexity without adding hot-path FFI.

For every migrated reducer, move its platform-neutral tests to the lowest Rust crate that owns the
contract. Keep a thin client adapter test proving generated binding conversion and UI effects. The
existing platform tests are valuable executable specifications; they should be ported before the
client implementation is removed, not discarded.

This audit works the opposite way round to improving client verification: rather than testing the
current clients harder, it reduces how much semantic behaviour needs independent client
verification at all.

## One rule, kept by hand in four places

Linux's calendar attendee presenter adds the email as a subtitle only when a display name exists
(`attendee_subtitle()` in `clients/linux/src/ui/calendar/attendees.rs`, pinned by
`attendees_tests.rs`), which is what Windows' equivalent does. Nothing enforces that agreement: each
client owns its own classifier, so the rule holds only as long as every copy is edited together.
This is presentation rather than a strong Rust-migration candidate, but it is the standing cost of a
“small” client classifier.

## Conclusion

The clients are not broadly too heavy because they are native; they are heavy where they have become
authorities for product semantics. The highest return comes from moving security provenance,
account/reading/composer transactions, editor reducers, and latest-wins coordination into Rust.
Doing that at session or command granularity preserves the responsive native architecture while
making behaviour impossible to drift across four clients.

The target should not be the fewest possible client lines. It should be one Rust authority for every
rule that must be identical, with native code limited to things that genuinely depend on the OS,
locale, DOM, accessibility tree, or frame loop.
