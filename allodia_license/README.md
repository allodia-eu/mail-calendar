# `allodia_license/`

Everything in this directory is under the [Allodia License](LICENSE.md), not the GPL. It is the
only part of this repository that is.

**The application is not affected by it.** The mail and calendar app is GPL-3.0-only, free, with no
commercial-use restriction (for one person or for a whole company) and
[`docs/pledge.md`](../docs/pledge.md) is the promise that every capability listed there stays that
way. Nothing here narrows that, now or later.

## What lives here

The client surfaces for capabilities that exist only because Allodia runs a service behind them:
signing in to an Allodia account, and whatever a paid plan turns on. Using them needs a current
subscription. The source is published so it can be read and audited, not so it can be reused:
[`LICENSE.md`](LICENSE.md) §2 is what anyone may do with it, §4 is what nobody may.

Today that is one crate, `crates/allodia-license`: it asks what the account is entitled to, and
decides what a client draws between one answer and the next. **[`entitlement.md`](entitlement.md)
is the contract**: read that first; the crate is where its rules are implemented so that four
clients cannot each decide them slightly differently.

It opens no socket (the host passes a `Transport`, because the app already owns TLS and a second
provider in one process is a runtime conflict) and reads no clock (`now` arrives as Unix seconds).
Both are what make every rule in it testable without a network and at an arbitrary time, and the
fourteen tests are.

Signing in is Authorization Code with PKCE against the account service, which is an OAuth 2.0
authorization server. The flow itself is `mailcal-oauth`'s (the same one Microsoft, Google and
standards-discovered JMAP already use), so `signin.rs` is four facts and no protocol: which issuer,
which scopes, which redirect, and that the registration is **injected rather than
self-registered**: a first-party app has no reason to mint a registration on every install, and a
static one can be revoked.

No password ever reaches this crate, and `available()` is false in a build carrying no
registration, so a build from source has no Allodia sign-in surface at all, rather than one that
fails when pressed.

## What does not live here

Anything a mail and calendar client does on its own. If a capability works against the user's own
server, it belongs in the free tree and stays there. That is the pledge's fifth promise, and it is
the rule that decides every future boundary question, not a preference.

## Why it is a directory rather than a repository

Because the seam has to be visible. A closed component in a private repository is a thing you have
to take on trust; one in the open tree, excluded from the default build, is one you can check. The
default build is checked, in CI: `scripts/ci/check-license-dir.sh` fails if anything outside this
directory references anything inside it.

That check is what makes the pledge's fourth promise mechanical rather than aspirational: the open
repository compiles, tests and runs with no reference to anything closed. Delete this directory and
the build does not notice.

## How it is kept out of the default build

- **Rust**: the crate is a workspace **member** and deliberately absent from `default-members`, so
  a bare `cargo build` does not compile it while `cargo test --workspace` does. Membership is what
  lets it share one lockfile, one set of dependency versions and one copy of the engine: it reuses
  `mailcal-oauth` rather than carrying a second OAuth client, and its own workspace would have meant
  a second engine on disk for one small crate. `default-members` is an explicit list rather than a
  subtraction, so nothing joins the default build by accident.
- **The one line that connects them**: `crates/mailcal-app/Cargo.toml` carries the crate as an
  **optional** dependency behind an `allodia-license` feature, off by default. `cargo build` does
  not compile it and `cargo tree` does not list it; `cargo build --features allodia-license` does
  both. `check-license-dir.sh` allows exactly that line and the feature that gates it. Drop
  `optional = true` and the check fails, because that is the moment the open tree stops standing
  alone.
- **Every client**: nothing in `clients/` refers to this directory. Allodia's own builds pass the
  feature; the check above is what proves no one else's does.

## Contributing to it

The same as anywhere else in the repository: [`CONTRIBUTING.md`](../CONTRIBUTING.md) and
[`AGENTS.md`](../AGENTS.md) apply unchanged, and a contribution here is covered by the same
[`CLA.md`](../CLA.md). The licence grants what that needs: you may build and run this code locally
without a subscription in order to work on it ([`LICENSE.md`](LICENSE.md) §5).
