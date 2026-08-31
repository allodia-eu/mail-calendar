# Auto-detect account settings: email-first setup contract

How the product turns a user's **email address alone** into a routed, prefilled account
setup (a JMAP / IMAP / Microsoft / Google path) instead of asking them to type server
hostnames.
This is the cross-platform contract so every client detects the same way and gates the
same security decisions. It reimplements the approach of Thunderbird for Android's
autodiscovery in the shared Rust core (`mailcal-autodetect`), so every client gets it
from one implementation.

The shared core owns the whole detection path; a client owns only the **email-first UI**
(the prompt, the result card, the untrusted-approval control, the manual escape) and its
**native DNS resolver** (the one piece the core deliberately does not ship). Detection
feeds the *existing* connect path (`account_config_toml` / `jmap_account_config_toml` →
`add_account`). It prefills the same forms, it does not replace them.

## The rules

1. **Strategies raced in priority order.** For `user@domain` the core runs, at once:
   (0) a **JMAP** well-known probe: apex `.well-known/jmap`, then the `_jmap._tcp` SRV
   target; (1) Mozilla **autoconfig** (`autoconfig.{domain}` + `/.well-known/autoconfig/…`,
   HTTPS then HTTP); (2) the **ISPDB** (`autoconfig.thunderbird.net`); (3) an **IMAP/SMTP
   SRV** lookup (`_imaps._tcp` + `_submissions._tcp`, RFC 6186/8314); and (4) an **MX
   fallback** (host-resolved). The lowest-priority-number success wins; a lower-priority
   success waits for every higher-priority strategy to finish, and the losers are cancelled.
   This product is JMAP-first, so JMAP is priority 0.

2. **The email address never appears in a URL.** Only the domain is disclosed: to the
   provider's own endpoints, to the ISPDB, and to the DNS resolver (as the
   `_service._tcp.{domain}` SRV owner name). A JMAP-SRV *target* host is disclosed only to
   that host, when its `.well-known/jmap` is probed. (Thunderbird's optional
   `?emailaddress=` parameter is deliberately not implemented.)

3. **A result is trusted when it was obtained over TLS.** `is_trusted` is true when every fetch
   hop was HTTPS. DNS-based discovery (an SRV endpoint, or an MX-derived provider autoconfig)
   is trusted on that validated TLS alone: **DNSSEC is not required.** The servers are
   TLS-validated on every connect, the resolved host is pinned into the stored config so DNS
   can't move it after setup, and this is how mail autodiscovery works across the ecosystem
   (rule 9). The only **untrusted** case is a **non-HTTPS hop** (the `http://` autoconfig
   variants some small providers still publish), which **must be shown to the user and require
   explicit approval** ("I trust these settings") before any credential is sent. This gate is
   identical on every client.

4. **Autodetect never yields a plaintext config.** The autoconfig parser rejects any
   `socketType` that is not `SSL`/`STARTTLS`, and the recommendation layer only ever routes
   to a server the engine can actually connect over a secured link: implicit TLS (993/465)
   **or STARTTLS** (143/587), the latter since the engine gained STARTTLS support. The
   detected connection security rides on the recommendation (`imap_security`/`smtp_security`)
   and the client passes it straight back on connect, so the engine dials exactly what
   detection found. A **STARTTLS** connect still **fails safe**: the engine requires the
   server to advertise `STARTTLS` and never downgrades to cleartext auth. A plaintext-only
   provider is unrepresentable (a parse error) and can never become an account.

5. **Detection is bounded and fails silently.** Each request has a short timeout (4 s), the
   whole run a hard deadline (10 s), redirects a 5-hop cap, and bodies a 256 KB cap. A
   TLS/cert/transport failure (a self-signed cert, an unreachable host) is a **silent skip**,
   logged at debug, never an error the user sees, never a long hang. "Nothing found" always
   offers manual setup.

