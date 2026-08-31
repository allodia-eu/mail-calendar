# Entitlement: the cross-platform contract

What a client may draw, and how it behaves when it cannot ask. Everything here is under the
[Allodia License](LICENSE.md); the free application has no entitlement and never asks, which is
[`docs/pledge.md`](../docs/pledge.md)'s fourth promise made structural rather than promised.

**Sovereignty scope.** Signing in and asking for an entitlement dispatch **only** to Allodia's own
EU-hosted account service, whose address is fixed at build time by the injected client registration,
so there is nothing for the `JurisdictionGate` to decide and it would be a no-op, in the style of
the 2026-07-11 analytics carve-out; see [`../AGENTS.md`](../AGENTS.md). A build with no registration
baked in (every build from source) has no Allodia sign-in at all and reaches the service never, so
an air-gapped or self-hosted deployment behaves identically to a connected one.

Re-ratified 2026-08-27, when the service's sign-in page gained Apple and Google beside an ordinary
email and password. That is an identity provider one step further out than this rule reaches: the
person's browser goes there, only if they choose it over the plain route, and what the app sends is
unchanged. It is disclosed in [`../docs/privacy-policy.md`](../docs/privacy-policy.md) §3 rather
than gated, because a gate here would decide nothing about a page the app does not control.
⚠️ **Lapses** if the service's address becomes configurable, if a third-party identity provider
is ever reached by the **app itself** rather than chosen on that page, or if an entitlement is
ever used to reach anything but Allodia's own services.

## The one rule everything else follows from

**An entitlement check is never on a path anyone waits for.** Not at boot, not before an action, not
while a screen paints. A client renders from what it has stored and refreshes in the background.

A cache is not sufficient on its own: a cache still misses, and a miss must not block either. If
the answer is not already in hand, the answer is the free app, and the refresh changes the screen
when it lands.

## Asking

`GET /api/v1/entitlement`, with an OAuth access token. The reply is derived and carries no
credential:

| Field | Meaning |
|---|---|
| `plan` | The plan's name, for display. `free` when nothing is active. |
| `active` | Whether a paid plan is live. |
| `capabilities` | A closed list of labels. Empty on the free plan. |
| `currentPeriodEnd` | RFC 3339, for display only. `null` when there is no plan. |
| `refreshAfterSeconds` | How long to wait before asking again. |

**A duration, not a deadline.** The server sends how long to wait rather than a timestamp to wait
until, because a device's clock can be wrong by any amount. Compared against a skewed clock, an
absolute deadline either hammers the service every launch or never comes due at all, and both
failures are invisible to the device experiencing them.

## Storing

A client stores the reply and when it took it. The answer is derived, not a credential, so it
belongs in the app's preferences beside the store, **not** in the platform keystore, which is for
secrets and costs a prompt on some platforms.

Two horizons, and they answer different questions:

| | Question | Value |
|---|---|---|
| `refreshAfterSeconds` | When should I *try* again? | The server's, 12 hours today |
| **Grace** | How long does a stored answer keep granting while I cannot reach the service? | **30 days** |

Grace is deliberately long. Someone on a three-week trip with poor connectivity must not lose a
capability they are paying for, and the cost of being wrong in that direction is close to nothing:
what a paid capability turns on is inert without Allodia's servers anyway, so an extra fortnight of
a stale *yes* grants the use of something that cannot work regardless. Being wrong in the other
direction takes a capability from someone who paid for it, during an outage that is Allodia's fault.

## The distinction that matters

**"You are not entitled" and "I could not ask" are different answers, and a client must never
collapse them.**

- The service replied `active: false`: a cancellation, a lapsed card, a plan that ended. It takes
  effect **immediately**. The stored answer is replaced by what the server said.
- The request did not arrive: an outage, a captive portal, a plane. The stored answer **stands**,
  until grace runs out.

Collapse them one way and a cancellation is honoured a month late. Collapse them the other and
every paying customer loses their capabilities the moment Allodia has a bad afternoon. Neither
failure produces an error anyone would report: both look like the app working.

## The client draws; the server decides

**Everything above is a rule about what to draw. None of it is access control, and no version of it
could be.**

