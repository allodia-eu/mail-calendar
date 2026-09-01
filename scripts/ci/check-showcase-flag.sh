#!/usr/bin/env bash
# Guard the showcase (store-screenshot) flag contract. Runs in the always-run `checks` CI job
# and locally:
#
#     scripts/ci/check-showcase-flag.sh
#
# Showcase mode's whole safety property is "the app on screen is showing fictional mail, never the
# developer's". Nothing about a screenshot reveals whether that held, so the ways it silently breaks
# are worth machine-checking. Each assertion below is a bug that actually shipped or was one edit
# away from shipping.
#
# `git grep` searches tracked files only, so it never descends into a submodule, `target/`, or the
# gitignored generated bindings.
set -euo pipefail

SELF='scripts/ci/check-showcase-flag.sh'
fail=0

# 1. The retired name is gone.
#
# The flag was once ALLODIA_DEMO. It was renamed to MAILCAL_SHOWCASE across every client, but
# clients/windows/build-and-run.ps1 kept reading the old name; so the banner that warns "this
# launch shows the showcase dataset, NOT your accounts" tested a variable nothing sets and could
# never fire. The launcher stayed silent exactly when it had the most to say.
retired='ALLODIA_DEMO'
if hits=$(git grep -lI --fixed-strings "$retired" -- ":(exclude)$SELF" 2>/dev/null) && [ -n "$hits" ]; then
  printf 'ERROR: the retired showcase flag %s is still referenced:\n' "$retired" >&2
  printf '  %s\n' $hits >&2
  printf 'The flag is MAILCAL_SHOWCASE. A stale name here reads as a working guard that never fires.\n' >&2
  fail=1
fi

# 2. The Windows launcher's mailbox banner reads the flag the app actually reads.
#
# build-and-run.ps1 names the mailbox it is about to open. If it parses a different variable than
# Services/ShowcaseMode.cs, it will cheerfully announce "your stored accounts" while opening the
# showcase dataset, or; far worse; stay silent while opening the real one.
for file in clients/windows/build-and-run.ps1 clients/windows/Mailcal/Services/ShowcaseMode.cs; do
  if ! git grep -qI --fixed-strings 'MAILCAL_SHOWCASE' -- "$file"; then
    printf 'ERROR: %s no longer references MAILCAL_SHOWCASE.\n' "$file" >&2
    printf 'The launcher banner and the app must agree on the flag, or the banner lies.\n' >&2
    fail=1
  fi
done

# 3. Windows showcase mode stays compiled out of a release build.
#
# The Apple client hard-falses `isOn` behind `#if DEBUG`; Android gates on FLAG_DEBUGGABLE. Windows
# must too, or a shipped binary honours a stray MAILCAL_SHOWCASE in the user's environment and
# silently replaces their mailbox with fictional mail.
if ! git grep -qI --fixed-strings '#if DEBUG' -- clients/windows/Mailcal/Services/ShowcaseMode.cs; then
  printf 'ERROR: clients/windows/Mailcal/Services/ShowcaseMode.cs has no `#if DEBUG` guard.\n' >&2
  printf 'ShowcaseMode.IsOn must be hard-false in Release, as on Apple (#if DEBUG) and Android\n' >&2
  printf '(FLAG_DEBUGGABLE): otherwise a shipped build honours MAILCAL_SHOWCASE.\n' >&2
  fail=1
fi

# 3b. The same property on Linux; the one platform that ships a binary a user installs from a
# packaged bundle rather than sideloads. The explicit, non-default `dev-harness` feature also keeps
# showcase available to an optimised renderer measurement; the Flatpak must build default features
# so its release binary still has no showcase path to reach. Without both halves, either the
# measurement silently times a debug build or an installed app can honour a stray environment flag.
linux_showcase='clients/linux/src/showcase.rs'
linux_guard='#![cfg(any(debug_assertions, feature = "dev-harness"))]'
if ! git grep --untracked -qI --fixed-strings "$linux_guard" -- "$linux_showcase"; then
  printf 'ERROR: %s has no file-level `%s` guard.\n' "$linux_showcase" "$linux_guard" >&2
  printf 'Showcase may exist only in debug or an explicit dev-harness build.\n' >&2
  fail=1
