# Contributing

Thanks for looking. This is a mail and calendar app people trust with their correspondence, so the
bar is on correctness and on not surprising anyone, not on volume.

Start with [`README.md`](README.md) for what the project is, and [`docs/pledge.md`](docs/pledge.md)
for what it promises to stay.

## The rules live in AGENTS.md

[`AGENTS.md`](AGENTS.md) is the full rulebook: layering, the cross-platform contracts, testing,
the traps that have caught people. It is written for both humans and coding agents, and it is the
one place a rule is stated, so this file links rather than repeats. Read the sections your change
touches; you do not need the rest.

The four that catch most first contributions:

- **Files stay under 500 lines**, in every language here, enforced in CI. Split by responsibility.
- **A behaviour change is test-first, and a bug fix gets a regression test** at the lowest layer
  that observes the contract.
- **Protocol knowledge belongs in the sync engine**, not here. About to parse an iCalendar
  property, a MIME part or a DAV payload? Search the engine first: a second parser here is one
  that will disagree with the one that ships.
- **A cross-platform contract binds every client.** If your change touches a surface one of the
  docs in AGENTS.md's contract table covers, the same change updates that doc *and* every platform
  that ships the surface. A shortfall goes under that doc's **Known gaps**, never left silent.

## Building

[`BUILDING.md`](BUILDING.md) has the toolchains. Two things worth knowing before you start:

- **You do not need Google or Microsoft credentials.** They are injected at build time, and a build
  given none simply drops those two sign-in routes from the setup wizard. That is a supported
  build, not a broken one, so a fork, and every pull request from one, builds green without any
  secret.
- **Debug against the local seeded mail server**, not a personal account. `scripts/dev/harness.sh`
  brings up a Stalwart instance with deterministic data; [`docs/debugging.md`](docs/debugging.md)
  has the loop for each platform.

## Before you push

```sh
scripts/dev/gate.sh            # --clients also builds every client this host can
```

That is the same set of checks CI runs, ordered cheapest-first so the step most likely to fail on
your change fails first. Run it: CI is where a green build is *confirmed*, not where it is
discovered.

## Opening a pull request

- **One change per pull request.** If the work chains, stack it: each branch bases on the one below
  (`gh pr create --base <the-branch-below>`), and they merge bottom-up. AGENTS.md → "Building &
  verifying" explains why a correct base is not yet a registered stack.
- **A user-facing change writes its changelog fragment in the same change**:
  `docs/changelog/unreleased/<slug>.md`, every locale in the catalog, one sentence. Refactors,
  tests, docs and tooling write none.
- **Say what you verified**, and on what. "Ran the gate" and "tried it on an iPhone" are different
  claims, and both are more useful than "works".
- Describe what the code does now. Ticket numbers and the story of the bug belong in the pull
  request, not in a comment that outlives it.

## Everything else

- **You sign a contributor licence agreement once**, on your first pull request:
  [`CLA.md`](CLA.md). You keep your copyright; it exists so Allodia's App Store builds can
  ship at all, and what Allodia commits to in return is [`docs/pledge.md`](docs/pledge.md).
- **A security problem is not an issue**: [`SECURITY.md`](SECURITY.md).
- **The name is not part of the licence**: you may ship your build; ship it under your own name.
  [`TRADEMARKS.md`](TRADEMARKS.md).
- **Be decent to people**: [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
- **Licensing**: the application is GPL-3.0-only. [`LICENSE`](LICENSE), and
  [`REUSE.toml`](REUSE.toml) for the per-file statement.

Not sure whether something is wanted? Open an issue and ask before building it. That is a cheaper
conversation than a closed pull request.