The source is published, so anyone can delete the `active` check, return a paid entitlement from the
cache, or point the base URL at a service of their own. Moving the clock inside the core rather than
taking it from the host would change none of that: it would only make the rules untestable, and a
rule about thirty days that cannot be exercised on day thirty-one is a rule nobody exercises.
Obfuscation buys delay against someone who already has the source. Platform attestation is not
available on desktop and would break the promise that a from-source build keeps working, so it is
out by our own commitments.

That costs nothing, because **every paid capability needs Allodia to do something**:

| Capability | What only Allodia can do |
|---|---|
| `push` | The relay wakes the device. |
| `send_later` | A server holds the message and submits it at the hour. |
| `central_admin` | The administration surfaces are the service. |

A build that flips `active` to true draws a switch and then gets a refusal. What it cannot do is
take a capability, because the capability is not on the device.

**So the enforcement point is the server, on every request, resolved from the access token, never
from anything the client asserts.** `GET /api/v1/entitlement` already takes no input for that
reason: the caller is the subject, because an instance id in the request would let one account ask
about another's plan.

**The rule for anyone adding a paid capability:** the server checks entitlement before it does the
work. A client-side check that gates a call to that server is a convenience for the person using
the app, and nothing more. If a capability could be taken by editing the client, it does not belong
behind a plan.

## Degrading

Everything that is not a live entitlement is the **free app**, never an error screen, never a
prompt, never a modal:

- no stored answer;
- a stored answer past its grace;
- a stored answer that will not parse, or was written by a newer version;
- a capability label this version does not recognise: it is kept, and never granted, so a client
  older than a capability keeps working rather than guessing;
- any capability at all when `active` is false, even if the list is non-empty.

A person whose plan lapsed still has a complete mail and calendar client. Saying so is the whole
posture: there is no state in which this app stops working because of Allodia.

## Signing in

