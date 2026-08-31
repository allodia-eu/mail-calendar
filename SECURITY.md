# Reporting a security issue

**Please do not open a public issue for a security problem.** A mail client holds someone's
correspondence; a report that is public before there is a fix is a report that helps an attacker
first.

## How to report

- **Preferred:** GitHub's private vulnerability reporting: the **Security** tab, then *Report a
  vulnerability*. It is private to the maintainers and keeps the discussion in one place.
- **By mail:** **info@allodia.eu**.

Useful in a report: what an attacker gains, the steps to reproduce it, the platform and app
version, and the kind of account involved (IMAP, JMAP, Microsoft Graph, Google, CalDAV, CardDAV).
The providers differ enough that a flaw in one path often does not exist in another.

**Send no real mail, credentials or tokens.** A redacted excerpt is enough; if a token is what
demonstrates the problem, say so and we will arrange somewhere to put it.

## What happens next

This is a small team, so what is promised is what can be kept: an acknowledgement, an honest
assessment of severity, and a fix released as soon as one is ready. You will be credited in the
release notes unless you would rather not be. There is no bug-bounty programme.

## What is in scope

This repository: the Rust core and the macOS, iOS/iPadOS, Windows, Android and Linux clients.

Two things live elsewhere and are still worth reporting here; we will route them:

- The **PIM sync engine**, which owns every protocol implementation (IMAP, JMAP, DAV, MIME,
  iCalendar), in its own public repository.
- The **services behind the paid tier**, which are closed source.

A finding in a dependency belongs upstream first. Tell us anyway if the app's use of it is what
makes the flaw reachable.

## What the app already promises

Two contracts describe the gates untrusted content passes and the way the composer is hosted:
[`docs/rendering-security.md`](docs/rendering-security.md) and
[`docs/composer-security.md`](docs/composer-security.md). Both carry a **Known gaps** section
naming what is *not* defended against; reading it first may save you the report, or sharpen it.
