# Signing in to a mail account: what setup asks for, and why

The cross-platform contract for connecting an **IMAP/SMTP** account that authenticates with
OAuth rather than a password. It decides what the setup screen asks each person for, where an
authorization server may be learned from, and how an account behaves once its token starts
expiring.

[`provider-oauth.md`](provider-oauth.md) is the contract for Microsoft and Google, whose
endpoints and client registration this app holds at build time. This one is for every other
provider: a server we have never met, discovered from the standards. A JMAP account takes the
same path and differs in one step, noted below.

The shared core (`mailcal-account` + `mailcal-oauth` + the bindings) owns the whole decision
and the whole state machine; a client owns only what it draws and the **browser half**
(opening the authorization URL, capturing the redirect), which is inherently platform-specific.

## Which standards this follows

The profile is [`draft-ietf-mailmaint-oauth-public`][profile], the IETF working-group draft for
open public clients on IMAP, SMTP, JMAP, CalDAV and CardDAV: Authorization Code with PKCE
(RFC 7636), issuer identification (RFC 9207), authorization-server metadata (RFC 8414) and
dynamic client registration (RFC 7591), with the token presented over SASL `OAUTHBEARER`
(RFC 7628). Where the draft and a deployed server disagree, the server wins and the difference
is recorded here.

[profile]: https://datatracker.ietf.org/doc/html/draft-ietf-mailmaint-oauth-public

## The rules

1. **The server is asked first.** Before any OAuth discovery, the setup path opens one TLS
   connection to the mail server and reads its pre-authentication `CAPABILITY`
   (`provider_imap::probe_imap_auth`). That line is what the account will actually be judged
   by: a domain may publish an authorization server for its web sessions and take only a
   password on IMAP, and a sign-in offered on that evidence mints a token the mailbox refuses.
   **Nothing is presented and no failed sign-in is recorded**: the probe stops at the
   capability list, closes with `LOGOUT`, and never sends a credential.

2. **Three answers, because they are three screens.**

   | The server says | Setup offers |
   |---|---|
   | An OAuth mechanism, and an authorization server was found this install can register with | **Sign in**, primary. The password field, if a password also works, behind a secondary control. |
   | An OAuth mechanism, but no usable authorization server | A line saying the provider admits only applications it registered in advance, plus the password field. |
   | No OAuth mechanism, or nothing (unreachable, timed out) | The password field, exactly as before. |

   The middle row is the reason this is an enum and not a flag. A provider whose sign-in
   exists but is closed to this application is not the same as one that offers none, and
   showing one bare password form for both leaves a person wondering why the button their
   colleague has is missing.

3. **A password is offered only where one works.** `AUTH=PLAIN`/`AUTH=LOGIN`, or the absence
   of `LOGINDISABLED`. A server that has switched password authentication off (Microsoft 365's
   shape) gets no password field: it would be a dead end nobody finds until they have typed
   one.

4. **An issuer comes from the provider describing itself, never from a third party.** Two
   channels, in order: the `<oAuth2><issuer>` of the provider's **own** autoconfig, fetched
   over HTTPS from its own domain; failing that, an RFC 8414 well-known probe of the **email
   domain** and the mail server's **registrable domain**. The ISPDB's `<oAuth2>` block is
   dropped, and so is any issuer read off an **untrusted** (non-HTTPS) hop: those settings are
   approved by the user before a credential is *sent*
   ([`account-autodetect.md`](account-autodetect.md) rule 3), which is not the same question
   as where one is *typed*. Only the `issuer` is ever taken from a document; the endpoints
   beside it are ignored, and the real ones come from that issuer's own metadata.

   **JMAP differs here and only here.** A JMAP server is an HTTP resource, so an
   unauthenticated request answers `401` and names its authorization server (RFC 9728). IMAP
   has no such surface to be challenged on, which is why the probe above exists.

5. **A client id is registered, never invented.** Where the metadata advertises a
   `registration_endpoint`, this install registers itself (RFC 7591) and the issued id is
   persisted with the account, so a launch never re-registers and a server that later withdraws
   open registration cannot break an account that already has one. Repeated attempts inside one
   session reuse the id already minted: without that, three taps of a flaky sign-in leave three
   orphaned client registrations on the user's account.

