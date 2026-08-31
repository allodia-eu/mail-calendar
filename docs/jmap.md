# JMAP accounts: connection & auth contract

How the product connects a **JMAP** account (RFC 8620/8621), the third provider kind
alongside password IMAP and OAuth Microsoft 365. JMAP is a modern HTTP mail protocol
(Fastmail, self-hosted Stalwart / Cyrus / Apache James). This is the cross-platform
contract so every client connects a JMAP account the same safe way.

The shared core (`mailcal-account` + the bindings) owns the whole JMAP path; a client
owns only the **setup form** (collecting the fields), the platform auth session for the
optional OAuth sign-in, and secure storage of the resulting config.

## The rules

1. **A JMAP account is a base URL + one credential.** The engine's `JmapProvider`
   discovers the API endpoint, the server-assigned account ids, and the mailbox set from
   the session resource (`/.well-known/jmap`); the stored config carries only the account
   email, the server base URL, and a single secret.

2. **There is exactly ONE secret field, and it is stored as `password`.** RFC 8620 §8.2
   specifies no authentication mechanism of its own, so the *server* declares the scheme in
   the `WWW-Authenticate` challenge of its `401` and the engine's transport negotiates from
   it. A password, an app-specific password and an API token are therefore the same opaque
   secret to us, and the setup form asks for one, labelled "Password or API token", never
   two. Whatever the user pastes is stored as **`password`**, with the email as username,
   because that form is strictly more capable: it can be presented as `Basic` *or* re-framed
   as `Bearer` (`Credentials::can_present`), while a bare `token` has no username to build a
   Basic header from and is bearer-only. A `token` key is still *read* from configs stored
   before this collapse, and still works.

   > Two fields is what produced the original Fastmail bug report: a user with an API token
   > put it in the box that could not carry it. Do not reintroduce a second field.

3. **OAuth sign-in is offered when, and only when, the server advertises it.** A JMAP
   account may instead be connected by signing in with the provider, discovered entirely from
   the standards, with **no per-provider code**: RFC 9728 (the `401`'s
   `resource_metadata` names the authorization server) → RFC 8414 (its metadata names the
   endpoints and scopes) → RFC 7591 (dynamic client registration mints our `client_id`) →
   Authorization Code + PKCE. Four rules bind it:
   - **It fails soft, always.** Any step failing means "this server doesn't do this": the
     password/API-token field is still there and still works. It is never a dead end, and the
     specific cause goes to the diagnostic log rather than to the user.
   - **Every discovery hop must be HTTPS**, the metadata's `issuer` must match the issuer we
     asked about (RFC 8414 §3.3), and the server must advertise **S256** PKCE. Any of these
     failing declines the flow rather than running a weaker one.
   - **We request only the capabilities we use:** `offline_access` plus the advertised
     scopes whose last segment is `mail`/`calendar`/`calendars`. Never the whole
     `scopes_supported` list; a consent screen asking for contacts or admin we never exercise
     is a user-visible harm.
   - **Discovery runs once.** The endpoints, the registered `client_id`, and the refresh
     token are persisted with the account, so a launch re-registers nothing, **and neither does
     a re-authentication** (rule 8).

4. **The secret lives only in the OS keystore.** The stored `[jmap]` config carries the one
   secret, or, for an OAuth account, the `[jmap.oauth]` grant (client id, endpoints, refresh
   token) and **no** long-lived password, and redacts it in logs, exactly like a password
   IMAP config. Storage is the same OS secure store as every other account (Keychain /
   Credential Manager / EncryptedSharedPreferences): the config is provider-agnostic TOML
   keyed by account id. Every core takes the host's
   `AccountCredentialStore` **at construction** (there is no setter to forget) because a
   **rotated** refresh token that reaches no store leaves the account dead at the next launch,
   and the **core** does the writing and erasing through it, because a client cannot see a
   rotation that lands mid-connect (`provider-oauth.md` rule 5).

   > **Losing a rotation is worse than it sounds on a JMAP server, and Fastmail is the proof.**
   > It rotates the refresh token on every refresh *and* detects replay: presenting a superseded
   > token answers `invalid_grant — ratchet or client_id mismatch` and **revokes the whole
   > grant**. So a dropped rotation does not degrade to "re-authenticate later": the next launch
   > presents the stale token and destroys the account's access. Two defects did exactly that
   > (concurrent refreshes replaying one token; the cold background worker registering no store),
   > and both are covered by [`provider-oauth.md`](provider-oauth.md) rule 5. Neither was visible
   > on Microsoft or Google, which forgive both.