fi
linux_cargo='clients/linux/Cargo.toml'
if sed -n '/^\[features\]/,/^\[/p' "$linux_cargo" |
  grep -Eq '^[[:space:]]*default[[:space:]]*=.*dev-harness'; then
  printf 'ERROR: %s enables dev-harness by default.\n' "$linux_cargo" >&2
  printf 'The Flatpak builds default features, so dev-harness must remain opt-in.\n' >&2
  fail=1
fi
# The committed manifest is the NEUTRAL one and is named after the neutral app id; package.sh
# derives the branded copy beside it at package time and removes it again (docs/branding.md).
# Read that id straight from branding/default.env, the way check-branding.sh does: this checks
# what is COMMITTED, so a checkout that has a brand must not send it looking for a name only a
# package run ever writes.
neutral_id="$(sed -n 's/^MAILCAL_APP_ID="\{0,1\}\([^"]*\)"\{0,1\}$/\1/p' branding/default.env | tail -n1)"
linux_manifest="clients/linux/flatpak/$neutral_id.yml"
if [ ! -f "$linux_manifest" ]; then
  printf 'ERROR: %s does not exist.\n' "$linux_manifest" >&2
  printf 'The Flatpak manifest is named after MAILCAL_APP_ID in branding/default.env.\n' >&2
  fail=1
elif ! grep -Eq \
  '^[[:space:]]*-[[:space:]]*cargo build --release --locked -p mailcal-linux[[:space:]]*$' \
  "$linux_manifest"; then
  printf 'ERROR: %s no longer builds the exact default-feature Linux release.\n' "$linux_manifest" >&2
  printf 'The shipping command must not enable dev-harness or any showcase path.\n' >&2
  fail=1
fi

# 3c. Every store screen scripts/dev/showcase.sh offers Linux is one its client can actually reach.
#
# Assertion 7 makes this argument for the documentation set. It applies here too, and only here,
# because Linux is the platform whose store list has had to be spelled out separately; it reached
# the whole set only once it drew the invitation card, and `store_screens_for` still keeps its arm
# so this check has something to read. Adding a name to that list without a surface behind it
# breaks every full capture run; the client refuses the name and exits 2, which is loud, but it is
# loud in the middle of a 35-shot run rather than here.
linux_store_screens=$(
  sed -n '/^store_screens_for()/,/^}/ s/^[[:space:]]*linux)[[:space:]]*printf[^ ]*[^ ]* \(.*\);;$/\1/p' \
    scripts/dev/showcase.sh | tr -d "'\"" | tr ' ' '\n' | grep -E '^[a-z][a-z-]+$' | sort -u || true
)
if [ -z "$linux_store_screens" ]; then
  printf 'ERROR: parsed no Linux store screens out of scripts/dev/showcase.sh: this check is blind.\n' >&2
  printf 'Expected a `linux)` arm in store_screens_for.\n' >&2
  fail=1
fi
# The names `parse_screen` ACCEPTS; read off the arms that return `Ok`, not grepped for anywhere
# in the file. Grepping the file is the version of this check that cannot fail: showcase.rs's own
# unit test asserts `parse_screen(Some("invitation"))` is *refused*, so the literal is there, and a
# whole-file search happily reports that Linux can reach a screen it explicitly rejects.
linux_client_screens=$(
  sed -n '/fn parse_screen/,/^}/p' "$linux_showcase" |
    grep 'Ok(ShowcaseScreen::' | grep -o '"[a-z-]*"' | tr -d '"' |
    grep -E '^[a-z][a-z-]+$' | sort -u || true
)
if [ -z "$linux_client_screens" ]; then
  printf 'ERROR: parsed no accepted screens out of %s: this check is blind.\n' "$linux_showcase" >&2
  printf 'Expected `Some("<name>") => Ok(ShowcaseScreen::…)` arms in parse_screen.\n' >&2
  fail=1
fi
for screen in $linux_store_screens; do
  # shellcheck disable=SC2076  # a literal match on the padded list, not a regex
  if ! [[ " $(printf '%s ' $linux_client_screens)" =~ " $screen " ]]; then
    printf 'ERROR: %s does not accept the store screen "%s".\n' "$linux_showcase" "$screen" >&2
    printf 'store_screens_for offers it to Linux, but parse_screen returns Err for it: so the\n' >&2
    printf 'client exits 2 and the capture run dies partway through a 35-shot set.\n' >&2
    printf '  offered : %s\n' "$(printf '%s ' $linux_store_screens)" >&2
    printf '  accepted: %s\n' "$(printf '%s ' $linux_client_screens)" >&2
    fail=1
  fi
done

# 4. The showcase log marker still reads the same in all three languages that speak it.
#
# Every capture run proves the app is on the fictional dataset by finding this line, which the core
# logs from inside build_showcase. Rust emits it; bash (scripts/dev/lib.sh) and PowerShell
# (clients/windows/showcase.ps1) match it. Reword the Rust and the matchers go blind; every capture
# would abort, which is at least loud, but silently "fixing" one matcher and not the other would
# leave a platform asserting nothing at all. Pin all three together.
#
# Only the stable prefix is compared: the Rust format string interpolates `{locale:?}` after it.
#
# The Rust side is searched as a DIRECTORY, not as boot.rs. It used to name that file, and when
# `boot.rs` was split (the 500-line rule) the marker moved to `boot/inmemory.rs` and this check
# began failing on every run; a guard that cannot pass is as useless as one that cannot fail, and
# a permanently-red gate is one people learn to skip. The directory keeps it honest across the next
# split: what matters is that the *core* still emits the line, not which file holds it.
marker='showcase (screenshot) app starting (in-memory engine, seeded'
for file in \
  crates/mailcal-bindings/src \
  scripts/dev/lib.sh \
  clients/windows/showcase.ps1; do
  if ! git grep -qI --fixed-strings "$marker" -- "$file"; then
    printf 'ERROR: %s no longer carries the showcase log marker:\n  %s\n' "$file" "$marker" >&2
    printf 'The core emits this line and every capture path matches it to prove the app is on the\n' >&2
    printf 'fictional dataset. All three copies must agree, or a platform stops checking.\n' >&2
    fail=1
  fi
done

# 5. Every capture path offers the same locales.
#
# Assertion 4 above pins the marker's stable PREFIX, which is identical in all three copies; so it
# stayed green while clients/windows/showcase.ps1 still capped `-Locale` at en|nl and resolved the
# capitalized variant with `if ($Locale -eq 'nl') { 'Nl' } else { 'En' }`. Five languages were added
# to the catalog, showcase.sh and lib.sh grew to seven, Windows did not, and nothing said so: the
# README claims "5 screens x 7 languages" for Windows, and `showcase.sh windows --locale de` died on
# a parameter-binding error. A check that pins the part which cannot drift, while the part that does
# drift goes unwatched, is not a check.
#
# So compare the LISTS, not the sentence around them. Windows is the one that fell behind, and it is
# the one no non-Windows developer can run; which is precisely why it needs a gate that runs
# everywhere.
locales_from() { # <file> <sed-extract-script>
  sed -n "$2" "$1" | tr -d "'\"," | tr ' |' '\n\n' | grep -E '^[a-z]{2}$' | sort -u | tr '\n' ' '
}
sh_locales=$(locales_from scripts/dev/showcase.sh 's/^ALL_LOCALES=(\(.*\))$/\1/p')
# A range followed by ONE command, not a `{ … }` block: BSD sed (every macOS host, where this is
# run before pushing) rejects a brace group written on one line; `bad flag in substitute command`
#; and `set -e` then killed the script at this assignment, *before* any of the comparisons below.
# So on a Mac this whole checker exited 1 on every run and proved nothing; on GNU sed it passed.
# That is the failure this file keeps warning about, in the file itself.
lib_locales=$(locales_from scripts/dev/lib.sh '/^showcase_marker_for()/,/^}/ s/^[[:space:]]*\([a-z |]*\))$/\1/p')
ps_locales=$(locales_from clients/windows/showcase.ps1 's/.*ValidateSet(\([^)]*\)).*\$Locale.*/\1/p')

for pair in "scripts/dev/lib.sh:$lib_locales" "clients/windows/showcase.ps1:$ps_locales"; do
  other_file=${pair%%:*}
  other_locales=${pair#*:}
  if [ "$other_locales" != "$sh_locales" ]; then
    printf 'ERROR: %s offers different showcase locales than scripts/dev/showcase.sh:\n' "$other_file" >&2
    printf '  scripts/dev/showcase.sh : %s\n' "$sh_locales" >&2
    printf '  %-24s: %s\n' "$other_file" "$other_locales" >&2
    printf 'Every capture path must offer the same languages, or a locale silently has no Windows\n' >&2
    printf 'screenshots while the README says it does. Adding a language means editing all three.\n' >&2
    fail=1
  fi
done

# A parse that finds nothing would compare "" to "" and pass; the same failure mode as above.
if [ -z "${sh_locales// /}" ]; then
  printf 'ERROR: parsed no locales out of scripts/dev/showcase.sh (ALL_LOCALES): this check is blind.\n' >&2
  fail=1
fi

# 6. Every capture path offers the same SCREENS.
#
# Exactly assertion 5's argument, applied to the other list that drifts. A MAILCAL_SHOWCASE_SCREEN
# name is a cross-client contract; the same string reaches the Apple, Android and Windows drivers
#; and the two capture paths keep it twice: `showcase.sh` in ALL_SCREENS + EXTRA_SCREENS, and
# `showcase.ps1` in the -Screen ValidateSet. Adding a screen to one and not the other does not
# fail loudly: `showcase.sh windows --screen <new>` dies on a PowerShell parameter-binding error
# that reads like a broken script rather than a missing list entry, and a `--screen all` run on a
# Windows host silently captures one screen fewer than every other platform.
#
# The client enums are deliberately NOT compared here: they are three languages' worth of syntax,
# and each already has a test that pins the literal spellings (Android's ShowcaseScreenTest is the
# one that runs on every PR). What no test could see is the two *scripts* disagreeing.
screens_from() { # <file> <sed-extract-script>
  sed -n "$2" "$1" | tr -d "'\"," | tr ' |' '\n\n' | grep -E '^[a-z][a-z-]+$' | sort -u | tr '\n' ' '
}
sh_screens=$(
  {
    screens_from scripts/dev/showcase.sh 's/^ALL_SCREENS=(\(.*\))$/\1/p'
    screens_from scripts/dev/showcase.sh 's/^EXTRA_SCREENS=(\(.*\))$/\1/p'
  } | tr ' ' '\n' | grep -E '.' | sort -u | tr '\n' ' '
)
ps_screens=$(screens_from clients/windows/showcase.ps1 's/.*ValidateSet(\([^)]*\)).*\$Screen.*/\1/p')

if [ "$sh_screens" != "$ps_screens" ]; then
  printf 'ERROR: clients/windows/showcase.ps1 offers different showcase screens than scripts/dev/showcase.sh:\n' >&2
  printf '  scripts/dev/showcase.sh : %s\n' "$sh_screens" >&2
  printf '  clients/windows/showcase.ps1: %s\n' "$ps_screens" >&2
  printf 'Adding a screen means editing ALL_SCREENS (or EXTRA_SCREENS) and the -Screen ValidateSet.\n' >&2
  fail=1
fi

# And, as above, a parse that found nothing would compare "" to "" and pass.
if [ -z "${sh_screens// /}" ]; then
  printf 'ERROR: parsed no screens out of scripts/dev/showcase.sh (ALL_SCREENS): this check is blind.\n' >&2
  fail=1
fi

# 7. Every DOCUMENTATION screen a platform is offered is one its client can actually drive to.
#
# Assertion 6 deliberately leaves the client enums alone, because each client has a test pinning its
# own spellings. That reasoning does not carry over to the doc set (docs/user-docs.md), because the
# doc lists are *per platform*: `doc_screens_for` promises Android the four `setup-*` screens, and
# nothing else states that Android's ShowcaseScreen has them.
#
# The gap matters more here than anywhere else in this file, because the failure is invisible at
# every later stage. A client that does not recognise a screen name does not error; it falls back
# to the mailbox list. So the run would enter showcase mode (`require_showcase_launch` passes), shoot
# a clean, well-lit, non-black frame of the *inbox* (the byte floor passes), and file it under
# `setup-detected`. The manifest would hash it, the page would embed it, and the first thing to
# notice would be a reader following a setup guide illustrated with a picture of an inbox.
doc_screens_from() { # <variable name>
  sed -n "s/^$1=(\(.*\))\$/\1/p" scripts/dev/showcase.sh | tr -d "'\"," | tr ' ' '\n' |
    grep -E '^[a-z][a-z-]+$' | sort -u
}
doc_shared=$(doc_screens_from DOC_SCREENS_SHARED)
doc_macos=$(doc_screens_from DOC_SCREENS_MACOS_ONLY)

if [ -z "$doc_shared" ] || [ -z "$doc_macos" ]; then
  printf 'ERROR: parsed no documentation screens out of scripts/dev/showcase.sh: this check is blind.\n' >&2
  printf 'Expected DOC_SCREENS_SHARED and DOC_SCREENS_MACOS_ONLY (see docs/user-docs.md).\n' >&2
  fail=1
fi

# The client files that must carry each list. Every platform `doc_screens_for` does not special-case
# gets DOC_SCREENS_SHARED, so a new client is added here at the same time it is added there.
check_doc_screens() { # <file> <screens...>
  local file=$1 screen
  shift
  for screen in $@; do
    if ! git grep -qI --fixed-strings "\"$screen\"" -- "$file"; then
      printf 'ERROR: %s cannot drive to the documentation screen "%s".\n' "$file" "$screen" >&2
      printf 'scripts/dev/showcase.sh offers it to this platform, but the client does not know the\n' >&2
      printf 'name: so the run would fall back to the mailbox list and file a photograph of the\n' >&2
      printf 'INBOX under that screenshot id. Nothing downstream can tell the difference.\n' >&2
      fail=1
    fi
  done
}
apple_showcase='clients/apple/Packages/MailcalKit/Sources/MailcalUI/ShowcaseMode.swift'
android_showcase='clients/android/app/src/main/java/eu/allodia/mailcal/ShowcaseMode.kt'
check_doc_screens "$apple_showcase" "$doc_shared" "$doc_macos"
check_doc_screens "$android_showcase" "$doc_shared"

# 8. Both capture paths offer the same APPEARANCES.
#
# Assertions 5 and 6 again, for the third list the two scripts keep twice. The store set is shot in
# a named appearance now; light for every screen, plus a dark capture of the mailbox list; and
# `showcase.sh` passes the word straight through to `showcase.ps1 -Appearance`, whose ValidateSet
# would reject a spelling it does not carry. On Windows that reads as a PowerShell binding error
# mid-run; on every other platform a word no client parses is *ignored*, and the capture comes out
# in whatever the machine was set to.
appearances_from() { # <file> <sed-extract-script>
  sed -n "$2" "$1" | tr -d "'\"," | tr ' |' '\n\n' | grep -E '^[a-z]+$' | sort -u | tr '\n' ' '
}
# Every appearance word `appearances_for` can print, read out of its body with its comments
# stripped; a comment naming a theme the function does not shoot would only make the containment
# below stricter, but it would also make this list a lie, so it is not read.
sh_appearances=$(
  awk '/^appearances_for\(\) \{/ { inside = 1 } inside { print } inside && /^\}$/ { exit }' \
    scripts/dev/showcase.sh |
    grep -v '^[[:space:]]*#' |
    # The words are separated by a literal `\n` inside a printf, so the two characters have to go
    # before the words can be read: dropping only the backslash leaves `ndark`, which matches
    # nothing and takes `dark` out of the list silently; the check would then pass over the very
    # word it was added for. (Caught by running it: it reported `light` alone.)
    sed 's/\\n/ /g' | grep -oE '\b(system|light|dark)\b' | sort -u | tr '\n' ' '
)
ps_appearances=$(appearances_from clients/windows/showcase.ps1 \
  's/.*ValidateSet(\([^)]*\)).*\$Appearance.*/\1/p')

# `showcase.sh` names only the two it actually shoots; the Windows ValidateSet also carries
# `system`, which is a legitimate hand-run ("shoot it however this desktop is set"). So the rule is
# containment, not equality; every word the capture loop can pass must be one the driver accepts.
for appearance in $sh_appearances; do
  case " $ps_appearances " in
    *" $appearance "*) ;;
    *)
      printf 'ERROR: scripts/dev/showcase.sh captures in appearance "%s", which\n' "$appearance" >&2
      printf 'clients/windows/showcase.ps1 -Appearance does not accept (%s).\n' "$ps_appearances" >&2
      printf 'A Windows run would die on a parameter-binding error mid-capture.\n' >&2
      fail=1
      ;;
  esac
done

if [ -z "${sh_appearances// /}" ] || [ -z "${ps_appearances// /}" ]; then
  printf 'ERROR: parsed no appearances out of the capture scripts: this check is blind.\n' >&2
  fail=1
fi

# 9. No Android surface picks its light/dark colours from the DEVICE.
#
# `AppTheme` resolves the app's Appearance setting once and publishes it as `LocalAppDark`; every
# composable that owns a light/dark pair; the calendar swatches, the month chips, the invitation
# preview, the composer's system bars; reads that. A direct `isSystemInDarkTheme()` asks what the
# *phone* is set to instead, which is exactly the thing an app-level Light or Dark overrides.
#
# It is guarded here, in the showcase file, because the showcase is where it does visible damage: a
# capture pinned to `dark` on a light emulator paints the app dark and then colours those swatches
# for a light device. The frame is the right screen, in the right language, at the right pixel size,
# and it ships to a store looking like a rendering bug. Nine such call sites existed before the
# setting landed; the ninth arrived by way of a merge, in a file no conflict marked.
theme_kt='clients/android/app/src/main/java/eu/allodia/mailcal/Theme.kt'
if strays=$(git grep -lI --fixed-strings 'isSystemInDarkTheme' \
  -- 'clients/android/app/src/main' ":(exclude)$theme_kt" 2>/dev/null) && [ -n "$strays" ]; then
  printf 'ERROR: these Android sources read the DEVICE theme directly:\n' >&2
  printf '  %s\n' $strays >&2
  printf 'Read LocalAppDark instead (Theme.kt). isSystemInDarkTheme() ignores the app-level\n' >&2
  printf 'Appearance setting, so a showcase capture pinned to dark would colour these for a\n' >&2
  printf 'light device: a store screenshot that looks like a rendering bug.\n' >&2
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  exit 1
fi

printf 'OK: the showcase flag contract holds (no retired name, launcher agrees, Release-gated,\n'
printf '    marker in step, all three capture paths offer: %s\n' "$sh_locales"
printf '    and both offer the screens: %s).\n' "$sh_screens"
printf '    and the appearances: %s(driver accepts: %s).\n' "$sh_appearances" "$ps_appearances"
printf '    Documentation screens reach their drivers: %s| macOS also %s\n' \
  "$(printf '%s ' $doc_shared)" "$(printf '%s ' $doc_macos)"