6. **A provider that will not do open registration needs an entry this build carries**, and
   absent is a supported build. `mailcal_oauth::static_mail_providers` holds the endpoints and
   host patterns; the client id is injected like Google's and Microsoft's, and a route whose id
   is absent is simply not offered. The table ships **empty**: rule 2's middle row is what a
   person meets until an entry exists, and it says something true rather than nothing.

   ⚠️ A provider that issues only a **confidential** client is one to raise with them, not to
   work around: an installed binary cannot keep a secret, and embedding one to look compliant
   is worse than the honest message rule 2 already shows.

7. **Every failure is the password field.** A refused dial, a metadata document that will not
   parse, a registration refused, a server that never answers: all of them mean the question
   went unanswered, and the answer that works everywhere is a password. A setup screen never
   shows an error for any of this. Which step gave up is in the diagnostic log, where it is
   useful.

8. **A card shows nothing to act on until the server answers**, and a **deadline races the
   probe** (10 s). Whichever lands first decides, and only the first answer for a given server
   counts: a late one would rebuild a card the person is already typing into. A credential
   field that appears and is then taken away reads as the app changing its mind.

9. **The credential is resolved per dial, and one authentication failure is worth one
   re-dial.** An access token expires within the hour while an IMAP session does not, so a
   config built once would authenticate for exactly as long as its first token. The engine
   deliberately refreshes nothing itself, so the host mints a token for every dial, and a
   `FailureClass::Authentication` on an **OAuth** account triggers exactly one re-dial with a
   fresh one. On a **password** account it triggers none: the same secret would go back to the
   same server, at a provider that may be counting attempts toward a lockout.

10. **An OAuth account stores no password anywhere.** Its `[imap]` section carries a grant and
    no secret, its calendar and address book present the same bearer token rather than a reused
    password, and "repair this account" is a re-authorisation rather than a typed secret. The
    `calendars` and `contacts` scopes are requested at sign-in precisely so the second half
    works.

11. **`iss` is checked where a server says it sends one** (RFC 9207). That is the only thing
    that catches a mix-up relay once `state` has matched, and it is checked strictly: a
    response missing the parameter from a server that advertised it is refused, because
    deleting a query parameter must not be all it takes to switch the check off.

## What is deliberately not done

- **No RFC 7628 challenge probe.** §3.2.2 lets a server describe its OAuth configuration,
  `openid-configuration` URL included, in the challenge it returns when it *rejects* a token,
  so a client could discover the authorization server by presenting a deliberately invalid one.
  That would leave a failed authentication on the person's account before they had signed in
  once, on the screen they are most likely to abandon, and the field is optional anyway:
  Stalwart answers a bad token with a bare `NO [AUTHENTICATIONFAILED]` and no challenge at all.

- **No RFC 8707 `resource` for an IMAP account.** The profile's example is a JMAP session URL
  and it defines no URI form for an IMAP endpoint. A server that scopes tokens by resource
  applies its default without one; inventing `imap://…` would risk `invalid_target` on the
  exchange and on every refresh after it. A JMAP account still sends the one its RFC 9728
  metadata published.

- **No mechanism knob.** Which SASL mechanism carries the token is negotiated by the engine
  from what the server advertises, and a caller able to pin one would be encoding which
  provider it is talking to.

## Per-platform matrix

Legend: ✅ implemented · 🚧 code-complete, runtime unverified · ⬜ planned.

| Gate | Shared core | macOS / iOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Server asked before a credential field is drawn | ✅ | 🚧 | 🚧 | ✅ | ✅ |
| Sign-in primary, password behind a secondary control | ✅ | 🚧 | 🚧 | ✅ | ✅ |
| "Only pre-registered apps" explained rather than shown as a bare form | ✅ | 🚧 | 🚧 | ✅ | ✅ |
| No password field where the server refuses passwords | ✅ | 🚧 | 🚧 | ✅ | ✅ |
| Nothing to act on until the answer, with a deadline racing it | ✅ | 🚧 | 🚧 no deadline | ✅ under the spinner | ✅ |
| Browser sign-in + redirect capture | n/a | 🚧 `ASWebAuthenticationSession` | 🚧 protocol activation | ✅ Custom Tab | ✅ loopback |
| Grant stored with no password beside it | ✅ | n/a | n/a | n/a | ✅ |
| One re-dial on an expired token | ✅ | n/a | n/a | n/a | n/a |