6. **DNS is the host's job.** The core ships no DNS resolver: the MX strategy calls a
   host-provided `MxResolver` (a UniFFI callback port) so each platform's native API answers,
   honouring the device's real DNS configuration (private DNS, a VPN). A client that passes no
   resolver simply skips the MX fallback.

7. **OAuth endpoints are never taken from a fetched config.** The ISPDB's `<oAuth2>` block is
   ignored; a **Microsoft- or Google-family** provider routes to the app's own browser OAuth
   for that provider, and a provider that offers only OAuth we don't integrate routes to manual
   (never a broken sign-in). **Google is a native-API integration** (Gmail + Google Calendar,
   not IMAP/CalDAV), so a Google address short-circuits detection entirely; see the Routing
   section and rule 10.

   The **one** OAuth configuration ever taken from the network is a JMAP server's own published
   metadata, and it is not taken from a *fetched autoconfig* at all: it comes from the server the
   user is connecting to, over the RFC 9728 → RFC 8414 chain, after detection has already routed
   to JMAP. It is bound by the same discipline as everything else here: every hop HTTPS, only
   the domain disclosed, and a failure is silent and falls back rather than dead-ending, plus
   two of its own: the metadata's `issuer` must match the issuer asked about (RFC 8414 §3.3),
   and the server must advertise S256 PKCE. See [`jmap.md`](jmap.md) rule 3. Detection itself
   never runs this; it is a **setup-form** step, gated on the user having chosen JMAP.

8. **A found IMAP config gets a CalDAV follow-on probe.** Autoconfig/ISPDB describe **mail
   only** (they carry no calendar endpoint), so once IMAP settings are found the core
   probes `.well-known/caldav` (RFC 6764) on the account's **email domain** and its
   **provider's registrable domain** (derived from the winning IMAP host, e.g.
   `imap.soverin.net` → `soverin.net`, which advertises CalDAV even when the custom
   `allodia.eu` does not), concurrently, email-domain hit preferred. Only an HTTPS `401`
   (a credential challenge) or `207` (a WebDAV multi-status) counts, so a catch-all
   `301`-to-homepage is not a false positive, and because only HTTPS is followed, a
   discovered endpoint is always tamper-resistant-sourced. The client offers it as
   calendar sync **pre-selected** (opt-out), reusing the IMAP credentials; when nothing is
   found it offers an opt-in manual CalDAV field. The probe is **soft**: bounded by its own
   timeout, run outside the overall deadline, it never turns a found mail config into a
   miss, and the engine still does the real authenticated collection discovery at connect
   (a wrong guess degrades to "no calendar", surfaced non-blocking, never a broken
   account). Discovery attaches to the **IMAP** route only; JMAP/Microsoft/Gmail carry
   their calendar over their own session and need no probe.

