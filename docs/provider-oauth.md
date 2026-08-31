# Provider OAuth: token & redirect contract

The cross-platform contract for connecting an **OAuth mail account** (Microsoft 365 and
**Google** (Gmail + Google Calendar) today; IMAP/SMTP `XOAUTH2` later reuses the same
machinery). It governs how the browser sign-in is run, how tokens are held, and how they are
refreshed, so every client does it the same safe way rather than each reinventing it. The
rules below are written for Microsoft; **"## Google" records only where Google differs**; the
rest of the contract is shared verbatim.

The shared core (`mailcal-oauth` + `mailcal-account` + the bindings) owns the whole OAuth
**state machine**; a client owns only the **browser half** (opening the authorization URL and
capturing the redirect), because that is inherently platform-specific.

## The rules

1. **Authorization Code + PKCE, public client.** The app is a public client; the PKCE
   `code_verifier` (S256) is what protects the token exchange. The client id is *not* a secret and
   may be embedded. **No _confidential_ secret is ever embedded**, with one documented exception
   below: a Google _Desktop_ client (the macOS/Windows loopback flow) carries a **non-confidential**
   `client_secret` that Google's token endpoint requires anyway, and which Google itself says "is
   obviously not treated as a secret." See "## Google" and
   <https://developers.google.com/identity/protocols/oauth2#installed>.

2. **System browser, never an in-app WebView.** Sign-in runs in the platform's system auth
   session (Apple `ASWebAuthenticationSession`, Android Chrome Custom Tabs, Windows the default
   browser + a custom-scheme protocol activation), **not** an embedded WebView. This reuses the
   user's existing browser login session (often skipping a password prompt) and keeps the app
   out of the credential path. The session is **non-ephemeral** on purpose (it *wants* the
   shared session).

3. **Custom-scheme redirect, validated by `state`.** The redirect returns to a registered
   custom scheme (e.g. `eu.allodia.mailcal://oauth`). The core generates a random `state`
   in `begin_microsoft_login` and rejects a callback whose `state` doesn't match (CSRF). The
   client passes the **raw callback URL** back to the core (`complete_microsoft_login`); it does
   not parse tokens itself.

4. **Tokens live only in the OS keystore.** The stored account config carries **only** the
   refresh token (the single at-rest secret), never a password; access tokens are minted on
   demand and never persisted. The config secret redacts itself in logs. Storage is the same OS
   secure store as password accounts (Keychain / Credential Manager / EncryptedSharedPreferences).

