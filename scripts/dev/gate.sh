#!/usr/bin/env bash
# The local gate, as one command, in fail-fast order.
#
#     scripts/dev/gate.sh                 # the workspace gate: fmt, docs, lint, tests, checkers
#     scripts/dev/gate.sh --clients       # ...plus every client this host can actually build
#     scripts/dev/gate.sh --keep-going    # run everything and summarise, instead of stopping
#     scripts/dev/gate.sh --list          # print the steps and exit
#
# This is the executable form of AGENTS.md → "The local gate, in the order CI runs it". That list
# is still the documentation; this is the thing you run, so the gate cannot be half-run from
# memory. Run it before the first push of a branch. CI costs real money and macOS runners bill at
# 10x, so a PR is where you *confirm* a green build, not where you discover one.
#
# ## Why the order is what it is
#
# **Cheapest first, and `cargo doc` early.** The order here is not CI's; it is chosen so the step
# most likely to fail on the change you just made fails *first*. That matters most for the doc
# gate, which has burned us repeatedly and always the same way:
#
#   * `cargo test` and `cargo clippy` never invoke rustdoc, so a broken doc link survives both;
#   * this workspace denies `rustdoc::all`, so a link to a **private** item is a hard error while
#     ``[`X`]`` is the idiomatic thing to type and correct almost everywhere else;
#   * run last, that error lands minutes after the sentence that caused it, long past the point
#     where you would connect the two.
#
# A warm `cargo doc --no-deps` over this workspace takes **under six seconds**. It was never the
# expensive step, it was just the *last* one. So it runs third here, before clippy and the tests.
# (A heuristic "don't link private items" linter was tried instead and thrown away: it flagged
# eighteen places `cargo doc` is perfectly happy with, because `private_intra_doc_links` only
# fires for docs that are actually published. A checker that cries wolf gets skipped; the real
# check is six seconds away.)
#
# ## What it deliberately does not do
#
# Nothing here needs a device, a simulator, an emulator or Docker. The client UI suites that do,
# `clients/windows/uitests`, `scripts/dev/test-linux-ui.sh`, anything under `scripts/dev/boot.sh`
# stay out, because a gate that cannot run is a gate people stop running. `--clients` adds only
# the headless, host-appropriate ones, and says out loud which it skipped and why: a skip that
# looks like a pass is the failure mode this whole file exists to prevent.
#
# ## One thing it does after the steps
#
# A green gate drops the incremental build cache when it has grown past a cap. See
# `prune_incremental` at the bottom, and docs/debugging.md -> "Build time and disk" for the
# measurements. It runs here because this script is already the point where a chunk of work is
# finished and the next thing is a push; a cleanup anybody has to *remember* is the same failure
# mode as a gate run from memory.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

RUN_CLIENTS=0
KEEP_GOING=0
LIST_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --clients) RUN_CLIENTS=1 ;;
    --keep-going) KEEP_GOING=1 ;;
    --list) LIST_ONLY=1 ;;
    -h | --help)
      sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      printf 'gate.sh: unknown option %s (try --help)\n' "$arg" >&2
      exit 2
      ;;
  esac
done

bold=$'\033[1m'
red=$'\033[31m'
green=$'\033[32m'
yellow=$'\033[33m'
reset=$'\033[0m'
[ -t 1 ] || { bold=""; red=""; green=""; yellow=""; reset=""; }

results=()
failed=0

# run <name> <command...> is one gate step. Records the outcome and, unless --keep-going, stops at
# the first failure with the exact command to reproduce it.
run() {
  local name="$1"
  shift
  printf '%s==> %s%s\n' "$bold" "$name" "$reset"
  if "$@"; then
    results+=("${green}PASS${reset}  $name")
    return 0
  fi
  results+=("${red}FAIL${reset}  $name")
  failed=1
  printf '%s!! %s failed:%s %s\n' "$red" "$name" "$reset" "$*" >&2
  if [ "$KEEP_GOING" -eq 0 ]; then
    summary
    exit 1
  fi
}