5. **The server URL may be discovered from the email.** A blank server URL derives
   `https://<email-domain>` and relies on `/.well-known/jmap` autodiscovery; a bare host
   (`mail.example.com`) gets an `https://` scheme. An explicit `http://` is preserved **only**
   for the local Stalwart test fixture. Real accounts are always TLS.

6. **Distinct account identity.** A JMAP account keys on `address@jmap:<host>`, so it never
   collides with an IMAP (`address@host`) or Microsoft (`address@graph.microsoft.com`)
   account for the same address; two JMAP accounts on different servers stay distinct.

7. **One provider per account.** Unlike IMAP/Graph (a provider per folder), a single JMAP
   provider covers the whole account: its mail scope is account-wide and each message
   carries its `mailboxIds` membership, so every folder syncs through it, and on-demand
   folder opens reconnect that same account-wide provider. An **OAuth** account wraps that
   provider in a `RefreshingJmapProvider`, which re-mints the access token and rebuilds the
   delegate whenever it changes (~hourly). The engine still only ever sees a finished bearer
   token, and needs no OAuth code of its own.

8. **A sign-in that dies can be re-run in place, from the account's own grant.** An expired or
   revoked grant raises the expired-sign-in prompt
   ([`provider-oauth.md`](provider-oauth.md) rule 12); for an account connected by *signing in*
   its button re-authorises that account rather than sending the user to Settings:
   `begin_jmap_reauth` builds the authorization URL from the **persisted** endpoints, client id,
   redirect URI, scopes and resource indicator (no network calls, no second registration), and
   `complete_jmap_reauth` connects, re-persists through the `AccountCredentialStore`, and retracts
   the prompt. The account keeps its identity, its settings and its downloaded mail: only the
   credential changes. The full contract, including why a sign-in completed as a *different*
   address is refused, is [`provider-oauth.md`](provider-oauth.md) rule 14.

   > This is where JMAP stops being one kind of account. A pasted password/API token has no
   > sign-in to re-run, so its prompt still points at Settings, where it genuinely can be
   > re-entered through `replace_account_secret`, which validates it before replacing the stored
   > credential. The core reports the two apart (`AccountProvider::Jmap` · `JmapOauth`) because
   > only the stored config can tell them apart, and a client that guesses gets it wrong in one
   > direction or the other: a missing button leaves remove-and-re-add as the only cure, and an
   > offered one dead-ends.

## What works today

- **Connect + folder list + message list** (subject / from / date / preview / flags-display),
  unified inbox, per-account folder sidebar, threading.