5. **The credential's lifecycle is the core's; the platform store is the host's.** A stored
   credential is created, replaced and erased by the **core**, through one host port:
   `AccountCredentialStore`, with `persist` and `delete`. The host owns what only it can: the
   encryption, the access control, the chunking, the keychain item. It does not own *when*.

   That split moved. Every client used to sequence the transaction itself: connect, then write
   the config it had been handed; remove the account, then delete the entry, which is four
   copies of an ordering, and a client cannot get one of its steps right even in principle: a
   refresh-token rotation can land **during** the connect, so the config the client is holding is
   already the token the server has moved past. The core writes what its *registry* holds, which
   is the only copy that has seen the rotation. It follows that `complete_microsoft_login` and
   `complete_google_login` no longer hand a config TOML back to view code for it to store; they
   return the account row, and the grant never leaves the core.

   **A refresh names no scope, and that rule is not negotiable.**
   [RFC 6749 §6](https://www.rfc-editor.org/rfc/rfc6749#section-6) requires the requested scope on
   a refresh to be a subset of what was granted, and an omitted one to mean exactly the original
   grant. Sending the build's *current* list therefore breaks every grant issued before that list
   grew: the server answers `invalid_scope` and the whole account stops working, not merely the
   feature the new scope was for. Omitting it is the only value that cannot go stale. Scope is
   asked for on the **authorisation** request, which is where consent is given and what signing in
   again re-runs.

   **A refusal is classified, never matched on text.** `OAuthError::refusal()` answers with a
   `GrantRefusal`: `Dead` (`invalid_grant`: revoked or expired), `Underscoped` (`invalid_scope`:
   alive but narrower than this build needs), or `Indeterminate` (everything else, which says
   nothing about the grant at all). The first two both need consent and neither recovers by
   waiting, so a token source treats them alike and stops retrying; the third must never change
   what anyone believes about a sign-in, or a bad afternoon at a provider signs somebody out. This
   is the OAuth-layer twin of the mail side's `FailureClass::Authentication`, and it applies to
   every provider at once because they all share this crate.

   The core refreshes an access token before expiry (skew margin) and, when the provider
   **rotates** the refresh token, re-persists the updated config through the same port, so the
   stored token never goes stale. **One port serves every family** (Microsoft, Google, an OAuth
   JMAP account): there were three, with identical signatures and identical one-line
   implementations in every client, and three ports is what made forgetting one cheap.

   **Both methods report success or failure, and the core answers differently depending on when
   it asked.** They returned nothing until the core took the transaction over, which meant the
   core logged `re-persisted a rotated refresh token to the host store` whether or not a byte had
   been written, a line that could not fail, on the one path whose failure mode is a revoked
   grant. What it does with each answer:
   - **Adding an account**: the add is rolled back and the error is returned, so setup fails on
     the surface the user is still looking at. An account that works until the next launch and
     is then silently absent is the worse outcome; an orphaned OAuth grant costs nothing.
   - **A rotation**: nothing can be rolled back, because the token it replaced is already spent.
     The session continues from the token in memory and the failure is logged at `error!`, saying
     what will happen at the next launch rather than what just did.
   - **Removing an account**: the account is gone from the runtime either way, so the error is
     returned rather than the removal being reverted. What survives an undeleted entry is a
     *zombie account*: it reappears at the next launch with nothing to explain it.

   Two client stores had to learn to say no for this to mean anything: Apple's `KeychainStore`
   discarded every `OSStatus` ("best effort"), and Windows' `CredentialStore.Save` returned
   `void`. Both now report, because a port that promises an answer above a store that cannot give
   one is the same unfalsifiable check one level down.

   **Four rules make that safe against a server that treats a replayed refresh token as theft.**
   Modern authorization servers (OAuth 2.1 / RFC 9700 refresh-token replay detection; Fastmail
   answers `invalid_grant — ratchet or client_id mismatch`) do not merely reject a superseded
   refresh token: they **revoke the entire grant**, so the account is dead and the user must sign
   in again. Fastmail's own developer documentation (<https://www.fastmail.com/dev/>) states it
   outright: the client *"MUST NOT try to use an old refresh token again; this will result in the
   authorisation being revoked as a protection against leaked refresh tokens"*, and *"you cannot
   share a refresh token between devices"*. Every rule below exists because that server, unlike
   Microsoft and Google, gives no second chance:
   - **One refresh at a time per account.** Every provider on an account shares one
     `GraphTokenSource` (for JMAP that is mail, calendar *and* contacts) and they sync
     concurrently. Without single-flight each one that finds the access token stale posts its own
     refresh carrying the *same* refresh token, and the loser of that race is a replay. The
     source therefore serializes refreshes and hands the winner's token to everyone waiting.
   - **A refresh *failure* is shared with the waiting callers, not just a success.** This is the
     half that serializing alone does not buy, and the half we shipped without. A waiter wakes,
     re-checks the cache, finds nothing there (because the refresh it queued behind *failed*) and
     goes on to post its own request presenting the same refresh token. The lock made the requests
     sequential rather than concurrent; it did not make them stop. One failed refresh on a JMAP
     account therefore produced **one replay per waiting provider**, measured at eight. So the
     source remembers its last failure and hands *that* to callers arriving within its cool-down.
     The pre-existing single-flight test could not see this: it asserted one request for eight
     concurrent callers against a server that **answers**, and the bug lives on the path where the
     server does not.
   - **A failure is only retried with the same token when the request provably never left the
     device.** `reqwest` reports "could not connect" and "the server answered and the response died"
     as one error, and the two have opposite consequences: the first means no server saw the token,
     the second means the token is spent and its replacement went down with that response.
     `OAuthError::reach()` splits them into `NotSent` / `MaybeProcessed`, deliberately
     **asymmetrically**: `NotSent` is claimed only where it is provable (`is_connect`: DNS, a
     refused peer, a failed TLS handshake) and everything unclear is `MaybeProcessed`, because being
     needlessly cautious costs a delay and being wrong the other way costs the account. A body that
     dies after the status line has its own variant, `OAuthError::ResponseLost`, so this can never
     collapse back into "transport". Each classification carries its own cool-down: seconds for
     `NotSent` (its only job is collapsing the fan-out), a minute for `MaybeProcessed` (each attempt
     into a failing network is another independent chance to spend the token and lose the answer, so
     the goal is fewer rolls of the dice, not faster ones). Both stay **retryable** errors
     (`ProviderError::retryable`, never `authentication`), so a dropped packet can never put a
     re-authentication prompt in front of a user.

     Two dependency assumptions here are pinned by tests that drive a real `reqwest` rather than
     hand-built errors, because `reach()` reading `is_connect()` is a claim about a library version
     and an assumption a test cannot falsify is a comment. The one that matters most is
     Android's: a backgrounded app's uid loses network access while the **device** keeps its
     network, so `getaddrinfo` fails with `EAI_NODATA`, 227 times in five days on one production
     device. Every one of those must classify `NotSent`, or an account would park itself over
     failures that never reached a network.
   - **Every core that can refresh must be able to persist.** A rotation that no store receives
     is not "kept in memory": it is a **stored token the server has already moved past**, which
     is a live grenade on a ratcheting server. So **both** constructors take the store as a
     **required parameter** (`new_background_worker` *and* `new_accounts`) rather than trusting
     a host to install one afterwards. There is no setter to forget.

     **The registry is the second half of the same rule.** The sink can only re-serialize a
     rotation into a config it can *find*, so every path that connects an account registers it
     **before** it dials. The obvious fix has a trap worth naming: re-inserting the config the
     connect *returns* would put the superseded token back, because that config was parsed from the
     original TOML and knows nothing of a rotation that landed while the connect ran. So the connect
     contributes only its token source; the config stays as the registry holds it. A connect that
     fails rolls the pre-registered entry back.

     ⚠️ **"The ordering the boot path always had" is not something this doc may assert.** It holds
     for the *interactive* branch, which prepares provider-less placeholders and registers every one
     before the background dial; a headless worker that connects every stored account under one
     `join_all` and registers them afterwards does the opposite, so every rotation during a cold
     pass reaches the sink with no entry, and the loop that registers them then re-inserts the
     pre-rotation config over what the sink managed to persist. That costs a real grant on a
     provider that treats a replayed token as theft; a doc asserting an invariant holds is
     worse than one that says nothing, because it ends the search. Both branches now register
     through one helper (`account_registry::register_before_connect`), and `StoredOutcome` carries
     no `ConnectedAccount` at all, so there is nothing a caller *could* re-register with. The
     regression test drives `new_background_worker` over a live loopback token endpoint that
     rotates, because a test that fires the sink after the constructor returns passes either way.
     **A third half, found on a device: one credential state per account, per _process_.** The
     single-flight above serializes refreshes because the folder providers share one *source*. It
     stops working the instant there are two sources for one account; and a host produces those
     routinely: on Android a one-time `MailSyncWorker` and the periodic one are different unique-work
     chains and can overlap, and `MailcalApplication.liveCore` is a `WeakReference`, so a worker can
     miss a warm core and build a cold one beside it. Two cold cores 6 ms apart, measured:

     ```text
     10:58:26.473  oauth: jmap [acct:05f4]: refreshed in 307ms; ... the server ROTATED the refresh token
     10:58:26.641  oauth: jmap [acct:05f4]: refreshed in 302ms; ... the server ROTATED the refresh token
     ```

     Two rotations of one grant from two independent refreshers, each having read the same stored
     token, so the second is a replay of one the first superseded. **A process-wide lock is not the
     fix**, and that is the part worth remembering: when the second source finally acquired the lock,
     its own state would still hold the token it read at boot, because the first refresh advanced a
     *different* core's state. It would wait politely and then present the spent token anyway. What
     has to be shared is the **state**: the current refresh token, the cached access token, the
     failure memo, and the single-flight over them, after which the second source finds a valid
     access token and never refreshes at all.

     Which leaves exactly one decision, and `CredentialOrigin` makes every call site state it: a
     source built from the host's **store** adopts any live state for that account (it is the same
     credential, possibly already advanced past what was stored), and a source built from a **fresh
     sign-in** must replace it (the credential the old state describes has just been retired).
     Adopting on a re-authentication would leave a repaired account refreshing with the dead grant it
     was repaired from: an hour of working normally, then permanently broken. An enum rather than a
     "remember to reset first", because forgetting it is silent.

     The Android host additionally *skips* an overlapping pass (`PassInFlight`), not as the fix, but
     so the fix is rarely needed: a second pass duplicates every socket and every SQLite handle to
     fetch mail the first is already fetching, inside an OS window that is about to close.

     Both boot paths are drawn step by step, before and after, in
     [`boot-sequence.md`](boot-sequence.md), which also lists the four independent assemblers of an
     account that made this divergence possible, and what to do about them.

     The interactive constructor needed this as much as the headless one, which was not obvious
     until it was measured. It starts the background dial in its **last statement**, and on a
     production Android launch the first OAuth refresh began **6 ms** later, with the host still
     blocked inside the call. Two of the four clients then installed their stores from a
     UI-thread post (Android `mainHandler.post`, Windows `TryEnqueue`), so whether a Graph
     rotation arriving ~660 ms after the constructor returned was persisted or dropped came down
     to whether the main thread had got a turn during app start. It usually had. A property that
     holds *usually* is not a property, and the failure is invisible for an hour and then
     permanent.

   Both were real defects, and both were invisible for a year because the only OAuth providers we
   had forgive them: **Google does not rotate on a refresh grant at all**, and **Microsoft leaves
   a superseded refresh token valid**. The first ratcheting server we connected was therefore also
   the first to lose an account, on a device where the Graph and Gmail accounts beside it kept
   refreshing perfectly. When adding a provider, assume it ratchets.

   **A refresh is logged at INFO, in the core, so every platform gets it.** Start (and why), the
   outcome with the new validity window, and **whether the server rotated the token**, plus an
   `ERROR` when a rotation cannot be persisted (the account will fail to authenticate at the next
   launch, which is not a warning), and a `WARN` naming `invalid_grant` when the stored token is
   refused. A failed refresh logs **which side of `reach()` it fell on** and how long the account
   will hold off, so a log can distinguish "the phone had no network" from "we may have just spent
   the token": the two look identical otherwise, and only one of them explains a dead account. And
   the line that says *why* a refresh is starting names a previous failure rather than reporting
   "first use in this process": a failed refresh leaves nothing cached, so that message repeated
   every attempt and made one retrying account read as several token sources, which cost an
   investigation. This was `debug!` and therefore absent from every shipped log: an account
   died of a refresh defect and the support log contained no line saying a refresh had ever
   happened. Privacy is unchanged: the provider family, durations and outcomes, never an address
   or a token ([`logging.md`](logging.md)).