# skip <name> <why> is a step **no install on this host can fix**: the wrong OS for a client, a
# file that is absent from this copy of the tree by design, work the caller did not ask for. Loud
# on purpose: a silent skip reads as a pass.
skip() {
  printf '%s== skipping %s: %s%s\n' "$yellow" "$1" "$2" "$reset"
  results+=("${yellow}SKIP${reset}  $1: $2")
}

# missing <name> <tool> <how> is a step whose tool is **installable right here**, and whose absence
# would otherwise take a check with it. That is a failure, not a skip.
#
# The distinction is the whole point: a skip says "this host cannot answer that question", and a
# reader is entitled to treat the rest of the run as complete. A missing `reuse` or `bun` says
# something else: a check CI *does* run has quietly stopped running for whoever builds, and
# they will find out from a red pipeline instead. Both tools are cross-platform and cheap, and
# each is the only thing watching what it watches.
missing() {
  local name="$1" tool="$2" how="$3"
  printf '%s!! %s needs %s, which is not installed:%s %s\n' "$red" "$name" "$tool" "$reset" "$how" >&2
  results+=("${red}NEED${reset}  $name: install $tool: $how")
  failed=1
  if [ "$KEEP_GOING" -eq 0 ]; then
    summary
    exit 1
  fi
}

summary() {
  printf '\n%s---- gate summary ----%s\n' "$bold" "$reset"
  for line in "${results[@]}"; do printf '%s\n' "$line"; done
}

if [ "$LIST_ONLY" -eq 1 ]; then
  cat <<'STEPS'
1  format          cargo +<rust-nightly.toml> fmt --all --check
2  file length     scripts/ci/check-file-length.sh
3  docs            cargo doc --workspace --exclude mailcal-linux --no-deps
4  version sync    scripts/ci/check-version-sync.sh
4b branding        scripts/ci/check-branding.sh
4c public hygiene  scripts/ci/check-public-hygiene.sh
4ca desktop handoff scripts/ci/check-desktop-handoff.sh: portal launchers only
4d licence dir     scripts/ci/check-license-dir.sh: the default build stands alone
4e reuse           reuse lint: required; the gate fails without it
4f public split    scripts/dev/public-split.sh: stages and verifies the public tree; absent there
5  store copy      scripts/ci/check_store_copy_length.py
6  user docs       scripts/ci/check_user_docs.py
7  showcase flag   scripts/ci/check-showcase-flag.sh
7b dev account     scripts/ci/check-dev-account.sh
8  log hygiene     scripts/ci/check_log_hygiene.py
8b british english scripts/ci/check_british_english.py
8c dash punctuation scripts/ci/check_dash_hygiene.py
9  composer labels scripts/ci/check_composer_labels.py
10 surface publish scripts/ci/check_surface_publish.py
11 script tests    unittest discover -s scripts/dev/tests and -s scripts/ci/tests
12 composer        typecheck + bun test + bun run check (clients/composer): requires bun
13 clippy          cargo clippy --workspace --exclude mailcal-linux --all-targets --all-features
14 tests           cargo test --workspace --exclude mailcal-linux
15 allodia tests   cargo test -p mailcal-bindings --features allodia-license
   --clients adds: Apple swift test + macOS/iOS app builds, Android :app:test, Windows dotnet test
STEPS
  exit 0
fi

# 1. Formatting, nightly, per the workspace rustfmt.toml (stable cannot read its options). The
#    date comes from rust-nightly.toml, the same file ci.yml's lint job reads, so this step and
#    CI's `fmt --check` are the same build of rustfmt. A floating `+nightly` is not: nightly
#    rustfmt output drifts between dates, so it goes green locally and red in CI.
nightly=$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' rust-nightly.toml)
[ -n "$nightly" ] || { printf 'gate.sh: no [toolchain] channel in rust-nightly.toml\n' >&2; exit 1; }

# `cargo +<pin>` auto-installs a missing toolchain with its DEFAULT components, without rustfmt, and
# then fails with "'cargo-fmt' is not installed", which reads like a bad pin rather than a missing
# component. So install it properly first. Probe with `rustup toolchain list`, which is the only
# one of these that stays local: `rustup component list --toolchain <pin>` installs it too.
if ! rustup toolchain list | grep -q "^${nightly}-" ||
  ! rustup component list --toolchain "$nightly" --installed 2>/dev/null | grep -q '^rustfmt'; then
  printf '%s==> installing rustfmt for %s (the pin in rust-nightly.toml)%s\n' "$bold" "$nightly" "$reset"
  rustup toolchain install "$nightly" --component rustfmt --profile minimal ||
    { printf 'gate.sh: could not install %s\n' "$nightly" >&2; exit 1; }