9. **SRV autodiscovery covers providers that publish services in DNS, not on the apex.** Two
   strategies use the host resolver's SRV lookup (rule 6). The **JMAP probe**, on an apex
   miss, resolves `_jmap._tcp.{domain}` and probes each target's `.well-known/jmap`: the
   Fastmail shape, whose apex `.well-known/jmap` `302`s to a `404` while `_jmap._tcp` points
   at `api.fastmail.com`. A separate **IMAP/SMTP SRV** strategy resolves both the
   implicit-TLS (`_imaps._tcp` 993 / `_submissions._tcp` 465) **and** the STARTTLS
   (`_imap._tcp` 143 / `_submission._tcp` 587) service labels (RFC 6186/8314), **preferring
   implicit TLS** and falling back to the STARTTLS label only when the implicit-TLS one is
   absent. A STARTTLS SRV target is offered as STARTTLS (rule 4), never plaintext, and the
   engine's connect fails safe if the server doesn't advertise `STARTTLS`. IMAP is required
   (no IMAP SRV → no mail config); Submission is best-effort (a domain publishing neither
   submission label leaves send unconfigured, never blocked). The RFC 2782 `.` "service not
   offered" target is honoured.

   **Trust (rule 3):** an SRV-discovered endpoint is **trusted over validated TLS, without
   requiring DNSSEC.** SRV is the JMAP spec's own discovery mechanism ([jmap.io](https://jmap.io/crash-course/index.html)
   demonstrates it with the cross-domain `example.fm` → `api.fastmail.com` shape), and its
   target is the very host we then TLS-validate, on the probe *and* on every connect, with
   the resolved host **pinned** into the stored config so DNS can't move it afterward. It is
   deliberately trusted even for a **cross-registrable-domain** target (a custom domain hosted
   by a provider, the common case) and even when the provider runs no DNSSEC, as
   **Fastmail does by policy** ([their reasoning](https://www.fastmail.com/blog/dnssec-dane/):
   low adoption, high operational fragility). The residual risk (an on-path DNS attacker at the
   *one-time* setup moment redirecting to a host with a valid cert for *its* name) is the same
   exposure all non-DNSSEC mail autodiscovery has always carried; it is an accepted, documented
   known issue (see Known gaps) pending broader DNSSEC adoption, and the SRV answer's AD bit is
   still surfaced by the resolver for a future opt-in "require DNSSEC" setting. The discovered
   host is always shown on the found card so a cautious user can eyeball it. (SRV is the one
   place the scope exceeds Thunderbird for Android, which does none.)

10. **A consumer Google address needs no detection.** `gmail.com` / `googlemail.com` is always
    Google's native API (there is nothing on a server to probe), so the core routes it to the
    Google sign-in *before* racing any strategy. A **Workspace** (custom) domain still runs
    detection; it is recognised as Google when a strategy returns a Google-family incoming host
    (`imap.gmail.com` / `*.google.com` / `*.googlemail.com`, including a Google MX), and then
    routes to the same native Google flow rather than to IMAP. Google is native-API only: see
    [`provider-oauth.md`](provider-oauth.md) → "## Google".

## Strategy order

The whole flow has three stages: **(1)** the five discovery strategies below, raced in priority
order; **(2)** once a mail config is found, the [CalDAV follow-on](#the-rules) probe (rule 8, not
one of the raced strategies); **(3)** [routing](#routing) the winner onto a prefilled client form.
Stages 1–2 are the discovery; stage 3 is what the client does with the result.

The five strategies, for `user@company.example`, in priority order (each an independent parallel
task):

| # | Strategy | Requests (in order) |
|---|---|---|
| 0 | JMAP probe | `https://company.example/.well-known/jmap`, then (on a clean miss) `_jmap._tcp.company.example` SRV → `https://{target}/.well-known/jmap` per target |
| 1 | Autoconfig | `https://autoconfig.company.example/mail/config-v1.1.xml` · `https://company.example/.well-known/autoconfig/mail/config-v1.1.xml` · the same two over `http://` (untrusted) |
| 2 | ISPDB | `https://autoconfig.thunderbird.net/v1.1/company.example` |
| 3 | IMAP/SMTP SRV | `_imaps._tcp.company.example` (required) + `_submissions._tcp.company.example` (optional) SRV lookups, implicit-TLS labels only |
| 4 | MX fallback | host MX lookup → registrable domain via the Public Suffix List (+ the MX host minus its first label) → `https://autoconfig.{mxdomain}/mail/config-v1.1.xml` then the ISPDB for it |

The **JMAP probe** counts a domain as JMAP-capable on a terminal 2xx JSON session
(`capabilities` present) or a `401` + `WWW-Authenticate` (the Stalwart/Fastmail shape), with
every hop HTTPS. On an apex hit the stored base URL is `https://{domain}`; on an SRV hit it is
the target's origin (`https://api.fastmail.com`, port kept only when non-standard). Either way
the engine re-resolves `/.well-known/jmap` at connect, so no ephemeral redirect target is baked
in. The SRV path is skipped under the dev-harness override (which targets a fixed local base).

After a **mail** result, a **CalDAV follow-on** (rule 8, not one of the raced strategies)
probes `.well-known/caldav` on the email domain and the provider's registrable domain, and
attaches any discovered endpoint to the IMAP route. It uses only `.well-known`, no
`_caldavs._tcp` SRV lookup: the well-known signal covers hosted providers like Soverin
(whose calendar lives on the provider domain, not the custom email domain). The mail SRV
strategies now use the same host resolver, so a CalDAV SRV lookup is a small extension (see
Known gaps).

No port guessing/probing, no Exchange AutoDiscover, no bundled `providers.xml`: otherwise
the scope of Thunderbird for Android, **extended** with the JMAP and IMAP/SMTP SRV strategies
(rule 9), which Thunderbird omits.

## Routing

The core maps a detection result onto a route the client prefills (this encodes what *this*
engine can connect, so it lives in `mailcal-account`, not the protocol-neutral detector):

- **JMAP found** (apex or `_jmap._tcp` SRV, including a cross-domain provider host) → the JMAP
  form, server prefilled (password/token entry only). The discovered host is shown on the card.
- **A Microsoft-family incoming host** (`outlook.office365.com` / `*.office365.com` /
  `*.outlook.com`) → the Microsoft 365 browser sign-in, regardless of any Basic auth the
  ISPDB still lists (Microsoft retired it). The detected address is passed as the OAuth
  `login_hint` so Microsoft targets *that* account, and a declined/failed sign-in is shown on
  the card (never a silent dead-end); see [`provider-oauth.md`](provider-oauth.md) rules 8–9.
- **A Google address, or a Google-family incoming host** (`imap.gmail.com` / `*.google.com` /
  `*.googlemail.com`) → the native **Google (Gmail + Google Calendar)** browser sign-in. A
  **consumer** address (`gmail.com` / `googlemail.com`) routes there immediately with no server
  detection (rule 10); a **Workspace** domain is recognised from a detected Google-family host.
  The detected address is passed as the OAuth `login_hint`, and the client shows the **Early
  Access gate** before sign-in (see [`provider-oauth.md`](provider-oauth.md) → "## Google"). This
  is native-API only, Gmail + Calendar over Google's own session, so it never falls back to
  IMAP/CalDAV.
- **Otherwise the first TLS-or-STARTTLS + password incoming** → the IMAP form, host fields
  prefilled and the detected connection security (implicit TLS or STARTTLS) carried through to
  connect; the outgoing server likewise, or none when the provider has no SMTP the engine can
  use (send stays unconfigured rather than blocking mail-read). An `_imaps`/`_imap` /
  `_submissions`/`_submission` SRV result (rule 9) lands here too. A CalDAV endpoint found by the
  follow-on probe (rule 8) rides along on this route, offered as pre-selected calendar sync
  that reuses the IMAP credentials.
- **OAuth-only (non-Microsoft, non-Google)** / **nothing found** / **offline** → manual setup,
  with a plain-language reason line.

**A route the build cannot start is never recommended.** The Microsoft and Google sign-ins need
that provider's OAuth client registration, which is injected at build time
([`BUILDING.md`](../BUILDING.md)), and `recommend` is told which of them exist. Without Google's,
a Gmail or Workspace address takes the IMAP app-password route the native one supersedes; it
still works, which is why it is the fallback. Without Microsoft's there is no such route: Microsoft
retired Basic auth, so the ISPDB settings that remain cannot log in, and the address is reported as
**OAuth-only** rather than prefilled into a form that fails at the password. The wizard hides the
matching account-type choice on the same evidence, so neither can be reached by hand either.

## Privacy

Detection discloses the **domain**, never the local part (rule 2). Per-strategy attempts are
logged at **debug** level (which carries the domain-bearing URL); info-level logging carries
only strategy names + outcome kinds, consistent with [`logging.md`](logging.md)'s
never-log-content rule. Account connection is **not** jurisdiction-gated: the
`JurisdictionGate` is an AI/model concern, not an account-connection one (see
[`provider-oauth.md`](provider-oauth.md) → "Sovereignty scope"). The domain is disclosed to
the provider's own endpoints, to Mozilla's ISPDB, and to the device's DNS resolver (including
the `_service._tcp.{domain}` SRV queries of rule 9); a JMAP-SRV *target* host additionally sees
a `.well-known/jmap` probe, but only that host. [`privacy-policy.md`](privacy-policy.md) §2
discloses this (the ISPDB third party, the DNS lookups, domain-only). The CalDAV follow-on
(rule 8) stays within this envelope: it discloses only the email domain and the provider's
registrable domain to their own `.well-known/caldav` (both the user's own provider, no new
third party), never the local part.

## Per-platform matrix

Legend: ✅ implemented · 🚧 code-complete, runtime unverified · ⬜ planned · n/a marked "n/a".

| Gate | Shared core | macOS / iOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| Email-first prompt → routed prefill | ✅ | ✅ | ✅ | ✅ | ✅ |
| Detected servers shown to confirm, not retype (password is the only field) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Manual account-type picker (IMAP · JMAP · Microsoft · Google) | n/a | ✅ | ✅ | ✅ | ✅ |
| Google native route (consumer fast-path + Workspace-host) | ✅ | 🚧 | 🚧 | 🚧 | ✅ consumer fast-path |
| JMAP probe · autoconfig · ISPDB | ✅ | ✅ | ✅ | ✅ | ✅ |
| Untrusted-settings approval gate | ✅ | ✅ | 🚧 | ✅ | ✅ |
| "Set up manually" escape + reason line | ✅ | ✅ | 🚧 | ✅ | ✅ |
| MX fallback (host DNS) | ✅ | ✅ libresolv | 🚧 DnsQuery_W | ✅ DnsResolver | ✅ GIO Resolver |
| JMAP-SRV autodiscovery (`_jmap._tcp`) | ✅ | ✅ | 🚧 | ✅ | ✅ |
| IMAP/SMTP SRV (`_imaps`/`_submissions`, RFC 6186/8314) | ✅ | ✅ | 🚧 | ✅ | ✅ |
| DNSSEC AD bit read (reserved for a future "require DNSSEC" opt-in) | ✅ | n/a | n/a | ✅ | n/a |
| CalDAV follow-on discovery (RFC 6764) | ✅ | ✅ | 🚧 | ✅ | ✅ |
| JMAP OAuth metadata chain (RFC 9728 → 8414 → 7591), offered only when advertised | ✅ | ✅ | 🚧 | ✅ | ✅ |

The **CalDAV follow-on** offers the discovered calendar as an ✉ Email / 📅 Calendar sectioned
card on macOS/iOS/Android/Linux (a pre-checked opt-out toggle when found, an opt-in manual field
otherwise, hidden until the offer is accepted, so an empty box never reads as a calendar we
failed to fill in); Windows reveals the full prefilled form, so it prefills the existing calendar
field with the discovered endpoint (clear it to skip), verified by the routing tests. The
Windows prefilled form itself is now UIA-verified on a Windows host (a live IMAP detection
routed `someone@gmail.com` to the form with its host fields filled); the discovered-CalDAV
prefill specifically is still owed.

The **host DNS resolver** (MX **and** SRV via one `MxResolver` port) is the only per-platform
native code. SRV- and MX-derived configs are trusted on the validated TLS fetch regardless of
the AD bit, so they detect the same on every platform (no approval prompt for a normal DNS-based
setup on any client). The AD bit is nonetheless read where the API allows: Android via
`android.net.DnsResolver.rawQuery` with a self-built query; macOS/iOS (libresolv `res_9_query`)
and Windows (`DnsQuery_W`) do **not** surface it; Linux's GIO resolver adapter likewise reports it
unavailable, and plumbed through, **reserved for the future opt-in "require DNSSEC" setting**
(which would need those resolvers to surface the bit first). Where a host exposes raw DNS, its
MX/SRV wire codec is pure and unit-tested (`DnsMessage` on Android/Apple; Windows parses via the OS
`DnsQuery` and its routing logic is unit-tested in the plain net10.0 assembly). Linux instead uses
GIO's typed records and needs no wire codec.

On Apple platforms, `res_9_query` is **not safe to call concurrently**: its per-thread `_res`
isolates the resolver *state struct* but not the process-global `dns_res_send` configuration and
socket path, so the parallel MX/JMAP-SRV/IMAP-SRV lookups (the engine fans them out on separate
blocking threads) race inside it and crash: a SIGSEGV in `sock_eq`. `SystemMxResolver` therefore
serializes every libresolv query behind one process-wide lock; the lookups run only at setup and
return fast (usually NXDOMAIN), so the serialized cost is immaterial. Windows' `DnsQuery_W` and
Android's `DnsResolver` are concurrency-safe and need no such lock. (This only surfaced once SRV
autodiscovery added a second and third concurrent lookup; the MX-only era ran one at a time.)

