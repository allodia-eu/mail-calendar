# Signing in with Nextcloud

The cross-platform contract for connecting a calendar and contacts account at a Nextcloud server
through Nextcloud's own sign-in for native apps, [Login Flow v2][flow], instead of a username and
a hand-made app password.

Nextcloud offers no OAuth an installed app could use: its `oauth2` app registers confidential
clients only, without PKCE or dynamic registration, so [`mail-oauth.md`](mail-oauth.md) rule 13
finds nothing there. Login Flow v2 is the route Nextcloud documents for its own clients and for
DAVx5, and the only one that works behind an organisation's single sign-on, where there is no
password to type and the login name is often an opaque id.

The shared core (`mailcal-nextcloud` for the protocol, `mailcal-account` for the stored account,
the bindings for the surfaces) owns every decision; a client owns what it draws and the browser.

[flow]: https://docs.nextcloud.com/server/latest/developer_manual/client_apis/LoginFlow/index.html#login-flow-v2

## The rules

1. **The server says it is Nextcloud, or nothing is offered.** The question is the one Nextcloud's
   own clients ask: `GET {root}/status.php`, answered by an installed server that is not in
   maintenance and names `"productname": "Nextcloud"`. The Theming settings cannot change that
   name. They can change the instance name a `WWW-Authenticate` realm carries, so the realm is
   never read, and neither is a cookie. ownCloud and ownCloud Infinite Scale answer the same
   document with their own names and get the password form.

   **It is asked only of a server whose DAV endpoint is a PHP script**, a path segment ending in
   `.php`, as Nextcloud's `/remote.php/dav/` and SabreDAV's `/dav.php/` are. A server that runs
   no PHP can read a request for `status.php` as a probe for exploitable scripts and ban the
   address that sent it: Stalwart's default port-scanning rule does, with no expiry, and so do
   fail2ban-style rules. The endpoint is the address as typed or stored, or where detection's
   `.well-known` hop landed. An address that names no endpoint, a bare host or its `.well-known`,
   costs one unauthenticated `PROPFIND` of the server's own `.well-known/caldav`, the request a
   connect makes first anyway; Nextcloud answers it with `/remote.php/dav/`. Nothing else is read
   for the sign, no DAV body among it.

   The root is the URL cut where Nextcloud's own paths start (`/remote.php/`, `/.well-known/`,
   `/index.php`, `/status.php`, `/ocs/`), so a DAV endpoint and a `.well-known` hop name the same
   server, and an installation in a directory keeps its directory. A bare host names the server
   its `.well-known` hop lands on, which may be another host or a directory, and the sign-in starts
   there, not at the host as typed or stored. Each request is bounded at three seconds and the
   question is never an error: a server that does not answer is offered the password, which works
   everywhere. Neither is a server whose certificate needs an
   exception ([`certificate-exceptions.md`](certificate-exceptions.md)): the browser half would
   meet the same certificate.

2. **Four surfaces offer it, and nobody picks a provider.** Each is a surface that already exists;
   none is a "Nextcloud" choice in a menu ([`onboarding.md`](onboarding.md)).

   | Surface | When it is asked | What is drawn |
   |---|---|---|
   | The found card for an address with a calendar and contacts but no mail, and the link step's card for servers found beside a JMAP server | During detection (`DetectedSetup.nextcloud_sign_in`, handed back to `dav_setup_beside`) | **Sign in with Nextcloud** first, under the line that explains it, and the password behind **Use a password instead** ([`mail-oauth.md`](mail-oauth.md) rule 2) |
   | The manual calendar and contacts form | Off the UI thread once a CalDAV or CardDAV URL is entered (`offers_nextcloud_sign_in`) | The line and the button above the fields; fields already drawn stay ([`mail-oauth.md`](mail-oauth.md) rule 8) |
   | A mailbox's separate-server step, and its twin on the account's page | By the core, as the step or the page is drawn (`AccountSeparateServer::sign_in`) | Above the password field, where rule 13's sign-in sits |
   | A calendar and contacts account's page, and its "sign in again" prompt | As the page opens, by the core: its Sign-in section (`AccountPage::sign_in`) offers the route; `AccountEntry.nextcloud` for an account a Nextcloud sign-in made | **Sign in with Nextcloud** again, or **instead** of a typed password |

   An IMAP route's found card is unchanged: its calendar reuses the mailbox's password, and a
   server that refuses it leads to the separate-server step, which asks.