fi

run "format (nightly rustfmt)" cargo "+$nightly" fmt --all --check

# 2. The 500-line rule. Instant, no toolchain, and it sees untracked files only if you staged
#    them. Inspect new *.rs / *.cs yourself before calling a branch green.
run "file length (<= 500 lines)" bash scripts/ci/check-file-length.sh

# 3. Docs. Third, not last, as explained in the header. Its own unit graph means it never rides on the tests.
run "docs (rustdoc, warnings denied)" cargo doc --workspace --exclude mailcal-linux --no-deps

# 4-6. The three doc-driven checkers. All dependency-free and instant.
run "version sync (/VERSION)" bash scripts/ci/check-version-sync.sh
# The identity contract (docs/branding.md). Beside version sync because it is the same shape of
# rule and the same shape of failure: a literal written back into a manifest builds, installs and
# runs, and is simply no longer re-brandable. Nothing else in this list would say so.
run "branding (name + application id)" bash scripts/ci/check-branding.sh
# What the public repository may not carry (the split's content pass). Beside branding for the
# same reason: work continues in this tree until the switch, so a reference stripped once has to
# stay stripped, and only a grep run on every change can say that it has.
run "public hygiene (split content rules)" bash scripts/ci/check-public-hygiene.sh
# How the Linux client hands a URI or a file to the desktop. The same shape again: the banned call
# works perfectly in a --host build and freezes the packaged one, so the only thing that can say so
# before a release is a grep run on every change.
run "desktop handoff (portal launchers)" bash scripts/ci/check-desktop-handoff.sh
run "licence directory (default build stands alone)" bash scripts/ci/check-license-dir.sh
# Licensing, stated once in REUSE.toml and checked per file. It fires on vendoring: a file with
# its own SPDX header whose licence text is not in LICENSES/, and a text left there after what
# needed it is gone. Here as well as in CI because that arrives in a branch, and a check only the
# runners get is a check nobody who builds gets. `--lines` rather than `--quiet`: silent when
# compliant, and it names the file when it is not.
# `pipx install reuse` alone is not enough on a host without libmagic: it installs, and then every
# invocation dies with NoEncodingModuleError. The extra is what makes it portable.
if command -v reuse >/dev/null 2>&1; then
  run "reuse (every file licensed)" reuse lint --lines
else
  missing "reuse (every file licensed)" "reuse" "pipx install 'reuse[charset-normalizer]'"
fi
# The public tree, staged and verified. The check above says the private tree carries nothing it
# must not; this one says the copy that gets published is still a copy anyone can read -- nothing
# links into a submodule it does not have. That is the one defect only the pipeline can
# see, because in this tree those links resolve. Three seconds; the build inside it is `--gate`,
# and that is not this.
#
# Absent in the public tree, which is the tree it produces: the pipeline is one of its own
# exclusions, so a contributor cloning the public repository has this gate and not that script.
# Skipping is therefore the correct answer there and a missing file is not, which is exactly what
# `public-split.sh --gate` reported before this said so.
if [ -f scripts/dev/public-split.sh ]; then
  run "public split (staged tree)" bash scripts/dev/public-split.sh
else
  skip "public split" "scripts/dev/public-split.sh is not in this tree: it stages the public copy, and is excluded from it"
fi
run "store copy (field limits)" python3 scripts/ci/check_store_copy_length.py
# The user-doc contract (docs/user-docs.md): locale parity, nav reachability, screenshot ids that
# resolve for every platform a page claims, and an `updated_for` that cannot out-run /VERSION.
# Here rather than only in CI for the reason the two above are: the change most likely to break
# these pages is a docs-only one, and a docs-only change is exactly what turns every gated job off.
run "user docs (contract)" python3 scripts/ci/check_user_docs.py