## Known gaps

- **The manual setup form is implicit-TLS only.** Autodetection now routes STARTTLS servers
  (143/587) end-to-end: a detected STARTTLS provider connects. But the manual tabs, for a
  self-hosted server autodetection doesn't find, still assume implicit TLS (no security
  picker); a hand-typed STARTTLS-only server can't be set up. A manual connection-security
  selector across the clients is future work.
- **Google routing depends on host recognition for Workspace domains.** A consumer
  `gmail.com` / `googlemail.com` address is routed to native Google with certainty (rule 10),
  but a **custom Workspace domain** is only recognised when a strategy returns a Google-family
  incoming host (`imap.gmail.com` / `*.google.com` / a Google MX). A Workspace domain that
  resolves to neither (e.g. one fronted by a mail gateway) detects as generic IMAP instead,
  and the user picks Google manually. (Consumer Gmail no longer routes to IMAP app-password;
  that path is gone.)
- **DNS-based discovery is not DNSSEC-protected (accepted, documented).** SRV **and** MX-derived
  autodiscovery trust a result reached over CA-validated TLS without requiring DNSSEC, because
  DNSSEC adoption is low and major providers (incl. **Fastmail**) run none by policy. The
  residual risk is an on-path DNS attacker **at the one-time setup moment** redirecting to a
  host with a valid cert for *its* own name; after setup the resolved host is pinned and DNS
  can't move it (autodiscovery never runs again, see below). This is the same exposure all
  non-DNSSEC mail autodiscovery has, is one tap from "Set up manually", shows the discovered
  host for eyeballing, and the AD bit is plumbed for a **future opt-in "require DNSSEC" security
  setting** (a cross-platform Settings surface; it would also need the Apple/Windows/Linux
  resolvers to surface the AD bit, which they don't yet).
- **Autodiscovery runs only at account creation.** `detect_account_settings` is called solely
  from the setup screens; nothing in connect/reconnect/sync re-resolves a server. The chosen
  host is frozen in the stored config and TLS-revalidated on every connection, so the trust
  decision above is a one-time, setup-only exposure, not an ongoing one.
- **A wildcard-`401` server can be a JMAP false positive.** It routes to a JMAP form whose
  real connect then fails honestly, one tap from manual: bounded, not silent. (An SRV-target
  wildcard `401` is the same, one extra DNS hop removed.)
- **SRV can leave send unconfigured.** The IMAP/SMTP SRV strategy now queries both the
  implicit-TLS (`_submissions._tcp` 465) and STARTTLS (`_submission._tcp` 587) labels, so a
  domain publishing either gets send configured. A domain that publishes an IMAP SRV but
  *neither* submission label still gets mail-read with send unconfigured, the same honest
  degradation as the outgoing-`None` case above. (Such domains are usually also covered by
  autoconfig/ISPDB, which rank higher, and a Google-hosted domain routes to the native Google
  API before this applies.)
- **Windows MX/SRV resolver + some setup-UI paths are runtime-unverified.** The WinUI XAML half
  now **compiles and is UIA-verified on a Windows host**: the email-first prompt routes a live
  detection to a prefilled form (`someone@gmail.com` → `imap.gmail.com` / `smtp.gmail.com`), and a
  **detected STARTTLS config connects for real** (a live IMAP-STARTTLS probe to the seeded harness
  synced its folders). Still unverified on a Windows host: `WindowsMxResolver`'s live MX/SRV
  lookups (incl. `ResolveSrv`), the native Google sign-in route, and the untrusted-approval /
  "set up manually" / CalDAV-follow-on runtime paths: their routing/gating logic and the DNS
  wire codec are unit-tested cross-platform (net10.0), not yet driven end-to-end on Windows.
- **Linux is runtime-verified; the automated run and two strategies are what remain.** Every route
  was driven against **real providers on 2026-08-19**, on the developer's own accounts, and the
  diagnostic log names the winning strategy in each case: `mail via MxIspdb` and
  `mail via MxAutoconfig +caldav` (so the **GIO MX fallback** and the **CalDAV follow-on** both
  fired for real), `mail via Ispdb`, and `jmap via JmapSrv` (Fastmail's cross-domain
  `_jmap._tcp` → `api.fastmail.com` shape, trusted with no warning). From there: a detected
  Microsoft address through browser sign-in to a synced Graph account (6 folder providers plus the
  calendar), a Soverin IMAP address to a found card showing both servers with its discovered
  `caldav.soverin.net` pre-checked, Gmail to the Early-Access-gated Google card and a connected
  Gmail + Google Calendar, and the Fastmail JMAP sign-in through the full RFC 9728 → 8414 → 7591
  chain, **including a refresh-token rotation persisted to Secret Service on the first refresh**
  (the ratcheting-server path of [`provider-oauth.md`](provider-oauth.md) rule 5, previously
  unexercised on this platform).

  **Microsoft** routes to the same browser sign-in as the other clients rather than to a password
  form Microsoft would refuse. The **manual** form is no longer IMAP-only: an account-type picker
  offers IMAP / JMAP / Microsoft / Google, "Set up manually" on a detected card opens that route
  prefilled, and the manual JMAP pane runs the same fail-soft sign-in pre-flight when the user
  moves on from the address.

  Two rows are ✅ on the strength of a **driven widget tree** rather than a live provider, and the
  distinction is worth keeping: the **untrusted-approval gate** was exercised through the
  showcase's scripted non-HTTPS recommendation (no real `http://` provider was contacted: the
  gate under test is client-side, and Connect stays insensitive until the box is ticked), and the
  **miss reason line** through each `MissReason` rendered into the manual form. What is still
  owed: **IMAP/SMTP SRV** (`_imaps`/`_submissions`) never won a race here, because every address
  tried was covered by a higher-priority strategy; the complete **automated** email → provider
  sign-in → Secret Service save → synced inbox AT-SPI run; and the Graph refresh/rotation path,
  since no Microsoft token has rotated yet.

  **The detected JMAP card shows nothing to act on until the pre-flight answers.** It used to render
  the secret field immediately and take it away when the offer arrived (measured at 1.6 s against
  Fastmail, long enough to read as the app changing its mind). It now shows what it is waiting for,
  then either the button or the field. Because the card would otherwise have nothing to act on, a
  **deadline races the probe** (the discovery chain has no overall timeout of its own): whichever
  answer lands first decides, and a silent server falls back to the secret field rather than
  stranding the user. The **manual** pane keeps its secret field throughout: it is already on
  screen, so a negative answer changes nothing there and must not rebuild over a secret being
  typed.