3. **The app password is named after this app and this device.** The flow starts with
   `POST {root}/index.php/login/v2` carrying `"{app name} ({platform})"` as its User-Agent,
   because the server names the app password after it: the person reads it on the page that asks
   them to grant access, and again in Nextcloud's list of devices. The platform is the one the
   host reported at construction. Every other request carries the product token as always.
   An "&" in the app name is left out of it, because Nextcloud's grant page escapes the name
   twice and would show it as "&amp;"; an administrator's `core.login_flow_v2.allowed_user_agents`
   pattern therefore matches the name without it.

4. **Every URL the server returns is on the server's own host.** The login page and the poll
   endpoint must name the root's host and port. An `http` URL there is answered as the `https`
   one, because a TLS-terminating proxy that does not tell Nextcloud the scheme makes it write
   `http://` into every absolute URL it returns; another host, or another port, refuses the
   sign-in. The `server` the grant names is never used to address anything: the app password goes
   only to the root the flow ran at, over TLS.

5. **The browser half is the platform's, and the wait is a poll.** There is no redirect back to
   the app. The client opens the login page in the platform's browser and polls
   `poll_nextcloud_sign_in` every `nextcloud_poll_interval_ms()`, and once more when it returns to
   the foreground. Twenty minutes after it started the server lets the flow go, and the poll
   answers `Expired` without asking. The way out of the wait is the one
   [`onboarding.md`](onboarding.md) requires of every browser sign-in.

6. **The grant comes once, so it is kept until it is an account.** The server answers the grant to
   one poll and forgets it. The client holds the grant in memory; a completion that fails keeps it,
   so Connect can try again without a second trip to the browser; a grant that becomes no account
   is withdrawn (`abandon_nextcloud_sign_in`).

7. **The account is a calendar and contacts account, named by its profile address.** Both uses
   connect at `{root}/remote.php/dav/`, an account switched over from a typed password included,
   with the login name the server returned as their username,
   verbatim, because the server accepts the app password under that name and no other spelling.
   The account stores where the password came from in `[nextcloud]` (`server`, the Nextcloud
   `user` id and the profile `address`, from `cloud/user`), and is read by that address, falling
   back to the login name ([`accounts.md`](accounts.md) rule 8). Its id is the login and host, as
   every calendar and contacts account's is, and it survives a sign-in under another spelling of
   the same name. A second sign-in as the same Nextcloud user is the account already set up,
   whichever name was typed. Settings edits neither its servers nor its login, and it does not
   switch to a typed password: it signs in through its provider.

8. **There is nothing to refresh; a refusal is a sign-in again, or a wipe.** The app password has
   no expiry. The server stops accepting it when the person revokes it or the account, when it
   goes unused for a year, when a stored login password stops verifying, or when the person
   chooses **Wipe device** for it. Each is a DAV `401`. On the first refusal, the core asks
   `wipe/check` once (the server allows ten in five minutes): a wipe removes the account and what
   the device holds of it, as a removal does, tells the server with `wipe/success`, and draws
   nothing new; any other answer leaves the account at "sign in again", whose action is this
   sign-in. The account is never redialled with the refused password.

9. **The password leaves with the account.** Removing the account withdraws its app password
   (`DELETE /ocs/v2.php/core/apppassword`), and signing in again withdraws the one it replaced,
   once the new one is stored. Both are best effort and never waited for. A wiped password is not
   withdrawn on top.

## Sovereignty and privacy

Connecting an account is not jurisdiction-gated ([`provider-oauth.md`](provider-oauth.md)
"Sovereignty scope"), and this is connecting an account. Every request goes to the person's own
server; nothing reaches Allodia. The log records the flow's steps and outcome classes, never a
URL, token, login name or address ([`logging.md`](logging.md)).

## Per-platform matrix

Legend: ✅ implemented · 🚧 code-complete, runtime unverified · ⬜ planned.