## Known gaps

- **Windows has no deadline racing the pre-flight.** Its gate leaves the credential fields
  absent until the server answers, and nothing yet brings the password field back if the server
  never does. The core's own call is bounded by its TLS and HTTP timeouts, so the wait ends, but
  it is longer than the ten seconds the other clients cap it at.
- **Windows' surface is compiled and has never been run.** `ImapSignInGate` and the routing are
  covered by `Mailcal.Tests`, plain `net10.0` and runnable on any host; the WinUI half (the XAML,
  the view partial, the model's browser flow) compiles only on a Windows host, which CI is. What
  no run has reached is the screen: `uitests/run-ui-tests.ps1` asserts against the running app,
  and the three answers have not been through it.
- **Android has no "still asking" state, deliberately.** It resolves the answer under the
  "Looking…" spinner that detection already shows, before the card exists, so the card renders
  in its final shape rather than settling into one. That is a stricter reading of rule 8 than a
  `checking` state, not a weaker one.
- **Apple's surface is compiled and has never been run.** CI builds it on a macOS runner, the
  package suite plus `xcodebuild` for macOS and iOS Simulator, which is the isolated-batch build
  the trap in [`AGENTS.md`](../AGENTS.md) warns `swift build` alone would not give. There is no
  Apple UI-test target, so no assertion reaches the screen: what the three answers look like, and
  that the sign-in sheet returns to the right card, are verified by hand on a Mac.
- **The static-provider table is empty**, so rule 2's middle row is what every closed-
  registration provider gets. Yahoo is the one people meet; its mail scopes are granted only
  after a developer-access review, and no entry is written until we know what that grant
  actually allows.
- **The full sign-in is not automatable end to end.** The authorisation step needs a browser
  and a person, so what CI proves is the probe, the decision, the handle that survives the
  browser hop, and the account the completion writes. The step between is driven by hand.
- **The local harness cannot serve a reachable issuer.** Stalwart derives its issuer from its
  configured hostname (`https://mail.test.local`), and the harness maps only a loopback HTTP
  port, so the metadata URL that issuer implies does not resolve from the host. The harness
  does enable open registration, so the registration endpoint itself is exercisable; the
  discovery step is proven against the standards' own shapes offline instead.
- **A rotated refresh token needs the host's credential store.** Every client already has one
  (it is how Microsoft, Google and JMAP accounts survive a rotation), so this is a note rather
  than a gap: an OAuth IMAP account added on a host without one would die at its first
  rotation.

## Testing

- **Engine** (`provider-imap`): the probe's classification against **observed** capability
  lines, including the two shapes that are easy to read backwards (`LOGINDISABLED` beside
  `AUTH=PLAIN`, and `LOGINDISABLED` alone), plus a harness-gated live suite proving a STARTTLS
  probe reads the post-upgrade capability on both protocols and that repeated probes leak no
  session.
- **Core**: which port a probe dials for each connection security, which issuers it is willing
  to ask and in which order, the host matching for a static entry (label-boundary, so
  `notyahoo.example` never matches `yahoo.example`), and the stored account: an OAuth account
  round-trips with a grant and no secret on any endpoint, `with_password` is a no-op on one,
  and a password account's stored TOML is byte-for-byte what it was.
- **Reconnect**: one re-dial on an authentication failure for an OAuth account, none for a
  password account, and a submission still never blind-retried whichever refused it.
- **Bindings**: everything that crosses the browser hop, because each field is checked later
  and somewhere else: a dropped `redirect_uri` is rejected on the next refresh, a dropped
  `issuer` silently stops checking RFC 9207, a dropped STARTTLS flag connects the account to a
  port the provider may not have open.
- **Live, harness-gated** (`mailcal-account/tests/live_imap_auth.rs`): the decision against a real
  server, which is the one thing the offline suite cannot show. It needs the harness CA and the
  hostname the certificate actually carries; both are in the file's header, because getting
  either wrong fail-softs to "ask for a password", which is indistinguishable from the code being
  broken.
- **Clients**: Linux drives the three screens through a real widget tree under Xvfb, asserting
  the half a screenshot cannot check: that no credential field is on screen while the server is
  being asked, that the offer and the password route appear together when both work, and that
  the closed-registration line appears with no sign-in button beside it.