# 7. The showcase-flag contract. It runs in CI's `lint` job, and it belongs here for the same
# reason the two above do: it is instant, dependency-free, and it guards a thing whose failure is
# invisible: a screenshot of the developer's real mailbox, or a capture path that quietly offers
# one screen or one language fewer than the others. It was also, until recently, dead on macOS
# (a GNU-only sed construct plus `set -e`), which is exactly the kind of thing running it locally
# surfaces and CI-only did not.
run "showcase flag contract" bash scripts/ci/check-showcase-flag.sh

# 7b. The same property for the dev-account switch: the harness fixture is compiled out of a
# release build, and the four clients' hand-written copies of it have not drifted apart. It was
# CI-only until a file split moved the fixture to a sibling and every local gate stayed green
# while the check that would have caught it never ran. A check nothing runs before pushing is one
# that reports at the worst moment.
run "dev account contract" bash scripts/ci/check-dev-account.sh

# 8. The log is product surface (docs/logging.md): a line describes the user's mail, never our
# source tree. This holds only the exact half, a repo path, a `.md`, an `#nnn` inside a logged
# string, because the jargon half needs judgement and a checker that guesses gets skipped. It
# earns its place by having caught two: an `error!` citing a design doc rule, and a Linux line
# logging a raw account id, which is an address.
run "log hygiene (no repo paths in log lines)" python3 scripts/ci/check_log_hygiene.py

# 8b. British English, in prose and comments (AGENTS.md -> "How we write"). The tree was swept once
# to satisfy that rule, which is the state a rule rots from: a hundred files agree, nobody remembers
# why, and the next `behavior` reads like precedent. Prose only -- fenced blocks, inline code spans
# and anything touching an identifier character are all invisible to it -- and the vocabulary that
# is somebody else's name (an RFC's, a toolkit's, a doc reference to a symbol) is listed in the
# checker rather than guessed at.
run "british english (prose and comments)" python3 scripts/ci/check_british_english.py
run "dash punctuation (prose and comments)" python3 scripts/ci/check_dash_hygiene.py

# 9. The editor's chrome is one shared bundle, so each client passes its own translations in. Every
# way of getting that wrong is silent: a client that never sends them keeps English, a
# key the bundle does not know is dropped, one it knows but a client omits keeps that control's
# default. Nothing throws and nothing logs; only this says so.
run "composer labels (every client sends every one)" python3 scripts/ci/check_composer_labels.py

# 10. A published surface may only be announced by publishing it. `Surfaced` closes the door in the
# type system, but `App` still owns an observer, so `self.observer.surface_changed(Surface::Reading)`
# would compile and silently reopen the ordering bug, and a stale paint is not a panic, so no test
# would catch it.
run "surface publish (no signal without a snapshot)" python3 scripts/ci/check_surface_publish.py

# 11. The script suites. `discover`, so a new helper's tests are picked up by existing.
run "script tests (dev + ci helpers)" bash -c \
  'python3 -m unittest discover -s scripts/dev/tests -q && python3 -m unittest discover -s scripts/ci/tests -q'

# 11. The shared rich-composer editor: its unit suite, and the check that the committed
# `editor.html` is what its TypeScript sources produce.
#
# That second half is the one that matters. `editor.html` is a build output we commit. Every host
# loads that single file, and generating it per build would make bun a prerequisite of cargo,
# MSBuild and Gradle. So the only thing standing between a source edit and a client shipping the
# previous bundle is this step, which is why AGENTS.md says never to skip it, and why a host
# without bun fails here rather than reporting a gate that quietly did not check the bundle.
if command -v bun >/dev/null 2>&1; then
  run "composer editor (typecheck + bun test + bundle freshness)" bash -c \
    'cd clients/composer && bun install --frozen-lockfile --silent && bun run typecheck && bun test && bun run check'
else
  missing "composer editor" "bun" "https://bun.sh: see clients/composer/package.json"
fi

# 12-13. The expensive pair, last, because everything above can fail in seconds instead.
run "clippy (pedantic, warnings denied)" \
  cargo clippy --workspace --exclude mailcal-linux --all-targets --all-features -- -D warnings