6. **`offline_access` is mandatory.** Without it no refresh token is issued and the account
   breaks an hour after setup; the core treats a completed sign-in with no refresh token as an
   error.

7. **Register the redirect URI in the provider portal, character-for-character**, under the
   "Mobile and desktop applications" platform (Azure). It must equal the client's
   `redirectURI`.

8. **Target the connected account with `login_hint`.** When the address is known (the
   email-first autodetection route, or the address typed on the manual Microsoft tab),
   `begin_microsoft_login` receives it as `login_hint`; the authorization URL passes it through
   and **drops `prompt=select_account`**, so Microsoft targets that account rather than a
   different one already signed in in the browser. With no hint (a bare Microsoft pick),
   `prompt=select_account` shows the picker. This matters most when the browser holds a
   *different* account (a work/school login): without the hint Microsoft evaluates the request
   against that account (whose org may block an unverified app with `access_denied`) instead
   of the personal account the user is actually adding.

9. **A failed or declined sign-in is surfaced, never swallowed.** Microsoft can return
   `access_denied` (the user declined consent, or an org policy blocked the app); the client
   **must show that error** on the sign-in surface so the user can retry or fall back to manual
   setup: a Connect/sign-in button that silently does nothing is a bug (regression-tested in the
   email-first flow's Microsoft found-card).

10. **The requested Graph scopes must be granted in the Azure app registration, and widening
    them re-consents.** `MICROSOFT_GRAPH_SCOPES` requests `Mail.ReadWrite` (mail sync **and** the
    write actions: mark-read/flag, move/archive, delete), `Mail.Send` (submission: a scope
    **distinct** from `Mail.ReadWrite`, which does not grant send), `Calendars.ReadWrite`,
    `User.Read`, `Contacts.ReadWrite` (the account's saved contacts) and `User.ReadBasic.All`
    (the tenant directory, and the permission a colleague's profile photo is read through), plus
    `offline_access` and the OIDC scopes. Each must be a delegated permission on the Azure app
    registration's **API permissions** or Microsoft returns `access_denied` at consent.

    ⚠️ **A scope the account cannot grant is a setup failure, not a missing feature.** That
    `access_denied` happens *during consent*, so an unregistered or admin-gated permission does
    not cost one capability: it stops the account being added at all. The requested set
    therefore stays to permissions a **user can consent to for themselves**, and
    `ProfilePhoto.Read.All` and `OrgContact.Read.All` are deliberately **not** requested: both
    are tenant-wide reads a tenant may put behind an administrator, and neither adds anything
    (`User.ReadBasic.All` already grants the photo read, verified against a real tenant
    directory user; organizational contacts are a source the product does not read). Before
    adding any Graph permission, check its **Admin consent required** column in the portal: a
    "Yes" there would put every user in such a tenant behind their administrator before they
    could connect.

    **Contacts are requested read *and* write although the product only reads them.** Widening a
    scope later costs every existing account a forced re-authentication (rule 11), so a capability
    we know is coming is cheaper to ask for once, up front. The consent screen says "full access
    to your contacts" and the app does less than that; what bounds it is
    [`privacy-policy.md`](privacy-policy.md), which states the read-only behaviour plainly and
    explains the gap. If contact editing is ever dropped, narrow both back to their read-only
    spellings (`Contacts.Read`, `contacts.readonly`).

    **All three OAuth providers moved together, deliberately.** Microsoft requests
    `Contacts.ReadWrite` + `User.ReadBasic.All`; Google requests `contacts`,
    `contacts.other.readonly` and `directory.readonly`; and a JMAP server's `contacts` scope is
    now in `WANTED_CAPABILITIES`. Spreading them across releases would charge a user a fresh
    re-consent prompt per provider per release, and would leave the privacy policy having to
    describe a different answer for each: the one thing §2 must not do. Google's contact scopes
    are **sensitive**, not restricted: a declaration, justification and demo video, not the second
    security assessment the Gmail scope is already waiting on. Scopes are granted by **incremental consent**: widening
    the requested set makes every existing account re-authenticate before the new capability
    works; until it does, the un-requested capability returns `403 ErrorAccessDenied` at the
    **point of use** (a mail write or a send), not at connect.

11. **A withheld Graph capability raises a re-consent prompt; one reconnect clears every prompt.**
    An account connected before a scope was added (or whose consent was **revoked server-side**)
    keeps a narrower grant than the app now requests, and a token *refresh* re-uses that original
    grant, so only a full interactive re-authentication widens it. Two detectors feed one prompt:
    the **calendar** scope is checked by a boot-time probe (`connect_graph_calendars`), and **mail
    write/send** is detected **reactively** at the point of use: a `403 ErrorAccessDenied` from a
    mark-read / move / delete or a `sendMail` is classified **structurally** (walk the engine
    error's `source()` chain to the typed `ProviderError` and match the `ErrorAccessDenied` code, so
    a non-authorisation 403 such as the idempotent-re-delete `ErrorCannotDeleteObject` is *not*
    mistaken for a permission gap) and raises `mail_reauth_accounts` on `Surface::Connectivity`.
    Each client renders a banner naming the affected account(s); its **Reconnect** re-runs sign-in
    with `login_hint` set to that address, re-requesting the whole `MICROSOFT_GRAPH_SCOPES` set by
    incremental consent, so a **single** re-auth clears **both** the calendar and mail prompts at
    once. The mail prompt self-clears when the next write/send succeeds. The whole re-request flow
    is logged for support (which scopes were requested, re-consent-for-existing vs. account-picker,
    and the outcome) **without ever logging the address** (per [`logging.md`](logging.md)).

12. **A grant that is *gone* is a different prompt from a scope that is missing, and it is never
    an outage.** Rule 11 covers a grant that is too **narrow**; this covers one that is **dead**:
    the refresh token expired or was revoked (Google `invalid_grant — Token has been expired or
    revoked`, a Microsoft `AADSTS700082`, a withdrawn OAuth JMAP token), so it mints no access
    token at all and **nothing** about the account syncs. It is classified **structurally**, on the
    engine's own `FailureClass::Authentication` rather than on error text, so every provider
    adapter's flavour of "your credential is no good" is covered the day it is added, including a
    refused IMAP password, and raises `signin_expired_accounts` on `Surface::Connectivity`. Three
    consequences bind every client: an account listed there is **excluded** from
    `unreachable_accounts` (the server *was* reached and answered: "can't reach this account's
    server" would be a lie, and the two prompts would contradict each other); only a **successful**
    sync retracts the prompt, never a transport failure or a concurrent-sync skip, since neither is
    evidence the credential works; and the button asks `MailcalApp::account_provider` which sign-in
    to launch, because unlike rule 11 this can hit a Microsoft, Google, JMAP **or** password
    account: the ones without a browser flow are pointed at Settings instead. Like the re-consent
    prompts it is **not** suppressed while offline, and the raise/clear is logged for support with
    a count and a reason, **never** the address.

    **"Settings" must be a remedy, not a shrug.** That wording is correct only where a credential
    can actually be re-entered: a password account. A **JMAP** account is two different things
    behind one protocol: one connected by *signing in* has a flow to re-run, one holding a pasted
    password/API token does not, and pointing the first at Settings sent the user to a screen that
    could not fix it. The only cure left was to remove the account and add it back, discarding the
    local store to repair a credential. So `account_provider` reports the two families apart
    (`Jmap` · `JmapOauth`), and every client offers the button for the OAuth one; see rule 14.
    For a password or pasted JMAP secret, `replace_account_secret` rebuilds the account from its
    existing endpoints with only that secret changed, connects it before writing anything, then
    replaces the credential through `AccountCredentialStore` and runs a catch-up. A refusal leaves
    both the registered account and its stored credential untouched.

    **A refusal that something else contradicts never reaches the user.** One credential serves
    every scope on an account, so a scope that authenticated in the same pass disproves an expired
    one; and servers do refuse a valid credential (a rejection deliberately delayed by ~2s is not a
    timeout). So the **sync** path raises the prompt only when *nothing* on the account reached, and
    a mixed pass moves it in neither direction: no evidence to raise, and none to retract one the
    user still has to act on. The price is that a credential expiring mid-pass is reported one pass
    later, which is the right trade for the one prompt a user cannot ignore.

    The **connect** path obeys the same rule, because a dial is not one login: an IMAP account
    authenticates its INBOX, lists folders over that session, then opens a connection per role
    folder. Only the **first** login can prove the stored credential wrong: a refusal in the folder
    loop is contradicted by the INBOX that authenticated seconds earlier with the same password, so
    it fails the dial as an outage and raises nothing. That is a property of *where* the verdict is
    built (`AccountError::from_first_imap_login`), not a check a caller can forget: every other
    conversion of the same engine error keeps the family's own variant. JMAP has no such sequence
    (session discovery authenticates on every connect), so its mail connect is its first login.

    **It must be classified at *both* ends, because a dead grant usually never reaches a sync.**
    The interactive app connects **nothing** synchronously at boot (it paints cached mail and
    dials every account in the background), so a revoked token surfaces in the bindings' reconnect
    pass, which fails the *connect* and therefore never runs a folder sync for the classifier to
    see. Classifying only the sync path leaves the commonest case badged as an outage, which is
    precisely the bug this rule exists to kill. So: the sync path classifies on the engine's
    `FailureClass::Authentication`, and the connect path classifies on
    `AccountError::SigninRejected`, the account layer's **distinct variant** for exactly this
    purpose. Neither may be re-derived from the rendered message: the connect boundary flattens its
    error to a string, so the verdict is decided **at** the `AccountError` and carried as a field
    (`ConnectFailure::signin_expired`).

    **And it must be classified for every kind of credential, not only a token.** An OAuth-shaped
    verdict at the connect end badges an account whose *password* the server refused as "can't reach
    this account's server", on every launch, and never points at the Settings field that would fix it:
    the lie this rule exists to forbid. Setup does not cover that case: a wrong password while
    *adding* an account is reported by the setup flow, which an already-added account never runs
    again. So the variant is named for the verdict (`SigninRejected`), not for a token, and each
    family maps its own typed refusal onto it at the boundary where that type still exists.

13. **A sign-in abandoned in the browser must never wedge the button: a second request
    supersedes the first.** Closing the browser tab is **undetectable**: neither a loopback
    listener nor a custom-scheme rendezvous receives any signal, so the flow waits out its cap
    (five minutes). A client that refuses a second attempt while one is "already running"
    therefore presents a button that silently does nothing for minutes, and the only escape a
    user finds is restarting the app, which is exactly what a reconnect prompt must not require,
    since the prompt exists to make the remedy one tap. So a fresh request **cancels the
    outstanding flow, waits for it to unwind, and starts a new one**. Cancel-then-wait is not
    optional: the redirect rendezvous is a single slot per client (on Windows
    `ProtocolAuthCallback` holds one static pending registration, shared by the Microsoft and
    JMAP flows), so starting before the old flow releases it would orphan one of the two. A form
    that disables its own buttons while submitting is **not** a substitute: the prompt that
    needs this is on another screen entirely.

14. **Re-authorising an existing OAuth JMAP account replays its persisted grant; it never
    re-discovers or re-registers.** `begin_jmap_reauth(account_id)` reads that account's own
    `[jmap.oauth]` (the authorization endpoint, the RFC 7591-registered `client_id`, the redirect
    URI, the scopes, the RFC 8707 resource indicator) and builds a fresh PKCE authorisation from
    exactly those, with **no network calls at all**. That is what those fields are persisted for
    (`docs/jmap.md` rule 3, "discovery runs once"); re-running registration instead would mint a
    **second** client id on the user's provider account for every reconnect and orphan the first.
    `complete_jmap_reauth(account_id, …)` then swaps the grant in place: it connects **before** it
    persists, writes the host's secure store through the **same** `AccountCredentialStore` port a
    token rotation uses (so there is one storage path, and the client makes no store write of its
    own), retracts `signin_expired_accounts`, and runs a catch-up refresh rather than a first
    download. It **refuses a sign-in completed as a different address** than the account being
    reconnected: `login_hint` targets an address, it does not pin one, so someone with two
    mailboxes at one provider can come back as the wrong one, and filing that grant under the
    original account id would point it at another person's mail while still displaying the original
    address. Because the account keeps its identity, settings and downloaded mail, a client calls
    neither `add_account` nor its own secure-store write on this path.

15. **The client registrations are injected at build time, and a build without one does not offer
    that route.** Google's and Microsoft's client ids (and Google's non-confidential Desktop
    secret) enter the product in exactly one place, `mailcal_oauth::credentials`, which reads them
    from the environment with `option_env!`. No client holds one; `begin_google_login` and
    `begin_microsoft_login` take no client id at all.

    | Target | Variables |
    |---|---|
    | macOS, Windows, Linux | `MAILCAL_GOOGLE_DESKTOP_CLIENT_ID`, `MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET` |
    | iOS, iPadOS | `MAILCAL_GOOGLE_IOS_CLIENT_ID` |
    | Android | `MAILCAL_GOOGLE_ANDROID_CLIENT_ID` |
    | every target | `MAILCAL_MS_CLIENT_ID` |

    Google's three exist because Google issues a **separate client per client type**, and the type
    decides the redirect and whether a secret is involved at all (see "## Google" below). Values
    come from the environment first and the repo's gitignored `.env` second; `BUILDING.md` has the
    developer and CI setup.

    **Absent is a supported build, not a broken one.** `oauth_routes()` reports which routes exist;
    a client shows only those, so the wizard offers IMAP / JMAP / CalDAV / CardDAV and no dead
    button. Detection follows the same answer: a Gmail or Workspace address falls back to the IMAP
    app-password route it would otherwise supersede, while a Microsoft-hosted one reports
    `OauthOnlyProvider`: Microsoft retired Basic auth, so there is no working password route to
    fall back to.

    Two things are derived rather than injected, and both would otherwise drift. Google's mobile
    redirect is the client id with its dotted parts reversed, so the core computes it and hands it
    to the host through `oauth_routes()`; Android's manifest `intent-filter` needs the same string
    as a literal, so `build.gradle.kts` derives it from the same variable into the
    `googleRedirectScheme` placeholder. Microsoft's redirect is **not** derived: Azure registers
    it against each host's own bundle/package identity, so it stays with the client.

    **A build we ship may not be missing one.** `MAILCAL_REQUIRE_INJECTED_CONFIG=1` turns an
    absent registration into a compile error naming the variables the target needed, so no artifact
    is produced rather than one that is correct everywhere except in front of a user. Every
    packaging path sets it and nothing else does: off by default, because a from-source build
    without credentials stays supported. The Linux Flatpak is the awkward one: its cargo runs inside
    a sandbox that forwards no host environment, so `clients/linux/package.sh` puts the values into
    the tree flatpak-builder copies. See [`BUILDING.md`](../BUILDING.md).

## Google (Gmail + Google Calendar)

Google is a **native-API** integration (the engine's Gmail + Google Calendar adapters), not
IMAP/CalDAV, so account setup routes straight to this OAuth flow, never to server-settings
autodetection. It reuses the whole state machine above; the deltas are:

- **`access_type=offline` + `prompt=consent`, not `offline_access`** (rule 6). Google issues a
  refresh token only with `access_type=offline`, and re-prompting consent on every authorization
  guarantees one comes back even for an already-consented account. The core still treats a
  completed sign-in with no refresh token as an error.
- **Full scopes: `https://mail.google.com/` + `https://www.googleapis.com/auth/calendar`.** The
  full-mail scope is required for send and permanent-delete; the calendar scope is granted **in
  the same consent**, so there is **no calendar-reauth step** for Google: the reconnect-for-
  calendar banner stays Microsoft-only. (Microsoft grants Mail and Calendars.ReadWrite together
  too, but its calendar arrived later behind a scope-upgrade reconnect; Google ships both at once.)
- **Redirect differs by client type (rule 3).** **iOS/iPadOS and Android** register a Google
  **iOS/Android client**, whose redirect is the **reversed-client-id custom scheme**
  `com.googleusercontent.apps.<CLIENT_ID>:/oauth2redirect`, where `<CLIENT_ID>` is the **whole**
  client id (everything before `.apps.googleusercontent.com`, **including the numeric
  project-number prefix**) with its dotted parts reversed; drop the prefix and Google fails the
  request with **`redirect_uri_mismatch`**. The **Android** client additionally needs **"Enable
  Custom URI scheme"** switched on in its Console *Advanced settings*, off by default, and without
  it Google returns **`invalid_request`: "Custom URI scheme is not enabled for your Android
  client."** (The scheme is safe under our mandatory PKCE: an intercepted code is useless without
  the in-process verifier. App Links would be the hijack-proof hardening; see Known gaps.)
  **macOS, Windows, and Linux register a
  Google _Desktop_ client**, which does not support custom schemes: it uses a **loopback redirect**
  `http://127.0.0.1:<ephemeral-port>/` captured by a one-shot local listener the client opens
  *before* minting the authorization URL (the port is baked into the redirect, so it must be chosen
  first): a `.NET HttpListener` on Windows, an `NWListener` on macOS, and a bounded Rust
  `TcpListener` on Linux. Google **deprecated** the
  loopback redirect for the *iOS* client type and recommends it for desktop, which is why macOS (a
  desktop app) uses a Desktop client rather than sharing iOS's custom-scheme flow; all three
  desktop hosts reuse the same Desktop client (loopback is not OS-bound).
- **Client secret: none for iOS/Android, a non-confidential one for the Desktop client (rule 1).**
  The **iOS/Android** clients are true public clients: Google's token endpoint takes the PKCE
  exchange with only the client id. A Google **Desktop** client (the **macOS/Windows/Linux** loopback
  flow) is *also* a public client, but Google's token endpoint nonetheless **rejects** the PKCE
  exchange (and the refresh) with `invalid_request — client_secret is missing` unless the Desktop
  client's secret is sent. So a desktop build is given that secret
  (`MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET`, rule 15) and the core sends it on **both** the code
  exchange **and** the refresh (it is persisted in the account config so a refresh a launch later
  still has it: miss the refresh and the account breaks ~1h after setup). That value is **not
  confidential**: Google's installed-app guidance says it "is obviously not treated as a secret," it
  is embedded in the app's source because an installed binary cannot keep one, and it grants nothing
  without a fresh PKCE verifier + user consent: PKCE stays the real protection. See
  <https://developers.google.com/identity/protocols/oauth2#installed>. (This is why **Android works
  with no secret while macOS/Windows failed until the Desktop secret was wired in**: a
  client-*type* difference, not a code-path one.)
- **Refresh tokens are long-lived and do not rotate on a refresh grant.** So re-persisting a
  Google rotation through the shared `AccountCredentialStore` (rule 5) is a backstop that rarely
  fires; but it is not optional and cannot be skipped, because Google *can* re-issue a refresh
  token (e.g. on re-consent) and every core takes the one store at construction.

**Early Access gate (client-side, above this contract).** While Google reviews the app for the
restricted scopes, only allow-listed **OAuth test users** can complete the flow; anyone else hits
a hard `access_denied` at Google's consent screen. So every client shows an **Early Access notice**
on the Google sign-in surface: what it is, a link to the sign-up form
(`https://mailcal.allodia.eu/forms/google-early-access`), and a **mandatory confirmation checkbox
that keeps the "Sign in with Google" button disabled until the user ticks it**. This is a UX
guard, not a security boundary (Google enforces the allow-list); the core is unaware of it. Remove
the gate once the app is verified in production.

## Per-platform matrix

Legend: ✅ implemented · 🚧 code-complete, runtime unverified · ⬜ planned · — n/a.

| Gate | Shared core | Apple (macOS/iOS/iPadOS) | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| PKCE + no client secret | ✅ | ✅ | ✅ | 🚧 | ✅ |
| System browser (no in-app WebView) | — | ✅ `ASWebAuthenticationSession` | ✅ default browser + protocol activation | 🚧 Chrome Custom Tabs | ✅ default browser + loopback |
| Redirect capture | — | ✅ custom scheme | ✅ custom-scheme protocol activation | 🚧 custom-scheme `intent-filter` | ✅ `127.0.0.1` Rust `TcpListener` |
| `state` validated on callback | ✅ | ✅ | ✅ | 🚧 | ✅ |
| Refresh token only, in OS keystore | ✅ | ✅ Keychain | ✅ Credential Manager | 🚧 EncryptedSharedPreferences | ✅ Secret Service |
| Token refresh + rotation persistence | ✅ refresh · rotation **verified live** (Graph + JMAP both rotate) | ✅ | ✅ refresh · rotation **verified live** `CredentialStoreSink` (Graph + JMAP) | ✅ verified on-device | ✅ verified live (Graph + JMAP) |
| One refresh in flight per account (no replay on a ratcheting server, rule 5) | ✅ | — | — | — | — |
| A rotation is persistable from the first refresh (the store is a constructor param on **both** constructors, rule 5) | ✅ | ✅ | ✅ (no worker; the app itself) | ✅ | ✅ |
| The **core** owns add/persist/rollback/erase; the host only implements `persist` + `delete` (rule 5) | ✅ | ✅ Keychain reports `OSStatus` | ✅ `CredentialStore` returns success | ✅ throws on a Keystore refusal | ✅ Secret Service reports a refusal |
| Re-consent prompt for a withheld scope (calendar boot-probe · mail write/send point-of-use) | ✅ | ✅ | 🚧 | 🚧 | ✅ |

**Google clients** (same core gates; the redirect row differs per the "## Google" deltas):

| Gate | Shared core | Apple (macOS/iOS/iPadOS) | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| PKCE state machine (`access_type=offline`) | ✅ | 🚧 | 🚧 | 🚧 | ✅ |
| Client secret on token exchange | ✅ optional (sent on exchange + refresh when present) | 🚧 iOS none · macOS non-confidential Desktop secret | 🚧 non-confidential Desktop secret | 🚧 none (Android client) | ✅ non-confidential Desktop secret |
| System browser (no in-app WebView) | — | 🚧 iOS `ASWebAuthenticationSession` · macOS default browser + loopback | 🚧 default browser + loopback | 🚧 Chrome Custom Tabs | ✅ default browser + loopback |
| Redirect capture | — | 🚧 iOS reversed-client-id scheme · macOS `127.0.0.1` loopback `NWListener` | 🚧 `127.0.0.1` loopback `HttpListener` | 🚧 reversed-client-id `intent-filter` | ✅ `127.0.0.1` Rust `TcpListener` |
| Refresh token only, in OS keystore | ✅ | 🚧 Keychain | 🚧 Credential Manager | 🚧 EncryptedSharedPreferences | ✅ Secret Service |
| Rotation persistence (the shared `AccountCredentialStore`) | ✅ | 🚧 | 🚧 | 🚧 | ✅ |
| Core-owned add/persist/rollback/erase (rule 5) | ✅ | 🚧 | 🚧 | 🚧 | ✅ |
| Early Access notice + mandatory confirm checkbox | — | 🚧 | 🚧 | 🚧 | ✅ |

**Provider-agnostic** (rule 12: it fires for a dead credential of *any* kind, so it belongs to no
single provider's table):

| Gate | Shared core | Apple (macOS/iOS/iPadOS) | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Expired sign-in prompt (`signin_expired_accounts`, classified on `FailureClass::Authentication`) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Prompt routes to the right sign-in via `MailcalApp::account_provider` (Settings where there is no browser flow) | ✅ | ✅ | ✅ Google · OAuth JMAP · Microsoft untested | ✅ | ✅ Microsoft · Google · OAuth JMAP runtime-verified; stored-secret repair code-complete |
| A second sign-in request supersedes one abandoned in the browser (rule 13) | — | ✅ never guarded | ✅ `SignInFlight` | ✅ never guarded | ✅ `AttemptSlot` |
| OAuth **JMAP** re-authentication in place from the prompt (rule 14: `begin_jmap_reauth` / `complete_jmap_reauth`) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Password / pasted JMAP secret replacement in Accounts, connect-before-persist (`replace_account_secret`) | ✅ | ⬜ | ⬜ | ⬜ | ✅ |

Each client fills its Azure **client id** in its `MicrosoftOAuth.{swift,kt,cs}` and registers a
redirect: **Apple** the custom scheme `msauth.eu.allodia.mailcal://auth` across macOS, iOS,
and iPadOS; **Android** the MSAL-format `msauth://<package>/<signature-hash>` (consistent with
Microsoft Authenticator);
**Windows** the custom scheme `eu.allodia.mailcal://auth`, delivered by OS **protocol activation**
(declared in `Package.appxmanifest` for packaged builds, registered at runtime against the current
exe for the unpackaged dev loop); **Linux** the **loopback** `http://127.0.0.1:<ephemeral-port>/`,
captured by the same bounded one-shot `TcpListener` its Google and JMAP sign-ins use. Linux claims
no URI scheme: a custom-scheme handoff there needs an installed `.desktop` entry and
single-instance URI routing, which the dev loop does not have, so ⚠️ **`http://127.0.0.1` must be
registered as a redirect** on the Azure app registration under "Mobile and desktop applications".
Azure matches a loopback redirect without its port (RFC 8252 §7.3), which is what makes an
ephemeral port workable; the literal `127.0.0.1` is registered rather than `localhost` because the
listener binds IPv4 and a host resolving `localhost` to `::1` first would never reach it. **A real
Microsoft sign-in completed on Linux on 2026-08-19** with that entry registered: browser consent,
loopback callback, token exchange and address lookup in 563 ms, six Graph folder providers plus the
calendar connected, and the grant written to Secret Service. The Windows scheme carries no `msauth` prefix on purpose: that
prefix is an MSAL-SDK requirement on Apple/Android, and the Windows client rolls its own PKCE flow
(no MSAL), so any registered custom scheme works.

**Linux re-consent completed live on 2026-08-25.** A stored `Mail.Read`-only grant raised the
calendar banner at launch; a refused mail action and `sendMail` raised the mail banner. One targeted
Reconnect requested the full ten-scope set and cleared both. Mail action/send and calendar
create/delete then succeeded, and a cold restart refreshed, rotated and re-persisted the upgraded
grant before reconnecting mail and calendar without either banner.

**Linux OAuth JMAP re-authentication completed live on 2026-08-25.** With the Fastmail grant
withdrawn, a cold launch classified `invalid_grant` as an expired sign-in and raised the account's
prompt. **Sign in again** replayed the persisted authorization endpoint, client, redirect, scopes
and resource without discovery or registration; the callback replaced the credential in place,
Fastmail rotated it, Secret Service persisted it, mail and calendar reconnected, and the prompt
cleared. A second cold launch refreshed, rotated and persisted the recovered grant again.

**Windows registers two schemes, and Azure must list both.** A packaged build claims
`eu.allodia.mailcal`; an **unpackaged dev build claims `eu.allodia.mailcal.dev`** instead
(`MicrosoftOAuthConfig.PackagedScheme` / `UnpackagedScheme`, selected by `AppIdentity.IsPackaged`,
the same predicate that decides who registers it, so the scheme a build *claims* and the one it
*sends* cannot drift). This is not tidiness: Windows registers a protocol **per user, not per
build**, so on any machine with the Store app installed *and* a dev build run (every Allodia
developer's machine) both claimed the same scheme, and the OS could only respond by putting up a
"select an app" **picker for a redirect carrying a live one-time auth code**. Whichever the
developer picked got the grant; a stray "Always" would have wired every future sign-in, *including
the shipped app's*, to the wrong build, silently and permanently. Both redirect URIs are registered
in the Azure app registration. JMAP needs no such entry: RFC 7591 dynamic registration sends
whatever redirect URI the client hands it, so a dev build registers itself under the dev scheme
with no portal step. Google is unaffected (loopback, not a custom scheme).

The Apple path is code-shared; macOS is runtime-confirmed, while iPhone/iPad use the same
`ASWebAuthenticationSession` host and remain behind the Apple background-delivery follow-up before
they are marked shipped.

**Windows completed a runtime sign-in on 2026-07-21**: a real account through the browser, the
redirect delivered by protocol activation, the grant stored, and a **relaunch** re-authenticating
from it (the refresh path: the token source always starts expired, so the first call after a launch
necessarily refreshes). A forged callback was also rejected as a possible CSRF, which is the `state`
row above proven rather than assumed. Two rows stay 🚧 there because this pass did not exercise
them: **rotation** persistence (no server has rotated one yet) and the **re-consent prompt**. The
Microsoft row in the README capability matrix likewise stays 🚧 for Windows, since it also covers
mail actions, send and calendar writes that were not touched. **Android is 🚧**: code-complete (it
compiles via `assembleDebug`), pending a runtime sign-in. A new platform must meet every core-side
gate above and register its own redirect.

For **Google**, each client fills its own **Google Cloud Console OAuth client** in
`GoogleOAuth.{swift,kt,cs}` and Linux's `ui/google.rs`: **iOS/iPadOS** and **Android** an *iOS/Android* client (redirect =
reversed-client-id scheme `com.googleusercontent.apps.<CLIENT_ID>:/oauth2redirect`, where
`<CLIENT_ID>` is the *whole* client id (project-number prefix included) reversed; Android also
declares that scheme in an `AndroidManifest` `intent-filter` and needs **"Enable Custom URI
scheme"** switched on in the client's Console *Advanced settings*); **macOS**, **Windows**, and **Linux** a
*Desktop* client (loopback `http://127.0.0.1:<port>/`, a public client that additionally embeds
Google's **non-confidential** Desktop `client_secret` (required by Google's token endpoint; see the
"## Google" deltas), macOS via `GoogleLoopbackFlow` (an `NWListener`), the Swift sibling of the
Windows `GoogleLoopback`; Linux uses a bounded one-shot Rust `TcpListener`. All three reuse the same
Desktop client id **and** secret. The clients ship as
**🚧**: real client ids are wired in,
pending Google Cloud Console app **verification** for the restricted scopes. Verified so far:
bindings, both Apple app builds (macOS + iOS), the macOS loopback listener mechanics (bind +
capture), the iOS/macOS/Android Early-Access gate at runtime, and, on **Android**, on a real
device against an Early-Access-allow-listed account, the **full OAuth round-trip end to end**
(consent → token exchange → Gmail read + Google Calendar connect → first sync). On **Linux**, a
real Early-Access-allow-listed account completed the system-browser → loopback → token-exchange →
Secret-Service-save path and loaded its Gmail inbox on 2026-07-22; calendar sync, relaunch refresh,
and rotation persistence remain to be exercised there.

## Sovereignty scope

Account connection is **not** jurisdiction-gated. The `JurisdictionGate` governs AI/model
dispatch (which models may run), **not** which mail accounts a user may connect: a user may
connect any provider wherever it is hosted. (This is the product decision of record; it narrows
the doctrine's "provider sync" language for *account connection* specifically.)

## Known gaps

- **Graph mail: read/sync + mail actions + sending.** The engine's Graph adapter does mail folders
  + messages + message source (bodies render via `/messages/{id}/$value`) + a `receivedDateTime`
  sync-depth window, mail edits (`edit_mail`: mark-read/flag, move/archive, permanent
  delete), **and** submission (`submit_email` → `POST /me/sendMail`, which files the Sent copy
  itself), all outbox-mediated like IMAP/JMAP. Calendar read/sync + write also ships (see
  [`docs/calendar.md`](calendar.md)). One deliberate asymmetry in the token-refreshing wrapper: a
  send is never re-issued on a transport error the way the idempotent reads/edits are: a resent
  `sendMail` double-delivers, and a retryable transport error can't distinguish a lost-before-send
  from a lost-after-send, so the wrapper drops its (possibly dead) socket and lets the **outbox**
  own the retry, parking an ambiguous loss for the user to confirm rather than risk a duplicate.
- **Gmail: read/sync + mail actions + sending.** The Gmail adapter does mail folders + messages +
  message source + a received-date sync-depth window, mail edits (`edit_mail`: mark-read/flag,
  move/archive, permanent delete), **and** submission (`submit_email` → `messages.send`, which
  files the Sent copy and returns the sent id directly), plus Google Calendar read **+ write**,
  all outbox-mediated like IMAP/JMAP/Graph. The token-refreshing wrapper carries the same
  send asymmetry as Graph's: a send is never re-issued on a transport error (a resent
  `messages.send` double-delivers), so the wrapper drops its suspect socket and lets the outbox
  own the retry. **Archive is the one verb with no Gmail folder behind it**: Gmail has no
  Archive place, archiving is the *absence* of the Inbox label, so the core resolves Archive to
  the account's `\All` mailbox (`resolve_move_target`) and the engine turns a move there into a
  label removal that adds nothing. Without both halves archive is a silent no-op: the row leaves
  the list optimistically and comes straight back.
- **The mail write/send re-consent prompt is reactive, not proactive.** A Graph account missing
  `Mail.ReadWrite`/`Mail.Send` (connected before those scopes, or consent revoked server-side) is
  now caught and prompted (see rule 11) but, unlike the calendar scope's **boot-time probe**,
  mail reads fine on the read-only grant and there is no cheap idempotent write probe, so no banner
  appears until the user actually attempts a mark-read / move / delete or a send: that first action
  fails (surfaced as a `Failed` hint and logged), and *then* the reconnect banner appears. A
  reconnect for any reason (including the calendar prompt) re-requests the whole scope set and
  clears it ahead of a first failure.
- **Both ends of rule 12 are verified on a device, except the mixed *sync* pass.** The three legs that
  matter were driven on real hardware (Android, 2026-08-10) with
  `scripts/dev/imap-fault-proxy.py` refusing a login the harness would have accepted: `--refuse-all`
  raised the prompt pointing at Settings with **no** outage badge
  (`sign-in refused by the server: sign-in rejected: IMAP authentication failed:
  [AUTHENTICATIONFAILED]`); `--refuse-every 5` refused the **fourth role folder** after the INBOX had
  authenticated (`conn[1] login forwarded … conn[5] login REFUSED`) and correctly raised **nothing**,
  badging the account unreachable instead; and letting the same login through again retracted the
  prompt (`an account's sign-in works again`). What is still unreached on a device is a mixed **sync**
  pass: one folder refused while others sync. IMAP holds its authenticated session per folder, so a
  refusal can only land on a connection being *established*, and every connection of a re-dial belongs
  to the same dial; producing it needs the injector to drop an *established* connection so one folder
  reconnects mid-life. Not built: the rule itself is covered by tests at two levels.
- **A non-token Graph or Google refusal is still not classified.** Both families mint their credential
  through the shared token source, so a dead grant arrives as `SigninRejected` and is covered. A `401`
  from the `/me` (or profile) lookup made **with a freshly minted token** is a different animal: it
  arrives as `AccountError::Graph` / `Google` and badges an outage. Left alone deliberately: it means
  the server accepted the token and refused the call, where a retry is at least plausible, and no
  such failure has been observed. If one is, it classifies where the others do.
- **Rules 12 and 13 have no automated Windows coverage, only a manual pass.** Both were verified
  by hand on a Windows host (2026-07-27): the grant revoked at
  <https://myaccount.google.com/permissions>, the banner raised on the next launch, its button
  completing a real re-auth, and (closing the browser tab mid-flow) a second click superseding
  instead of doing nothing. That closes the *compile* and *runtime* gaps, so the matrix rows are ✅.
  What remains is that nothing **automated** guards the WinUI half: `Mailcal.Tests` targets
  `net10.0` and cherry-picks the non-XAML sources, so `MailboxModel.Connectivity.cs`, the
  `MailListView.xaml` `InfoBar`, and the three call sites `SignInFlight` serializes are only ever
  checked by a human on a Windows box. The decision halves (`SignInFlight`) *are* unit-tested
  everywhere. A UIA pass on the `SignInExpiredBanner` / `SignInExpiredAction` automation ids is the
  cheapest way to close it.
- **Rule 14 is verified on all four platforms.**
  The four 2026-08-01 passes used a real Fastmail account and covered both ways a grant dies:
  Android after the **ratchet** revoked it, macOS and iOS after a **manual revoke** in Fastmail's
  Connected-apps UI: different `error_description`s (`ratchet or client_id mismatch` vs `oauth
  token not found`), both classified correctly, which is the structural-`invalid_grant` rule
  paying off. Each exercised every load-bearing property: the prompt routed to the button (not
  Settings), the authorisation opened with no network call, the exchange carried the resource
  indicator, the rotated refresh token reached the platform keystore (on macOS the Keychain item's
  modification time matches the re-auth to the second), the prompt retracted, and the account
  resumed with a catch-up instead of re-downloading. The passes also proved rule 5's logging on
  real devices: Google *kept* its refresh token while Graph and JMAP both **rotated** theirs, in a
  single boot.

  **Windows joined them the same day**, against the same account after the macOS/iOS revoke had
  killed it, so the Windows pass ran on a grant that was *already* dead when the app first saw
  it (`invalid_grant — oauth token not found`, classified on the boot reconnect, not on a sync).
  It proved the same properties: the banner carried the **button** wording rather than "update it
  in Settings" (the whole point of `AccountProvider::JmapOauth`) with `SignInExpiredAction`
  visible and enabled; `begin_jmap_reauth` logged only its stored authorization endpoint and made
  **no** network call; the exchange carried the resource indicator
  (`https://api.fastmail.com/jmap/session`); the prompt retracted; and the account resumed with a
  **2-message catch-up** (37 → 38 rows), not a re-download. **8.2 s** end to end. It also
  exercised rule 13 for free: the first attempt was abandoned in the browser and the second
  click superseded it (`jmap re-authentication cancelled`, then a fresh begin) rather than
  wedging on `ProtocolAuthCallback`'s single pending slot.

  The Windows analogue of the macOS Keychain-mtime check is the **Credential Manager blob**: read
  through `CredRead`, the account's `LastWritten` and the SHA-256 of its blob both moved at each
  rotation (19:35:16 re-auth, 19:38:22 the next boot's refresh), and a **cold relaunch connected
  with no prompt at all**, which is the property that actually matters on a ratcheting server,
  since it proves the token the store holds is the one the server has *not* moved past. The same
  boot re-persisted a rotated **Graph** token too, so the Microsoft table's rotation cell moved
  with it.
- **Rule 14's *failure* half is still unrun on Windows.** A re-authentication that the server
  refuses appends `signin_expired_failed` to the banner (`MailboxModel._signInReauthFailed`), so
  the click never looks like it did nothing. The 2026-08-01 pass only ever succeeded, so that
  sentence has not been on screen on Windows, and, like everything else in
  `MailboxModel.Connectivity.cs`, no automated test can reach it (`Mailcal.Tests` is a plain
  `net10.0` assembly; see the rules 12–13 bullet above). The *core* half (refusing a sign-in
  completed as a **different address**) is unit-tested in `jmap_oauth/reauth.rs`, so what is
  untried is the WinUI wording, not the decision.
- **A re-authorisation replays the persisted client registration, so a server that has forgotten it
  would dead-end.** (Fastmail does **not**, verified 2026-08-01: the replayed `client_id` was
  accepted after a full grant revocation, with no `invalid_client`.)
- **Reusing the client id does not stop the provider's "connected apps" list from growing.** Rule
  14 sends no RFC 7591 registration, but Fastmail lists one entry per **authorisation**, not per
  registered client, so every re-authentication still adds a row, on top of the one each setup
  sign-in leaves ([`jmap.md`](jmap.md) → "Dynamic registration is cached for the session only").
  A real account showed **eight** entries after ~two weeks of testing. Nothing breaks, and the
  user can revoke them, but it is untidy in a surface that exists for the user to audit who has
  access to their mail. The fix is not to re-register less: it is to **revoke the old grant** on
  a successful re-authentication, via the RFC 7009 `revocation_endpoint` when the authorization
  server's RFC 8414 metadata advertises one (we already parse that document). Deliberately not
  done here: it is a network call on a path that has just recovered a broken account, and it
  wants its own change with its own failure handling: a revoke that fails must never fail the
  re-authentication that just succeeded. Rule 14 deliberately does not re-run RFC 7591, which means a provider that deletes
  the dynamically-registered client when the user revokes the app answers the authorization request
  with its own `invalid_client` error page. The remedy is then what it was before this rule existed:
  remove the account and add it back. Not fixed speculatively: re-registering on every reconnect
  has a certain cost (a second client id per attempt, orphaning the first) against a hypothetical
  one, and no server we test against is known to behave this way. Revisit if one turns up.
- **Two Windows paths within rules 12–13 are still unexercised.** The manual pass used a **Google**
  account, so the prompt's **Microsoft** branch (`SignInWithMicrosoft` via `account_provider`) has
  not been run on Windows; and the **JMAP setup-form** sign-in (re-routed through the shared
  `SignInFlight` in the same change, because it shares `ProtocolAuthCallback`'s single static
  pending registration with the Microsoft flow) was not re-tested after that re-routing. Neither
  is a known break; both are simply untried, which is why the `account_provider` row reads
  "Google · Microsoft untested" rather than a bare ✅.
- **The headless boot path still badges a dead grant as an outage.** Rule 12 classifies the two
  paths a real client takes: the background reconnect (`AccountError::TokenExpired`) and the sync
  pass (`FailureClass::Authentication`). The **synchronous** boot connect, used only by the
  headless background-sync worker, records its failures as `(id, detail)` strings before anything
  can classify them (`record_stored_outcome`), so an expired account is badged unreachable there.
  It is deliberately left: that path has no UI to show a prompt on, and the account is reclassified
  the moment the interactive app next dials it. Threading the typed error through would be a wider
  refactor of the boot outcome plumbing.
- **Google runtime sign-in: Android verified end-to-end; Linux Gmail sign-in verified; the other
  hosts and remaining Linux paths stay 🚧.** The full
  OAuth round-trip is proven on **Android** (real device, allow-listed account, Gmail + Google
  Calendar connect + first sync). **Linux** has likewise proven a real allow-listed system-browser
  sign-in through loopback capture, token exchange, Secret Service persistence, and Gmail first
  sync; calendar sync, relaunch refresh, and token rotation have not yet been exercised there.
  iOS/iPadOS, macOS, and Windows are code-complete with real client ids but their round-trips are
  not yet runtime-verified. All Google clients remain gated on Google's app **verification** for
  the restricted scopes; until then only Early-Access allow-listed testers can sign in.
- **Android custom-scheme redirect is not hijack-proof (App Links follow-up).** The Android client
  uses a reversed-client-id custom scheme, which any app can register. PKCE makes an intercepted
  code useless, but the hijack-proof option is **App Links**: a verified `https://` redirect on a
  domain we own, with a hosted `/.well-known/assetlinks.json`. Deferred: overkill for Early Access,
  worth doing before general availability.
- **Windows and Android** Microsoft sign-in is code-complete but not yet runtime-verified on-device
  (🚧). iPhone/iPad share the Apple auth host but remain behind the same Apple shipping gate as the
  rest of the iOS/iPadOS foreground client.
- **Linux's stored-secret credential remedy remains live-account unverified.** Password and
  pasted-secret JMAP accounts expose an in-place replacement in Settings → Accounts. Its rollback
  and UI routing are automated, but it has not been driven against a real expired Linux account yet.
- **Apple, Windows, and Android still tell stored-secret accounts to use Settings without offering
  a credential editor there.** The shared connect-before-persist operation exists, but those
  clients have not exposed it in Accounts yet; a refused password or pasted JMAP secret still
  requires remove-and-re-add on them.
- **Windows leaves a dangling browser tab.** Apple (`ASWebAuthenticationSession`) and Android
  (Custom Tabs) auto-dismiss their session on the redirect; the Windows **default browser** has no
  API to close its own tab, so after the custom-scheme hand-off the app raises itself but a blank
  tab is left behind. This is inherent to reusing the default browser (required to inherit the
  signed-in Microsoft session); the only auto-dismissing alternative, `WebAuthenticationBroker`,
  uses its own cookie jar and would lose that session, so it is deliberately not used. The
  **Google** macOS, Windows, and Linux loopback flows (`NWListener` / `HttpListener` / `TcpListener`) have the same
  dangling-tab limitation, but their listener at least serves a small "you can close this tab" page
  on the redirect rather than a raw browser error.
