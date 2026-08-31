<!--
What the change does now, in present tense. The story of the bug belongs here rather than in a
comment that outlives it.
-->

## What this changes

## How it was verified

<!--
Be specific about what ran and where. "Ran the gate on macOS" and "tried it on a real iPhone" are
different claims, and both beat "works".
-->

- [ ] `scripts/dev/gate.sh` is green
- [ ] Tests cover the behaviour — a bug fix has a regression test that fails without the change

## Contracts

<!-- Delete the lines that do not apply. -->

- [ ] Touches a surface one of AGENTS.md's cross-platform contracts covers → that doc's rule **and**
      its matrix are updated, and every platform shipping the surface moves in this change (a
      shortfall is written under **Known gaps**, not left silent)
- [ ] A user-facing capability's reach shifted → the README capability matrix is updated
- [ ] A user could notice this → `docs/changelog/unreleased/<slug>.md` in every catalog locale,
      with `Platforms:` and `Bump:`
- [ ] Touches what the app stores, sends or shares → `docs/privacy-policy.md` is updated in every
      locale, and the PR says **"⚠️ publish: allodia.eu/privacy needs the matching update"**