Authorization Code with PKCE against the account service, which is an OAuth 2.0 authorization
server ([RFC 8414](https://www.rfc-editor.org/rfc/rfc8414) discovery at
`/.well-known/oauth-authorization-server`). Access and refresh tokens are the OS keystore's, like
every other provider's: the token/redirect rules in
[`docs/provider-oauth.md`](../docs/provider-oauth.md) apply unchanged, and so does the shared
`AccountCredentialStore`.

**A grant is not simply good or bad, and a refusal on the wire is not the same as a bad
afternoon.** Three states, and the whole reason they are named:

| | What it means | Remedy | Drawn as |
|---|---|---|---|
| `Ok` | Nothing known to be wrong | — | nothing |
| `NeedsReauth` | Alive, but issued before a scope this build needs | sign in again | an offer |
| `SignedOut` | Refused outright: revoked here, or removed elsewhere | sign in again | a statement |

Read `allodia_grant_health()`, and **never an error's text**. A client that renders a failure's own
words is a client that ships whatever the OAuth layer happened to say:
`oauth endpoint error: invalid_scope — unable to issue scope mailcal:accounts:read` reached a
person that way. There is no `Unreachable` state, deliberately: it is the absence of one. A pass
that could not ask learned
nothing, so nothing is recorded, which is the same rule as a stored entitlement surviving an
outage.

**Adding a scope may never break an existing grant, and this is the mechanism.** A refresh names no
scope at all ([RFC 6749 §6](https://www.rfc-editor.org/rfc/rfc6749#section-6): omitted means the
scope originally granted), because sending the build's *current* list is refused with
`invalid_scope` by every grant issued before the list grew, which kills the account, not just the
new feature. What a grant actually carries is recorded beside it, from the token response's `scope`
(or, when the response names none, from what was requested; §5.1 makes it optional when they
match). A feature names the scope it needs (`allodia_license::Feature`), so adding one is one line
and the prompt follows on its own.

⚠️ **A grant whose scopes were never recorded is not a grant with no scopes.** Every entry stored
before this existed is the first; read as the second, it prompts every signed-in person on sight
and withholds every feature from them. Not-known concludes nothing, and the request stays the
authority.

⚠️ **A grant that is replaced or erased takes the access token minted from it.** The token is held
for the process, about an hour, so a sign-in that stored a new grant and left the old token cached
went on presenting it, and the service refused it, because the new authorisation had superseded
the grant it came from. What that looks like is somebody signing in successfully and being told, a
fraction of a second later, that they are signed out. Sign-out had the same hole, and left a
usable token in memory for the rest of the hour.

It is invisible while the only way in is from a signed-out state, where there is no stale token to
present; offering **sign in again** to somebody already signed in is what reaches it. Every path
that writes or clears the stored grant calls `forget_allodia_access_token`, and it is the kind of
rule no gate can hold: found by signing in on a real account (2026-08-28), with every headless
suite green either side of it.

**A debug launch keeps its sign-in in a namespace of its own.** Those launches connect a canned
harness account rather than the person's real ones, and for a while they were handed a store that
refused every write: right for a fixture with no grant behind it, wrong for a real grant a real
person just obtained, and it made the harness the one mode where signing in never stuck. Every
client now separates the two rather than refusing: Windows and Linux by a namespace on the
credential itself (`dev`, `dev-imap`, `dev-multi`), Apple and Android by taking this one entry out
of the store beside the canned account. Nothing a harness run writes can reach the developer's own
accounts, and nothing it reads can see them. Which entry is this one is asked of the core
(`is_allodia_account_config`) rather than matched in a client, so only one place knows the stored
shape.

The client registration is **injected at build time** (`MAILCAL_ALLODIA_CLIENT_ID`), so a build
given none has no Allodia sign-in at all, the same mechanism, and the same absent-is-supported
rule, as every other provider ([`BUILDING.md`](../BUILDING.md)). It is the only injected credential
that also needs a **cargo feature** (`allodia-license`), because the code it turns on is the code in
this directory and the open tree has to build without it; either half missing is the same supported
outcome. Every build front door derives that feature from the registration rather than taking a
switch of its own, including the Linux **Flatpak**, whose `package.sh` writes it onto the cargo
line in the manifest it builds from, because cargo reads features from nowhere else. `MAILCAL_ALLODIA_HOST` points a development build at a local service; it is a build-time
value with no runtime path to it, which is what the sovereignty carve-out above rests on.

The registration is **static**, not RFC 7591 self-registered: a first-party app has no reason to
mint one per install, and a static one can be revoked. What it registers is one redirect per
platform family, all of them public-client (no secret, PKCE required):

| Platform | Redirect |
|---|---|
| macOS, iOS, iPadOS, Android | `<application-id>://account-oauth`, Allodia's builds: `eu.allodia.mailcal://account-oauth` |
| Windows | the same, **and** `<application-id>.dev://account-oauth`. Windows registers a scheme per user, not per build, so an unpackaged dev build claiming the shipped one would leave the OS unable to tell them apart; it claims a `.dev` scheme instead, exactly as the Azure registration already lists both. A static registration cannot mint one per install, so the service has to list both or the dev loop is refused at the authorization endpoint |
| Linux, and any desktop developer loop | `http://127.0.0.1:<port>/`, the port is picked per flow, so the service has to match the **loopback host** and not an exact port (RFC 8252 §7.3) |

`account-oauth` differs from the `auth` and `jmap-oauth` labels already in use because Windows and
Android dispatch a callback on it: two flows sharing a label is a redirect delivered to the wrong
one, and it fails by never coming back rather than by erroring.

The request must carry three things the service will not infer, and omitting any of them fails the
same way:

| | Why |
|---|---|
| `offline_access` | Without it no refresh token is issued, and the sign-in becomes a session that expires with no way back. |
| `mailcal:entitlement:read` | What the entitlement endpoint requires. Read, and only read. |
| The RFC 8707 `resource` | The service mints a **verifiable JWT** for a named resource and an opaque token without one, and the API refuses anything not minted for itself. |

The last is the one that fails invisibly: without it a token *is* issued and *does* work against
the service's own `userinfo`, so it looks entirely valid right up to the `401`, which names
neither the audience nor the scope.

### What a client calls

Five calls, and a client needs no other knowledge of any of this:

| | |
|---|---|
| `allodia_sign_in_available()` | Whether to draw the button at all. `false` is the ordinary answer for a build from source: the surface is then **absent**, never present-and-broken. |
| `begin_allodia_sign_in(redirect_uri)` | Returns the page to open and an opaque `pending` handle. **Blocking**: it reads the service's metadata, so call it off the main thread. |
| `complete_allodia_sign_in(pending, callback_url)` | Exchanges the code, asks who signed in, and stores the grant. **Blocking.** Returns the account. |
| `allodia_account()` | Who is signed in, or nothing. Local and instant: it never asks the service. |
| `sign_out_of_allodia()` | Forgets the account and erases its stored grant. |

The `redirect_uri` stays the host's, because only the host knows it: a platform that claims a URI
scheme passes `<application-id>://account-oauth`, and Linux binds a loopback port per flow, as it
already does for Microsoft.

**Where the screen is.** Settings → **Allodia account**, the first category and one of its own: an
Allodia account is not a mail account, so it never appears among them
([`docs/settings.md`](../docs/settings.md)). It is also offered on the screen that adds the first
mail account ([`docs/onboarding.md`](../docs/onboarding.md)); those are the only two places.

Four states and no more: signed out offers **both** a sign-in and a create, because someone who has
no account and someone returning to one need different first steps and guessing wrong costs a
round trip; a sign-in in flight shows progress and no button to press again; a signed-in account
names the **address** (the name only when the service holds one, because the address is what
identifies the account) and offers **Manage account** and **Sign out**; a failure says what the
service said.

**Create is a `prompt` on the same request, never a second URL.** The authorization endpoint is
discovered, so a literal sign-up address would be a second source of truth and would ignore
`MAILCAL_ALLODIA_HOST`. `prompt=create` is OpenID Connect Prompt Create 1.0 and the service
advertises it in `prompt_values_supported`; a build sends it **only** when that list carries it,
which is the same rule `AuthStyle::Discovered` already applies to everything else.

**Manage account opens the service's own page the same way sign-in opens the authorization
request**, using the platform's **in-app browser tab** where there is one (RFC 8252 Appendix B):
`ASWebAuthenticationSession` on Apple, a Custom Tab on Android, the system browser on Windows and
Linux, which have no such thing. One mechanism, not two, and the reason is not tidiness: an in-app
browser tab **is** the system browser, so the session cookie the sign-in just set is already there
and the page opens signed in. An embedded user-agent (`WKWebView`, `WebView`, `WebView2`) has its
own cookie jar, would show a login page instead, and is refused outright by Google: RFC 8252 §8.12
forbids it for the authorization request and nothing here needs it either.

Two consequences worth stating rather than discovering. On **Windows and Linux** the page is a
window switch instead of a sheet over the app, because the platform offers no in-app browser tab;
that is a presentation gap, not a functional one. And a browser session can expire while the
refresh token is still good, months later, in which case the page opens on its own sign-in, which
works, because it is a real browser, and is the reason no client may treat "manage account" as an
action that can fail.

**Signing out erases first and asks the server second.** The local grant is gone before the network
is touched, so a sign-out cannot fail halfway and leave someone signed in; the RP-initiated logout
URL (`end_session_endpoint`, when advertised) is handed back for the client to open afterwards.

What that URL is for, precisely, because the obvious assumption is wrong: it ends the **browser**
session, so the next sign-in on that device asks who you are instead of completing silently against
a session someone thought they had left. It does **not** end ours. The endpoint revokes the tokens
bound to the session it closes, but a refresh token carrying `offline_access` is deliberately
preserved: that is what `offline_access` means, and this build requests it, because without it a
sign-in becomes a session that expires with no way back.

⚠️ **Known gap:** so the only thing that would end this build's grant server-side is the
`revocation_endpoint` (RFC 7009), and the service's metadata advertises no `none` auth method for
it, which a public client is all it can offer. Until that changes a signed-out grant stays live at
the service until it expires. The local erase means nothing on the device can use it; nothing here
is left holding a credential. Windows and
Linux also offer **Cancel** while the browser is open, because a custom-scheme activation and a
loopback listener get no signal when the user simply closes the browser. Apple's
`ASWebAuthenticationSession` and Android's Custom Tab report the dismissal themselves.

**The grant is stored through the same `AccountCredentialStore` a mail account uses**, under a
reserved id, so no client writes a credential of its own and a rotation has somewhere to land. The
entry comes back at the next launch in the same list as the mail accounts, and the core takes it out
before anything reads it as a mailbox, including a build with **no** Allodia sign-in, which
recognises the entry so it can leave it alone rather than report an intact grant as a corrupt
account at every launch.

**All three are discovered, not assumed.** The only thing a build knows about the service is its
address. Asking the API unauthenticated returns a `401` whose `WWW-Authenticate` names its RFC 9728
metadata; that metadata names the API's canonical URI and which server protects it; and that
server's RFC 8414 metadata names the endpoints. Every hop is the standards' own, and the same chain
a discovered JMAP server goes through, so the audience a token carries cannot drift from the one
the service verifies, because neither is written down here.

## Per-platform status

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Sign in to an Allodia account | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Create an account from inside the app (`prompt=create`) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Manage or delete the account, in the service's own page | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Read an entitlement, with the grace and degrade rules above | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Tell a narrowed grant from a revoked one, and prompt for the right one | ✅ | 🚧 | 🚧 | ✅ | 🚧 | 🚧 |
| Signing in again widens the grant, end to end | ✅ | ⬜ | ⬜ | ✅ | ⬜ | ⬜ |
| Draw a paid capability | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

The core's first two rows are **verified against the production service**, not only against a local
one: sign-in, a JWT minted for the discovered audience, a refresh token, and the entitlement read
back. The sign-in row is verified **through the FFI a client calls**, and through a second launch
built from what the first one stored, which is where the entry has to come back as an account and
not as an account error (2026-08-26).

All five clients carry the screen and the flow, and every one of them is driven against the
production service. Linux (2026-08-27) through the card on a real Secret Service keyring, including
the relaunch that restores the account and a sign-out that erases it, on the GNOME runtime the
Flatpak links rather than the distribution's GTK. Its create, manage and delete buttons rest on the
widget suite rather than on a further live run: the suite renders the row and asserts each button
reaches its own input, and no account was created against the production service to see the pages
they open. The rest (2026-08-26): macOS through the card, on a real Keychain, including a relaunch
that restores the account and a sign-out that erases it; Windows the same on the Credential Manager,
in both the packaged and unpackaged shapes; Android on a physical device, where the redirect returns
through a Custom Tab to the activity and is routed to this flow by its **host** rather than its
scheme: the one thing no other platform exercises, because no other platform shares a scheme
between two sign-ins; and iOS on a simulator, through its own hub chrome, including the relaunch
that has to bring the account back rather than an account error.

Legend as [`README.md`](../README.md): ✅ shipped · 🚧 in progress · ⬜ planned.

## Known gaps


- **The grant-health prompt is built on all five clients and run on one.** Windows was built,
  driven and verified against the production service on a grant that really was narrower than the
  build wanted: the whole chain, from a refresh that now succeeds where it used to be refused, to
  the recorded scope set, to the prompt. Apple, Android and Linux draw the same three states from
  the same typed answer and are reviewed rather than run; the matrix says so.
- **Signing in again is verified on Windows only**, against the production service (2026-08-28):
  a grant that really was too narrow, the prompt, the browser round trip, and a pass that then
  published six accounts and adopted the one the service already held. It needs a person at a
  browser with real credentials, so it is asserted nowhere and the other four clients have not
  been driven through it.
- **Nothing writes an entitlement yet.** The account service has no billing, so every caller
  correctly receives the free answer. Until that changes, none of the rules above can be observed
  end to end against a real plan.
- **No client draws a paid capability**, so the grace and degrade rules are unit-tested in the core
  and unproven in a UI.
- **Nothing refreshes the access token yet.** The grant is stored with its refresh token, and
  `SignIn::refresh` exists, but no caller reaches for it, so a signed-in account is currently a
  stored identity rather than a usable credential. Whatever asks for the first entitlement is what
  will need it.
- **Signing out is local.** It erases this install's copy of the grant, which is what removing a mail
  account does too; the grant itself stays alive at the service until it expires or the person
  revokes it there. The service advertises an RFC 7009 revocation endpoint and nothing calls it.