- **Reading a message body**: the raw RFC 5322 source is downloaded on demand via the JMAP
  `blobId` + the session `downloadUrl` (RFC 8620 §6.2), then sanitised and rendered like any
  other account (this is the engine's `fetch_message_source`, added for JMAP).
- **Sending new mail**: JMAP `EmailSubmission` (the engine's `submit_email`), filed to Sent.
- **Calendar (read)**: the account's calendars + events sync into the agenda when the server
  advertises calendar support.

## Per-platform matrix

Legend: ✅ implemented · 🚧 code-complete, runtime unverified · ⬜ planned · — n/a.

| Gate | Shared core | macOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Connect (one secret, scheme negotiated), session discovery | ✅ | ✅ | ✅ | ✅ | ✅ |
| **One** secret field ("Password or API token"), stored as `password` | ✅ | ✅ | ✅ | ✅ | ✅ |
| OAuth sign-in: discovery + DCR + PKCE, offered only when advertised | ✅ | ✅ | ✅ | ✅ | ✅ |
| Re-authenticate an expired OAuth account in place, from its persisted grant (rule 8) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Replace an expired pasted secret in place, connect-before-persist | ✅ | ⬜ | ⬜ | ⬜ | ✅ |
| Detected card: the sign-in offer **replaces** the secret field (which returns on failure) | — | ⬜ | ✅ | ✅ | ✅ |
| Rotated refresh token re-persisted (the shared `AccountCredentialStore`) | ✅ | ✅ | ✅ | ✅ | ✅ |
| A rotation in a **cold background pass** is persisted (stores are constructor params) | ✅ | ✅ | — no worker | ✅ | — no worker |
| The **core** stores the grant on add and erases it on removal; the host only implements the port | ✅ | ✅ | ✅ | ✅ | ✅ |
| Folder list + message list + threading | ✅ | ✅ | ✅ | ✅ | ✅ |
| Read message body (blob download) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Send new mail (EmailSubmission) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar read/sync | ✅ | ✅ | ✅ | ✅ | ✅ |
| Secret in OS keystore, redacted in logs | ✅ | ✅ Keychain | ✅ Credential Manager | ✅ Keystore | ✅ Secret Service |

**Android**, **Apple** and **Windows** all ship the OAuth sign-in. Linux implements the same path in
its unshipped client. Every host shows the button **only** when the core's
`jmap_oauth_available` pre-flight says the server advertises it.

**The manual path is never removed, but on a detected card it does step aside.** Where a detected
server offers sign-in, Android, Windows, and Linux show the button *instead of* the secret field:
offering both asks the user to choose between two routes to the same place when the better one is a
single tap, and it makes the copy contradict itself ("just add your password or an API token" beside
"there's no API token to create", so that note stands down with the field, as do the server box and
Connect). What makes this safe is that it is never a dead end: a sign-in that **fails** brings the
whole manual path straight back, "Set up manually" reaches it at any time, and the **manual form
always shows it**: someone who chose to set the account up by hand asked for the fields. A server
that declines this flow can therefore never leave a user stuck. Linux's detected card itself
supplies the manual fallback. Apple still shows both on its detected card (see Known gaps).

Verified live against `api.fastmail.com` on **both** an Android emulator and the iOS simulator,
in three directions. The gate answers **yes** for `api.fastmail.com` (reached through email-first
detection, whose SRV lookup supplies that host), and the flow then runs discovery + RFC 7591
registration + PKCE and hands off to Fastmail's own login page with the address pre-filled: the
registration is real, since the authorization request would be rejected without an issued
`client_id`. It answers **no** for a domain that publishes no such metadata, leaving only the
secret field. And **dismissing the browser** returns to a fully usable form: no stuck spinner, no
error claimed for something the user chose to abandon.

The flow has since been completed end to end against a real Fastmail account on Android: consent
screen → token exchange → `add_account` → keystore → mail syncing. The account then survived an app
reinstall and re-authenticated from the stored grant, which exercises the **refresh** path:
`GraphTokenSource` always starts expired, so the first call after a launch necessarily refreshes.
That matters more than it sounds: the RFC 8707 resource indicator has to ride on the refresh as
well as the exchange, and an account that omitted it would have worked for exactly one hour.

**Windows is now verified live too** (2026-07-21, against the same real Fastmail account): email-first
detection routed to JMAP over `_jmap._tcp` SRV, the pre-flight answered yes for `api.fastmail.com`,
and the flow ran all four discovery steps: RFC 9728 resource metadata → RFC 8414 endpoints → scope
selection (3 of the 8 offered) → RFC 7591 registration, then the consent page, the redirect back
through the custom scheme, the exchange carrying the resource indicator, and `add_account`. A
**relaunch** then re-authenticated the account from its stored grant (`connect[jmap]: authenticated`
in a fresh process), which is the refresh path: the token source always starts expired, so the first
call after launch necessarily refreshes.

**Linux is verified live too** (2026-08-19, against a real Fastmail account). Its detected JMAP card
runs the same pre-flight off the GTK thread, then uses the system browser and a dynamically
registered `http://127.0.0.1:<port>/` redirect. One listener stays alive for the process so every
retry keeps the same redirect URI and reuses the core's session registration cache; a late callback
from a cancelled attempt is rejected by `state` while the listener keeps waiting for the current
one. Completion hands the returned config to the same `add_account` the password/API-token fallback
uses, and the core writes the grant through the host's `AccountCredentialStore` (on Linux the
Secret Service item its accounts are already read from), rolling the add back itself if that write
fails. That pass ran the whole RFC 9728 → 8414 → 7591 chain and **persisted a rotated refresh token
to Secret Service on the first refresh**, which is the ratcheting-server path of
[`provider-oauth.md`](provider-oauth.md) rule 5. Mail on a JMAP account (folder and message lists,
a body read, a send) is driven against the local Stalwart harness by the Linux AT-SPI acceptance
run. What no provider pass has covered yet: a **relaunch** re-authenticating from the stored grant,
and a provider JMAP account's **calendar**.

macOS is the first client of the tab itself (the setup form's **JMAP** tab: email, one secret,
optional server); **Android** ships the same tab, storing the secret via
EncryptedSharedPreferences over an Android-Keystore master key. **Windows** now ships it too: the
same **JMAP** tab in `AccountSetupView`, reusing the same core + FFI
(`jmap_account_config_toml` → `add_account`) and storing the secret in the Windows Credential
Manager. It adds only the setup fields and secure storage, since the core owns the whole JMAP path.
The field logic (which secret gates Connect, blank server ⇒ null) lives in the WinUI-free
`JmapSetupForm`, unit-tested in `Mailcal.Tests`.

## Known gaps

These are **engine** capabilities the JMAP adapter does not implement yet, so the product
cannot offer them for a JMAP account regardless of client:

> **Mail actions are no longer among them.** At the pinned engine revision `provider-jmap`
> implements `edit_mail`, and the core forwards it
> (`crates/mailcal-account/src/jmap/refreshing_provider.rs`), so mark-read/flag, archive, delete
> and move all work on a JMAP account, as they do on Microsoft 365, which the old wording also
> named. This paragraph said otherwise for longer than it was true.

- **No attachment download.** Received-file listing/saving needs the same on-demand fetch as
  bodies extended to attachment parts; not wired for JMAP yet.
- **No push / real-time.** The *engine* has a `JmapWatcher` (EventSource / `StateChange`, RFC 8620
  §7.3), but the product core does not wire it: `background.rs`'s watch loop is IMAP-only, so a
  JMAP account **polls** (the 15–120 min interval) rather than receiving mail as it arrives.
  This is the one place the OAuth work leaves a thread hanging deliberately: a standing
  EventSource connection is the one place a JMAP access token would expire *mid-stream*, so
  whoever wires push must handle a watch `401` by reconnecting with backoff (never a hot loop).
  The refresh machinery it would need already exists (`RefreshingJmapProvider`). Tracked as a
  follow-up; do not wire push without that handling.
- **Calendar is read-only.** JMAP calendar writes / RSVP are deferred in the engine.
- **The manual form's blank server hides the sign-in button for some providers.** With the server
  box empty the core derives `https://<email-domain>` (rule 5), and a provider whose JMAP lives on
  a different host (Fastmail's is `api.fastmail.com`) 404s there, so the pre-flight says "no
  sign-in" and only the secret field is offered. Arriving through **email-first detection** works,
  because its `_jmap._tcp` SRV lookup carries the real host; so does typing the server by hand.
  Closing this means giving the OAuth pre-flight the same SRV resolution detection has, which
  needs the host's `MxResolver` port plumbed into it. Deliberately not done here.
- **Fastmail message bodies fail to download (engine-side**: engine issue
  [#82](https://github.com/allodia-eu/email-calendar-sync-engine/issues/82)**).** Observed 2026-07-21 on Windows right
  after a successful OAuth sign-in: the account connects, folders and the message list sync, but
  every body prefetch fails with `Permanent provider error: JMAP HTTP 302` and an nginx redirect
  page. The session request logs `connect[jmap]: followed a redirect`, so redirects are handled
  there but evidently not on the **blob download**, which is why the matrix's "read message body"
  row holds against Stalwart and not against Fastmail. Not fixed here: the JMAP adapter that owns
  the download lives in the **engine** repo, and nothing in this repo emits that error string. It is
  not OAuth-specific as far as we can tell: a Basic/token Fastmail account would take the same code
  path, so it is a JMAP-adapter bug, not a sign-in one.
- **Apple's detected card still shows the secret field beside the sign-in offer.** Android,
  Windows, and Linux show the button *instead of* it there (see the rule above); Apple's
  `AccountSetupDetectView` renders the `SecureField` unconditionally, so the same detected Fastmail
  card reads differently on macOS/iOS than on the other two. Nothing is broken (both routes work)
  but the "two ways to do one job" UX the other clients removed is still present, and the
  contradictory "just add your password or an API token" note still shows under an offered sign-in.
  Closing it is the same edit made in `AccountSetupView.JmapSignIn.cs` / `AccountSetupDetect.kt`,
  and it needs a Mac to verify (the app build, not `swift build`).
- **The rotation callback is exercised now, and it is how the first production JMAP account
  died.** This bullet used to read "unexercised". Fastmail rotates on every refresh, so the path
  fired constantly; two defects meant the new token often never reached the keystore (concurrent
  refreshes replaying one token, and the cold background worker registering no store), and
  Fastmail's replay detection revoked the grant. Both are fixed and regression-tested
  ([`provider-oauth.md`](provider-oauth.md) rule 5). What is still unverified is the **fixed**
  path against a real rotating server over a long window: the offline tests model the ratchet,
  but only a live account proves the rotation survives days of background passes.
- **Re-authentication (rule 8) is verified on every platform that ships it.** Proven end to end on
  2026-08-01 against a real Fastmail account, twice, on two dead-grant *causes*: Android after
  Fastmail's ratchet revoked the grant (`ratchet or client_id mismatch`), macOS after the user
  revoked it by hand in Fastmail's Connected-apps UI (`oauth token not found`). Both took ~14 s
  end to end; both opened the authorisation with no discovery call, carried the resource
  indicator, landed the rotated token in the platform keystore (the macOS Keychain item's
  modification time matches the re-auth to the second), retracted the prompt, and resumed with a
  catch-up rather than a re-download. **iOS/iPadOS** was verified the same day on the simulator (6.1 s end to end,
  including the Keychain write). **Windows** followed the same day (8.2 s), on the grant the macOS
  revoke had already killed. So it exercised the case where the account is dead *before the app
  first sees it*: the prompt is raised on the boot reconnect rather than on a sync, the banner
  carries the button wording instead of "update it in Settings", and the fresh token lands in the
  **Credential Manager** (blob `LastWritten` and hash both move; a cold relaunch then connects with
  no prompt). Fastmail **accepted the replayed `client_id`** (no
  `invalid_client`), so the registration survived the revocation: see
  [`provider-oauth.md`](provider-oauth.md) → Known gaps for the server that might not, and for
  what reuse does *not* buy.
- **Dynamic registration is cached for the session only.** Each call to the registration endpoint
  mints a *new* client id, so repeated attempts would each leave an orphaned registration on the
  user's account. Attempts within one app session now reuse the first registration, and a
  completed sign-in persists its client id with the account and never registers again. So the
  only case still uncovered is *abandoning* a sign-in, quitting, and starting over, which leaves
  one stray registration. Linux deliberately holds one loopback port for the process so its
  retries keep the cache key stable too. Closing the remaining cross-launch gap needs a host storage
  port for a credential that belongs to no account yet. Note the client id itself is **not ours to
  choose**: RFC 7591 has the server mint it. The stable identifier we *do* control is `software_id`
  (a fixed UUID, identical across every install), which is what lets a server recognise Allodia
  across registrations.

## Testing

The OAuth discovery chain has two layers of cover. Offline, `mailcal-oauth`'s unit tests pin every
*decision* it makes: HTTPS-only hops, the RFC 8414 §3.3 issuer match, the S256 requirement, and
which scopes are selected, deliberately without a mock server, since a plaintext mock would have
to disable the very checks under test. Live, the gated
`crates/mailcal-oauth/tests/live_discovery.rs` runs the real chain against a real server
(`MAILCAL_LIVE_JMAP_ORIGIN=https://api.fastmail.com cargo test -p mailcal-oauth --test
live_discovery`); it skips with no network and stops **short of registration**, so running the
suite never creates a client on anyone's account. That test exists because the first real bug was
exactly a shape no mock predicted: Fastmail's `/.well-known/jmap` answers **302** to
`/jmap/session`, and only that URL returns the `401` naming the (path-scoped) protected-resource
metadata.

The Linux host tests additionally pin the detected-card transition (provider action alone → manual
fields restored on failure), stale pre-flight rejection, one stable loopback redirect across
retries, late-callback `state` filtering, cancellation, and the close-page response. They exercise
real GTK widgets under Xvfb, not only a projection of the setup state.

A real Stalwart JMAP server (`docker/stalwart/`, lifted from the engine) backs both local
testing and CI (`.github/workflows/ci.yml`). The gated integration test
`crates/mailcal-account/tests/live_jmap.rs` connects a JMAP account, syncs, and reads a body;
it **skips** unless `STALWART_HTTP_ADDR` is set, so the offline suite stays green with no
Docker. See [`docker/stalwart/README.md`](../docker/stalwart/README.md).
