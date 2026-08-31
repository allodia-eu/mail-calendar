# Building

How to get a build with Google and Microsoft sign-in working, and what you get without one.

For the build commands themselves (the gate, each client's build script, the toolchain rules),
see [`AGENTS.md`](AGENTS.md) → "Building & verifying".

## Provider sign-in is optional

Connecting a Gmail or Microsoft 365 account over the provider's own API needs an OAuth **client
registration** on that provider, which is per-project: ours is not yours, and it is not in this
tree. So the registrations are injected at build time and **absent is a supported build**, not a
broken one.

A build without them is a complete open-standards mail and calendar client (IMAP, JMAP, SMTP,
CalDAV, CardDAV) and simply does not offer the two browser sign-ins. The setup wizard drops the
routes rather than showing buttons that would fail at the provider, and account detection follows:
a Gmail or Workspace address falls back to the IMAP app-password route, while a Microsoft-hosted one
says the domain offers only OAuth, because Microsoft retired Basic auth and there is nothing to fall
back to.

## The variables

| Variable | Read when building for | What it is |
|---|---|---|
| `MAILCAL_GOOGLE_DESKTOP_CLIENT_ID` | macOS, Windows, Linux | A Google **Desktop app** OAuth client id |
| `MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET` | macOS, Windows, Linux | That client's secret: non-confidential, and Google's token endpoint requires it even under PKCE |
| `MAILCAL_GOOGLE_IOS_CLIENT_ID` | iOS, iPadOS | A Google **iOS** OAuth client id |
| `MAILCAL_GOOGLE_ANDROID_CLIENT_ID` | Android | A Google **Android** OAuth client id |
| `MAILCAL_MS_CLIENT_ID` | every target | The Application (client) ID of an Azure app registration |
| `MAILCAL_ALLODIA_CLIENT_ID` | every target | The client id for signing in to an **Allodia account**, which only Allodia's own builds carry. A build without it has no Allodia sign-in at all, which is the whole of what an unbranded build is missing here. It also needs the `allodia-license` feature: see below. |
| `MAILCAL_ALLODIA_HOST` | every target | Which account service that sign-in talks to. Defaults to Allodia's own; set it only to point a development build at a local instance. |
| `ALLODIA_TELEMETRY_URL` | every target | Where consented product analytics are reported ([`docs/analytics.md`](docs/analytics.md)). A build without it constructs no sink and sends nothing at all; it still asks for consent and still previews the payload truthfully, it simply has nowhere to send. |
| `ALLODIA_TELEMETRY_APP_KEY` | every target | Which product the relay is being told about. Not a secret (it ships in the binary) and not required: a build with the endpoint but not this one falls back to a default and still reports. |

Google's three are three genuinely different registrations: Google issues a **separate client per
client type**, and the type decides the redirect and whether a secret is involved at all. Set all of
them and each build picks the one for its target; nothing needs to know which platform it is
building for. Microsoft registers one app for every platform.

Set a variable to nothing and it counts as unset, so a CI run with no access to the secrets behaves
exactly like a from-source build.

None of these values is confidential. A client id is public by construction, and Google's own
[installed-app guidance](https://developers.google.com/identity/protocols/oauth2#installed) says the
Desktop secret "is obviously not treated as a secret": an installed binary cannot keep one, and it
grants nothing without a fresh PKCE verifier and the user's consent. They are injected rather than
committed so the tree can be published without carrying one project's registrations.

What each provider needs registered at its end (redirect URIs, scopes, the Android client's
"Enable Custom URI scheme" switch) is in [`docs/provider-oauth.md`](docs/provider-oauth.md).

## The one that also needs a feature

`MAILCAL_ALLODIA_CLIENT_ID` is the only variable that is not sufficient on its own. The code it
turns on lives in [`allodia_license/`](allodia_license), which is source-available rather than GPL
and which the open tree must build without ([`docs/pledge.md`](docs/pledge.md), promise 4), so it is
an optional, off-by-default dependency, and an Allodia build asks for it:

```sh
cargo build -p mailcal-bindings --features allodia-license
```

Either half missing is the same supported outcome and not a failure: no Allodia sign-in, and a
complete mail and calendar client. In a build **we ship** it is a failure like any other missing
registration, and the release guard below refuses it. The two halves cannot contradict each other, because
`allodia_sign_in_available()` (which is what a client draws its button from) answers `false`
unless **both** are present.

**A client build asks for nothing.** Every client's build front door derives the feature from the
variable (environment first, then `.env`, the order and file the core's own build script reads),
so a checkout with the registration builds the four sign-in screens into the app and one without it
builds them out, with no second switch to remember. One rule, four resolvers, because the front
doors speak four languages: `core_cargo_features` in
[`scripts/dev/lib.sh`](scripts/dev/lib.sh) (Apple, Android, Linux),
[`Get-CoreCargoFeatures`](clients/windows/core-features.ps1) (Windows), and `credentialValue` in the
[Android Gradle build](clients/android/app/build.gradle.kts), which needs its own because Gradle
builds the host cdylib the Kotlin bindings are generated from.

The Linux **Flatpak** is the one that does not: it builds with default features on purpose, so a
release bundle cannot link `dev-harness` or the debug-only account fixture. That client does not
ship yet; the gap is recorded in
[`allodia_license/entitlement.md`](allodia_license/entitlement.md).

## Locally: `.env`

Put them in `.env` at the repo root, the same gitignored file the publishing scripts read:

```sh
MAILCAL_GOOGLE_DESKTOP_CLIENT_ID=…apps.googleusercontent.com
MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET=…
MAILCAL_GOOGLE_IOS_CLIENT_ID=…apps.googleusercontent.com
MAILCAL_GOOGLE_ANDROID_CLIENT_ID=…apps.googleusercontent.com
MAILCAL_MS_CLIENT_ID=…
MAILCAL_ALLODIA_CLIENT_ID=…
```

`chmod 600 .env`. It is listed in [`.worktreeinclude`](.worktreeinclude), so a new worktree comes up
with a copy.

Every client builds the core through a different front door (Gradle, `xcodebuild`, MSBuild, the
GNOME SDK, plain `cargo`), so the file is read by the core's own build script
([`crates/mailcal-oauth/build.rs`](crates/mailcal-oauth/build.rs)) rather than by each of them. That
covers all of them at once, including a bare `cargo build`. **The real environment always wins**, so
you can override one value for a single build without editing the file.

Changing `.env` rebuilds what depends on it. Nothing else needs doing.

⚠️ **Moving `.env` aside and back does not**, which is the shape anyone takes to see the absent
case: the build without a registration. Cargo decides whether to re-run a build script by
comparing the file's **mtime**, and a move preserves it: the file comes back older than the last
run, so nothing re-runs and the binary silently keeps the values from the build you were undoing.
It looks exactly like the feature being broken. `touch .env` after putting it back.

## In CI: secrets

Hand them in as environment variables from repository secrets. A pull request from a fork receives
**no** secrets, so those runs build the absent case, which is the point: it is the build outside
contributors get, and it has to stay green.

## Release builds refuse to be built without them

Everything above makes a missing value harmless. That is right for a build someone makes for
themselves and wrong for one **we ship**: it would be correct in every respect except in front of a
user, and no test can see the difference, because a binary built without these is supposed to behave
exactly like that.

So `MAILCAL_REQUIRE_INJECTED_CONFIG=1` turns a missing value into a **compile error** naming the
ones it wanted: no artifact is produced at all. Only the variables the target actually compiles in
are required, so the message is never spurious: a macOS release is not asked for Android's client
id, and the app key is never asked for because it has a default.

Two build scripts enforce it, one per concern: [`mailcal-oauth`](crates/mailcal-oauth/build.rs) for
the registrations, [`mailcal-bindings`](crates/mailcal-bindings/build.rs) for
`ALLODIA_TELEMETRY_URL`, over one shared resolver,
[`mailcal-buildenv`](crates/mailcal-buildenv/src/lib.rs). **One flag covers both**, deliberately: a
second name would have to be added to every packaging path and to each new one, and the path that
forgot it would ship exactly the artifact this is here to prevent. The flag was called
`MAILCAL_REQUIRE_OAUTH_CREDENTIALS` when the registrations were the only thing injected; that
spelling is still honoured, so a path or a fork that has not been updated keeps its guarantee rather
than silently losing it.

Every shipping path sets it, and nothing else does:

| Path | Where |
|---|---|
| macOS / iOS packaging | `clients/apple/Scripts/package.sh` |
| Windows MSIX | `clients/windows/package.ps1` |
| Android release | `clients/android/build-release.sh` |
| Linux Flatpak | the tag-only CI steps, forwarded into the sandbox by `clients/linux/package.sh` |
| CI release builds | the three tag-gated steps in [`ci.yml`](.github/workflows/ci.yml) |

It is deliberately **not** workflow-level in CI, and deliberately off by default: an ordinary push,
a from-source build and every fork must keep building without secrets.

### The Flatpak is the one that needs help

`flatpak-builder` runs `cargo build` inside `flatpak build`, which forwards **no** host environment:
the manifest's `build-options.env` is the whole of it, and that is committed YAML no secret may go
into. What does cross the boundary is the source tree, which flatpak-builder copies, `.env`
included.

On a developer's machine that is already true and there is nothing to do. In CI there is no `.env`,
so `clients/linux/package.sh` writes a temporary one (mode 600) from the environment and removes it
on the way out, however the build exits. Without that, a tagged build would hand users a Flatpak
with both sign-ins quietly missing.

**The requirement travels by a different road than the credentials it guards.** If it rode in
`.env` too, then a `.env` that failed to cross would take the requirement with it: the build inside
would see nothing to enforce, and the bundle would ship credential-free behind a green run: the
guard unable to fire in exactly the case it exists for. It is not a secret, so `package.sh` puts it
in a generated copy of the Flatpak manifest instead, and fails outright if it cannot. A `.env` that
then does not arrive is a compile error naming the variables, not a silent downgrade.

## Checking what a build carries

`oauth_routes()` over the FFI reports which sign-ins a binary offers, and it is what every client
asks before drawing its account-type picker. In the app, the answer is visible as whether
**Settings → Accounts → Add account** lists Microsoft 365 and Google at all.

## The other thing a build is given

The app's **name and application id** are injected the same way, from `branding/`, and are not
secret: [`docs/branding.md`](docs/branding.md). A checkout with `branding/allodia.env` builds
Allodia Mail & Calendar; one without it builds the unbranded default, under a different id and in
a different data directory. Nothing has to be switched off to get there, and nothing needs setting
up to build either one.
