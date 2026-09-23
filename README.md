# Allodia Mail & Calendar

[![CI](https://github.com/allodia-eu/mail-calendar/actions/workflows/ci.yml/badge.svg)](https://github.com/allodia-eu/mail-calendar/actions/workflows/ci.yml)

Allodia Mail & Calendar is a mail and calendar app for **macOS, iPhone, iPad, Windows, Android
and Linux**. It is part of the [Allodia](https://allodia.eu) suite. It connects to accounts you
already have (IMAP/SMTP, CalDAV/CardDAV, JMAP, Microsoft 365), and no feature in it needs an Allodia
server to function, so a build from this source keeps working whether or not Allodia does.
[Screenshots](#screenshots) of every platform are at the end of this page.

This repository is the whole application. Deciding what happens is Rust's job and drawing it is the
client's: the product logic, the presentation state machines every client renders, the host-service
ports each platform implements and the UniFFI/C-ABI bindings live in the core, and the four native
clients that render it live under [`clients/`](clients): one for Apple's three platforms, one each
for Windows, Android and Linux. Beneath the core sits the product-neutral PIM sync engine, which is
its own repository. So a rule is written once rather than five times, and cannot drift between the
clients that show it.

The whole application (this core and every client) is **GPL-3.0-only**.
[`docs/pledge.md`](docs/pledge.md) is the promise in full: what is free, what stays free, and where
the line around a paid Allodia service is drawn.

## Getting it

| | |
|---|---|
| macOS, iPhone, iPad | [App Store](https://apps.apple.com/app/allodia-mail-calendar/id6792350379) |
| Windows | [Microsoft Store](https://apps.microsoft.com/detail/9nj7866z2nd3) |
| Android | [Google Play](https://play.google.com/store/apps/details?id=eu.allodia.mailcal) |
| Linux | [Flatpak](https://dl.allodia.eu/flatpak/eu.allodia.mailcal.flatpakref) |

On Linux, opening the link hands it to GNOME Software or Discover, which installs the app and keeps
it updated. From a terminal:

```sh
flatpak install https://dl.allodia.eu/flatpak/eu.allodia.mailcal.flatpakref
```

[`docs/capabilities.md`](docs/capabilities.md) is the row-by-row truth for what each client ships,
including the few rows a platform does not claim yet.

## Where to start

- **Using the app:** the help pages at [`allodia.eu/docs/mail-calendar`](https://allodia.eu/docs/mail-calendar/)
- **Building it:** [`BUILDING.md`](BUILDING.md), plus
  [`clients/linux/README.md`](clients/linux/README.md#prerequisites) for the Linux prerequisites.
  You need no credential of ours: the Google and Microsoft registrations are injected at build time,
  and a build given none drops those two sign-in routes rather than failing.
- **How it is put together:** [`docs/architecture.md`](docs/architecture.md): the layering, the
  crate map, and the dispatch → snapshot loop
- **What each client ships**, capability by capability: [`docs/capabilities.md`](docs/capabilities.md)
- **The cross-platform contracts**, one enforceable rule per file: [`docs/`](docs/README.md)
- **The rulebook**, written for humans and coding agents alike: [`AGENTS.md`](AGENTS.md)

## Contributing

[`CONTRIBUTING.md`](CONTRIBUTING.md) is the front door; [`AGENTS.md`](AGENTS.md) is the full
rulebook and the one place each rule is stated. A security problem goes through
[`SECURITY.md`](SECURITY.md), privately, never an issue. You may ship your own build, under your
own name: [`TRADEMARKS.md`](TRADEMARKS.md). A first pull request signs [`CLA.md`](CLA.md), which
is what lets the same code ship in Allodia's store builds.

## License

The application (this core and every client) is **GPL-3.0-only**; the full text is in
[`LICENSE`](LICENSE). Two vendored pieces keep their own: the Gradle wrapper (Apache-2.0) and the
Contributor Covenant (CC-BY-4.0). [`REUSE.toml`](REUSE.toml) says the same thing per file in a form
`reuse lint` can check, so the claim cannot quietly stop being true.

One directory is not the application: [`allodia_license/`](allodia_license) holds the client
surfaces for capabilities that exist only because Allodia runs a service behind them. Using those
needs a subscription, and the [Allodia License](allodia_license/LICENSE.md) publishes the code to
be read and audited rather than reused. It changes nothing about the app: the application builds,
tests and runs without that directory, and `cargo xtask check-license-dir` fails if anything
outside it so much as names something inside.

Why this licence, what stays free in every build, and what a paid Allodia service may ever be:
[`docs/pledge.md`](docs/pledge.md).

## Screenshots

Every client draws the same core state in its platform's own toolkit: SwiftUI on Apple, WinUI 3 on
Windows, GTK4 and libadwaita on Linux, Jetpack Compose on Android. None is a web page in a window;
only the message body and the composer, which are HTML, sit in a web view. The screenshots show the
built-in showcase data, not a real mailbox.

### macOS

| The inbox | An invitation |
|---|---|
| ![The inbox on macOS](docs/images/screenshots/macos/list.webp) | ![An invitation on macOS](docs/images/screenshots/macos/invitation.webp) |
| **The calendar** | **Dark appearance** |
| ![The calendar on macOS](docs/images/screenshots/macos/calendar.webp) | ![The inbox on macOS, dark](docs/images/screenshots/macos/list-dark.webp) |

### Windows

| The inbox | An invitation |
|---|---|
| ![The inbox on Windows](docs/images/screenshots/windows/list.webp) | ![An invitation on Windows](docs/images/screenshots/windows/invitation.webp) |
| **The calendar** | **Dark appearance** |
| ![The calendar on Windows](docs/images/screenshots/windows/calendar.webp) | ![The inbox on Windows, dark](docs/images/screenshots/windows/list-dark.webp) |

### Linux

| The inbox | An invitation |
|---|---|
| ![The inbox on Linux](docs/images/screenshots/linux/list.webp) | ![An invitation on Linux](docs/images/screenshots/linux/invitation.webp) |
| **The calendar** | **Dark appearance** |
| ![The calendar on Linux](docs/images/screenshots/linux/calendar.webp) | ![The inbox on Linux, dark](docs/images/screenshots/linux/list-dark.webp) |

### iPad

| The inbox | An invitation | The calendar | Dark appearance |
|---|---|---|---|
| ![The inbox on iPad](docs/images/screenshots/ipad/list.webp) | ![An invitation on iPad](docs/images/screenshots/ipad/invitation.webp) | ![The calendar on iPad](docs/images/screenshots/ipad/calendar.webp) | ![The inbox on iPad, dark](docs/images/screenshots/ipad/list-dark.webp) |

### iPhone

| The inbox | An invitation | The calendar | Dark appearance |
|---|---|---|---|
| ![The inbox on iPhone](docs/images/screenshots/iphone/list.webp) | ![An invitation on iPhone](docs/images/screenshots/iphone/invitation.webp) | ![The calendar on iPhone](docs/images/screenshots/iphone/calendar.webp) | ![The inbox on iPhone, dark](docs/images/screenshots/iphone/list-dark.webp) |

### Android

| The inbox | An invitation | The calendar | Dark appearance |
|---|---|---|---|
| ![The inbox on Android](docs/images/screenshots/android/list.webp) | ![An invitation on Android](docs/images/screenshots/android/invitation.webp) | ![The calendar on Android](docs/images/screenshots/android/calendar.webp) | ![The inbox on Android, dark](docs/images/screenshots/android/list-dark.webp) |