| Gate | Shared core | macOS / iOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| The server is asked, never the realm | ✅ | n/a | n/a | n/a | n/a |
| Found card: sign-in first, password behind | ✅ | 🚧 | 🚧 | ✅ | ✅ |
| The link step's card for servers found beside a JMAP server | ✅ | 🚧 | 🚧 | 🚧 | 🚧 |
| Manual form asks once a URL is entered | ✅ | ✅ | ✅ | ✅ | ✅ |
| Separate-server step offers it above the password | ✅ | 🚧 | ✅ | 🚧 | 🚧 |
| Account page and sign-in-again prompt | ✅ | ✅ | ✅ | ✅ | ✅ |
| Browser opened, polled, and polled again on return | n/a | ✅ | ✅ | ✅ | ✅ |
| A held grant retried, or withdrawn when abandoned | ✅ | 🚧 | ✅ | 🚧 | 🚧 |
| Remote wipe answered | ✅ | n/a | n/a | n/a | n/a |
| Password withdrawn on removal and on a new sign-in | ✅ | n/a | n/a | n/a | n/a |

## Testing

- **Protocol** (`mailcal-nextcloud`, loopback servers): the root of every URL shape, the PHP sign
  and the `.well-known` hop with no `.php` request to a server that shows none, the status
  document against Nextcloud, ownCloud, Infinite Scale, maintenance and a refusal, a flow naming
  another host refused and an `http` one upgraded, the poll's waiting, grant, outage and expiry,
  the User-Agent sent, and the OCS and wipe calls with the credential they present.
- **Detection** (`mailcal-autodetect`): a calendar-only domain on Nextcloud flagged, an
  installation in a directory asked there, a server whose endpoints are no PHP script never asked,
  and any doubt (an untrusted hop, a redirect elsewhere, two servers) not.
- **Account** (`mailcal-account`): the stored shape round-trips, an account no Nextcloud sign-in
  made has no table, the address names the account, and a sign-in again replaces login and
  password together.
- **Bindings**, against a stand-in server (`nextcloud_mock.rs`): the flow through the FFI, a
  second sign-in as the same user refused, another person's grant refused and withdrawn, removal
  withdrawing, a refusal at launch answered by a wipe, by no wipe, and not asked about for a
  typed password, and no `status.php` request from the manual form or an account's page to a
  server that shows no PHP.
- **Live**, gated on `NEXTCLOUD_HTTP_ADDR`, against the engine's Nextcloud fixture:
  `mailcal-bindings/tests/live_nextcloud_sign_in.rs` signs in through the server's own pages,
  connects CalDAV and CardDAV with the app password, signs in again and removes the account,
  asserting each withdrawal at the server. It is not in CI: this repository's harness is
  Stalwart, and the fixture is the engine's.
- **Clients**: each client's PR adds the assertions its platform can make about the four
  surfaces. Linux asserts the poll and its wake (`nextcloud_flight_tests.rs`), the found card,
  the manual pane and the separate-server step drawn, the account page's section, and the banner's
  route; against the engine's fixture it was driven through the manual pane, signing in again and
  removal. The found card and the separate-server step need a domain and a mailbox the fixture
  cannot stand in for, so they are runtime unverified there. Android asserts the same in its JVM
  suite (`NextcloudFlightTest`, `NextcloudSetupTest`, `NextcloudSetupScreenTest`,
  `NextcloudAccountSignInTest`) and was driven on an emulator through the same three flows. Apple
  asserts the flight and each surface's state in MailcalKit and the found card's drawing in
  `NextcloudSignInTests`, against the showcase's `nextcloud.example`; the macOS app was driven
  through the manual form and signing in again, and the iOS app on a simulator through the manual
  form, the account's page, signing in again, removal and the poll on return: a grant made while the
  app was in the background was taken 52 ms after it came back. Windows asserts the flight and each surface's state
  in `Mailcal.Tests`, and its UI suites (`NextcloudSignInTests`, `NextcloudAccountPageTests`,
  `NextcloudSeparateServerTests`) drive the manual form, the wait and its Cancel, a kept grant's
  retry and withdrawal, the account page, and a mailbox's separate-server step against the
  fixture. Linux and Android were also driven against a live Nextcloud server through an address
  whose domain has no mail server and redirects its `.well-known` to Nextcloud: the found card
  led with the sign-in, the grant arrived, a sign-in waiting in the browser asked again the moment
  the app was in front (`a window is active again; asking now` on Linux, `the app is in front
  again; asking now` on Android), and removal withdrew the password. The link step's card needs a
  domain whose detection finds a JMAP server with a calendar server beside it, and stays
  unit-tested on every client.