run "tests (workspace)" cargo test --workspace --exclude mailcal-linux
# 14. The same crate with the Allodia sign-in compiled in. `--all-features` above type-checks this
#     build, but clippy produces nothing runnable, and `cargo test --workspace` runs the *default*
#     feature set -- so without this step a test that only exists behind the feature is written,
#     passes locally when its author runs it by hand, and is then never run again.
run "tests (allodia sign-in compiled in)" \
  cargo test -p mailcal-bindings --features allodia-license

if [ "$RUN_CLIENTS" -eq 1 ]; then
  # Apple: the *app* build, not `swift build`. SwiftPM compiles the module as a whole, so a file
  # missing an import still builds when a sibling imports the same symbol, and only xcodebuild's
  # batched Debug compile catches it (AGENTS.md).
  #
  # The app build goes FIRST, and that order is load-bearing rather than taste: MailcalBindings is
  # generated and gitignored, so on a clean checkout, or any time the cdylib's shape changed,
  # `swift test` does not fail a test, it fails to resolve the package at all:
  #
  #     error: target 'MailcalBindings' referenced in product 'MailcalBindings' is empty
  #
  # `build-and-run.sh` regenerates them on its way through, so running it first makes the suite
  # test the bindings that match this working tree instead of whatever was there last.
  if [ "$(uname -s)" = "Darwin" ]; then
    run "apple (macOS app build + bindings)" ./clients/apple/Scripts/build-and-run.sh --macos --no-run
    run "apple (swift test)" bash -c 'cd clients/apple/Packages/MailcalKit && swift test'
    run "apple (iOS simulator build)" ./clients/apple/Scripts/build-and-run.sh --iphone --no-run
  else
    skip "apple" "needs macOS + Xcode"
  fi

  # Android: JDK 17 is pinned in the module (jvmToolchain), so this matches CI's locale data.
  #
  # Gated on the SDK as well as on the wrapper, and in the same order Gradle resolves it. Without
  # one, `:app:test` dies at configuration with "SDK location not found", a FAIL that says nothing
  # about the change under test, on a host that simply does not build Android. A machine without
  # the toolchain skips with a reason, exactly as a non-Mac skips Apple; a gate that is red for a
  # reason nobody can act on is one people stop reading.
  android_sdk_found() {
    if [ -f clients/android/local.properties ] &&
      grep -qE '^[[:space:]]*sdk\.dir[[:space:]]*=' clients/android/local.properties; then
      return 0
    fi
    for candidate in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}"; do
      if [ -n "$candidate" ] && [ -d "$candidate" ]; then return 0; fi
    done
    # The per-OS default the SDK manager installs into, which Gradle finds with nothing set.
    case "$(uname -s)" in
      Darwin) default_sdk="$HOME/Library/Android/sdk" ;;
      MINGW* | MSYS* | CYGWIN*) default_sdk="${LOCALAPPDATA:-$HOME/AppData/Local}/Android/Sdk" ;;
      *) default_sdk="$HOME/Android/Sdk" ;;
    esac
    [ -d "$default_sdk" ]
  }

  if [ ! -x clients/android/gradlew ]; then
    skip "android" "no clients/android/gradlew"
  elif ! android_sdk_found; then
    skip "android" "no Android SDK: set ANDROID_HOME, or sdk.dir in clients/android/local.properties"
  else
    run "android (:app:test)" bash -c 'cd clients/android && ./gradlew :app:test'
  fi

  # Windows: the pure net10.0 half runs anywhere dotnet does, including macOS. The WinUI app and
  # the UI Automation suite need a Windows desktop session and are NOT covered here.
  #
  # The two regeneration steps are not optional. `Mailcal.Tests` LINKS the generated C# bindings and
  # `L10n.cs`, and on Windows `build-and-run.ps1` regenerates both before it ever calls `dotnet
  # test`. Off Windows nothing does, so without these the suite compiles against whatever shape the
  # bindings had the last time somebody remembered, which after an FFI or catalog change is a green
  # run proving nothing (AGENTS.md: "the generated bindings are built, never committed", and a stale
  # one fails for reasons that look nothing like the cause).
  if command -v dotnet >/dev/null 2>&1; then
    case "$(uname -s)" in
      Darwin) cdylib="target/debug/libmailcal_bindings.dylib" ;;
      # Git Bash reports MINGW64_NT-*; the cdylib there is a plain .dll with no `lib` prefix.
      MINGW*|MSYS*|CYGWIN*) cdylib="target/debug/mailcal_bindings.dll" ;;
      *) cdylib="target/debug/libmailcal_bindings.so" ;;
    esac
    run "windows (regenerate C# bindings + l10n)" bash -c "
      set -e
      cargo build -p mailcal-bindings
      cargo run -q -p mailcal-bindgen-cs -- --library '$cdylib' --out-dir clients/windows/Generated
      cargo run -q -p mailcal-l10n -- generate --target winui --root . --out clients/windows/Mailcal"
    run "windows (Mailcal.Tests)" dotnet test clients/windows/Mailcal.Tests --nologo
    # MailcalVerify is the only thing off Windows that compiles `Verify.cs` against the freshly
    # generated bindings. Mailcal.Tests links the generated file too, but not this one. It is a
    # plain net10.0 console app, so the build costs seconds and catches an FFI-shape change here
    # instead of on the Windows box.
    run "windows (MailcalVerify compiles)" dotnet build clients/windows/MailcalVerify --nologo
    skip "windows (WinUI app + uitests)" "needs a Windows host: run build-and-run.ps1 and uitests/run-ui-tests.ps1 there"
  else
    skip "windows" "no dotnet on PATH"
  fi

  # Linux's GTK client is excluded from the workspace gate on every other host.
  if [ "$(uname -s)" = "Linux" ]; then
    run "linux (clippy)" cargo clippy -p mailcal-linux --all-targets --all-features -- -D warnings
    run "linux (tests)" xvfb-run --auto-servernum cargo test -p mailcal-linux --all-features
    run "linux (docs)" cargo doc -p mailcal-linux --no-deps
  else
    skip "linux (mailcal-linux)" "needs a Linux host with GTK 4.14+/libadwaita 1.5+"
  fi
