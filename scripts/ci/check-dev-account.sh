#!/usr/bin/env bash
# Guard the dev-account (local Stalwart harness) contract. Runs in the always-run `changes` CI job
# (not `lint`, which is gated on the `rust` area; most of what this checks is client code), and
# locally:
#
#     scripts/ci/check-dev-account.sh
#
# `MAILCAL_DEV_ACCOUNT` swaps the developer's real mailbox for a throwaway loopback one, and its
# IMAP mode additionally teaches the core to trust a self-signed certificate. Both properties fail
# *silently* when they break; a drifted fixture just fails to log in, and a trust path that leaks
# into a shipped build looks like nothing at all. Machine-check them.
#
# Repository-wide searches use `git grep`, so they never descend into a submodule, `target/`, or
# the gitignored generated bindings. Explicit fixture paths add `--untracked` so this check also
# works before a newly added client is staged.
set -euo pipefail

fail=0

# The four clients that inject the harness IMAP account each hand-write the same TOML: the config
# builder can't produce it (it always derives `server_name` from the dialed host, and the harness
# dials by IP while its cert's only SAN is `localhost`). So the fixture is duplicated by necessity,
# in four languages, and a value edited in one place fails to log in from exactly one platform,
# with a plain "authentication failed", nowhere near the edit. Pin the four fields together, and to
# the addresses `scripts/dev/*` dials and seeds.
# Named by directory rather than by file. The fields have to exist in the client; which file holds
# them is not the invariant, and splitting one for the 500-line limit has already moved them once.
imap_clients=(
  clients/apple/Packages/MailcalKit/Sources/MailcalUI
  clients/android/app/src/main/java/eu/allodia/mailcal
  clients/windows/Mailcal/Services
  clients/linux/src
)
imap_fields=(
  'addr = "127.0.0.1:12993"'
  'server_name = "localhost"'
  'username = "alice@test.local"'
  'password = "harness-alice-pw"'
)
for file in "${imap_clients[@]}"; do
  for field in "${imap_fields[@]}"; do
    if ! git grep --untracked -qI --fixed-strings "$field" -- "$file"; then
      printf 'ERROR: nothing in %s carries the harness IMAP fixture field:\n  %s\n' "$file" "$field" >&2
      printf 'All four clients inject the same hand-written [imap] config; a drift here logs in\n' >&2
      printf 'from every platform but one, and says only "authentication failed".\n' >&2
      fail=1
    fi
  done
done

# The same two values the harness itself is built around (lib.sh dials the listener and seeds
# alice's password). A port or password changed there must reach the clients' fixtures.
for value in '127.0.0.1:12993' 'harness-alice-pw'; do
  if ! git grep -qI --fixed-strings "$value" -- scripts/dev/lib.sh; then
    printf 'ERROR: scripts/dev/lib.sh no longer carries %s, which the clients hard-code.\n' "$value" >&2
    printf 'The harness and the injected fixture must dial the same listener with the same password.\n' >&2
    fail=1
  fi
done

# Every client compiles the switch out of a release build, so a shipped binary can never open the
# harness mailbox because of a stray environment variable; the same property check-showcase-flag.sh
# asserts for MAILCAL_SHOWCASE. Apple and Windows use `#if DEBUG`; Android decodes the mode only
# when FLAG_DEBUGGABLE is set.
for file in \
  clients/apple/Packages/MailcalKit/Sources/MailcalUI/MailcalModel+DevAccount.swift \
  clients/windows/Mailcal/Services/MailboxModel.Accounts.cs; do
  if ! git grep -qI --fixed-strings '#if DEBUG' -- "$file"; then
    printf 'ERROR: %s has no `#if DEBUG` guard around the dev-account switch.\n' "$file" >&2
    printf 'A release build must ignore MAILCAL_DEV_ACCOUNT entirely.\n' >&2
    fail=1
  fi
done
if ! git grep -qI --fixed-strings 'FLAG_DEBUGGABLE' -- clients/android/app/src/main/java/eu/allodia/mailcal; then
  printf 'ERROR: the Android dev-account switch is no longer gated on FLAG_DEBUGGABLE.\n' >&2
  printf 'A release build must ignore MAILCAL_DEV_ACCOUNT entirely.\n' >&2
  fail=1
fi
if ! grep -qF -- '#![cfg(debug_assertions)]' clients/linux/src/dev_account.rs; then
  printf 'ERROR: the Linux dev-account fixture is not compiled only under `debug_assertions`.\n' >&2
  printf 'A release build must ignore MAILCAL_DEV_ACCOUNT entirely.\n' >&2
  fail=1
fi

# The IMAP mode's extra trust anchor stays out of production. `dev_tls::extra_ca_anchors` folds the
# PEM named by MAILCAL_EXTRA_CA into the account's TLS roots; a shipped binary that kept it would
# trust any CA an attacker could place on disk and name in the environment. Two things hold it out:
# the `cfg(not(...))` arm in tls.rs, and `dev-harness` never being a default feature.
if ! git grep -qI --fixed-strings '#[cfg(not(any(debug_assertions, feature = "dev-harness")))]' -- crates/mailcal-account/src/tls.rs; then
  printf 'ERROR: crates/mailcal-account/src/tls.rs no longer excludes the extra-CA loader from\n' >&2
  printf 'production builds. `custom_roots()` must be an empty Vec unless debug_assertions or the\n' >&2
  printf '`dev-harness` feature is on, or a shipped binary trusts whatever MAILCAL_EXTRA_CA names.\n' >&2
  fail=1
fi
for manifest in crates/mailcal-account/Cargo.toml crates/mailcal-bindings/Cargo.toml; do
  if git grep -qIE '^default *= *\[.*dev-harness' -- "$manifest"; then
    printf 'ERROR: %s enables `dev-harness` by default.\n' "$manifest" >&2
    printf 'It must stay opt-in (the Android dev loop passes --features dev-harness); on by default\n' >&2
    printf 'it would compile the harness CA trust path into a shipped release binary.\n' >&2
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi

echo 'OK: the dev-account contract holds (IMAP fixture in step, switch is debug-only, harness CA trust excluded from release).'