- **CalDAV discovery is `.well-known`-only.** No `_caldavs._tcp` SRV lookup, though the mail
  SRV strategies now use the same host resolver, so adding one is a small extension. A provider
  that advertises CalDAV solely via SRV isn't found, and the user adds it manually. A
  Microsoft-family or Gmail result runs the CalDAV probe but discards it: those carry calendar
  over their own session, costing one bounded, concurrent request.
- **The discovered endpoint is a hint, not a guarantee.** The unauthenticated probe proves a
  CalDAV service exists; the engine's authenticated PROPFIND at connect is the real validation,
  and a failure degrades to "no calendar" (surfaced non-blocking), never a broken account.
- **JMAP accounts get no CalDAV probe.** Discovery attaches to the IMAP route only; a
  JMAP/Fastmail account's calendar is future work on that route.

## Testing

- **Core** (`crates/mailcal-autodetect`, offline): the parser (mutation-style, one test per
  error), the fetcher (one-shot 127.0.0.1 servers), the JMAP probe (apex **and** `_jmap._tcp`
  SRV fallback, trusted vs. approval-gated), the MX derivation, the IMAP/SMTP SRV strategy, SRV
  target selection (the `.` sentinel, a zero port, priority order, the trailing-dot trim), the
  CalDAV probe (classification + email-vs-provider candidate selection), and the orchestrator's
  priority/deadline behaviour under `tokio` paused time (including the **Fastmail-shape** case
  where a `_jmap._tcp` SRV hit beats an ISPDB IMAP result), ~108 tests. The recommendation
  mapping (`mailcal-account`) and the FFI conversion (`mailcal-bindings`) each have per-rule
  tests (including the CalDAV pass-through). A gated live test (`AUTODETECT_LIVE=1`) hits
  gmail/fastmail read-only.