else
  skip "clients" "not requested: pass --clients to build Apple / Android / Windows too"
fi

# The incremental build cache is pure cache, and nothing reclaims it. Cargo mints a session
# directory per distinct compilation context, every engine re-pin, every toggle of the
# local-engine `[patch]` override, every feature set, and keeps them all; `cargo clean` has no
# stale/age/size option, and the `cargo clean gc` that nightly offers cleans `$CARGO_HOME`, not
# `target/`. Two days of one branch's work left 20 GiB of it here.
#
# So: drop it once a chunk of work is done and only when it is over the cap, since the cost is a
# real one. Measured on an M-series Mac, `cargo build -p mailcal-bindings` after a one-line edit in
# `mailcal-app`: 3.3s with the cache, 12.0s on the first rebuild after a prune, 3.4s from then on.
# The cap is high enough that a normal branch never trips it.
INCREMENTAL_CAP_GIB=5

prune_incremental() {
  [ -d target ] || return 0
  local dirs kb gib
  # Both `target/debug/incremental` and the per-triple `target/<triple>/debug/incremental`.
  dirs=$(find target -maxdepth 3 -type d -name incremental 2>/dev/null)
  [ -n "$dirs" ] || return 0
  kb=$(printf '%s\n' "$dirs" | tr '\n' '\0' | xargs -0 du -sk 2>/dev/null |
    awk '{ total += $1 } END { print total + 0 }')
  gib=$((kb / 1024 / 1024))
  [ "$gib" -ge "$INCREMENTAL_CAP_GIB" ] || return 0
  printf '%s\n' "$dirs" | tr '\n' '\0' | xargs -0 rm -rf
  printf '\n%s== reclaimed %s GiB of incremental build cache%s (past the %s GiB cap; your next\n' \
    "$yellow" "$gib" "$reset" "$INCREMENTAL_CAP_GIB"
  printf '   rebuild after an edit costs about nine seconds more, once. See docs/debugging.md)\n'
}

summary
if [ "$failed" -ne 0 ]; then
  # Left warm on purpose: a red gate means you are still iterating, and that is exactly when the
  # cache is worth its disk.
  printf '\n%sThe gate is RED.%s Fix the above before pushing. CI is not the place to find this.\n' \
    "$red" "$reset"
  exit 1
fi
prune_incremental
printf '\n%sThe gate is GREEN.%s\n' "$green" "$reset"