- **Clients**: the DNS wire codec (MX **and** SRV: golden query bytes, parse, the `.` root
  target), the connect-gating, and the calendar opt-out/opt-in → effective-CalDAV-URL logic
  are unit-tested on Android (JVM), Apple (package `Testing`), and Windows (net10.0). Linux uses
  GIO's typed MX/SRV records and unit-tests trailing-dot/root-target normalisation plus the detected
  security/CalDAV conversion. End-to-end,
  detection was driven on a device against real public domains (gmail → Google sign-in, outlook →
  Microsoft) and against the local Stalwart harness (JMAP → connect → inbox sync) via the
  dev-only `MAILCAL_AUTODETECT_WELL_KNOWN_BASE` override (see [`debugging.md`](debugging.md)).
  The Fastmail JMAP-via-SRV path is proven by the deterministic orchestrator test above **and
  verified live on a real Android device** with the native resolver: `user@fastmail.com` routes to
  the JMAP found card showing `JMAP · api.fastmail.com`, **trusted, no warning**
  (`autodetect: jmap via JmapSrv (trusted)` in the log). Detection's job (route + prefill the right
  server) ends there; the subsequent JMAP-connect authentication is a separate concern.
- **Linux** additionally has a **GTK widget suite** driven under Xvfb, in the crate's single GTK
  test (GTK initialises once, on one thread, so what it covers lives in files beside it): which
  surface each route renders and (the half a screenshot cannot check) what it must *not* put on
  screen. An OAuth route exposes **zero** entry fields; a detected JMAP card mid-pre-flight shows
  neither the offer nor a secret; a failed sign-in restores the secret **and only it**, since
  detection already found the server; a discovered calendar is pre-checked with its manual field
  hidden until accepted; an unapproved untrusted card holds Connect insensitive until the box is
  ticked; the account-type picker reaches the model; and each `MissReason` renders its line.
  End-to-end it was driven against **real providers**; see the Linux entry under Known gaps for
  the strategies the log recorded.
