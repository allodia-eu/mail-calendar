#!/usr/bin/env bash
# Build the Linux client as a Flatpak. The twin of clients/apple/Scripts/package.sh and
# clients/windows/package.ps1: one command from a checkout to the artifact a user installs.
#
#   clients/linux/package.sh                 # build only
#   clients/linux/package.sh --install       # build, then install it for this user
#   clients/linux/package.sh --install --run # …and launch it
#   clients/linux/package.sh --bundle        # also write a single-file target/flatpak/mailcal.flatpak
#
# Everything about the app's identity, copy and version is decided elsewhere and generated during
# the build (see flatpak/org.mailcal.client.yml); nothing here is retyped from a document.
#
# The runtime is a ~2 GB download the first time. It is NOT installed automatically: on a
# developer's machine that is a surprise worth asking for, and CI installs it explicitly in its own
# step. A missing runtime fails with flatpak's own message, which names exactly what to install.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"          # clients/linux
ROOT="$(cd "$HERE/../.." && pwd)"              # repo root (worktree)
# The committed manifest is the UNBRANDED one (docs/branding.md); a brand, when the checkout has
# one, produces a copy of it beside itself. `brand.sh` resolves MAILCAL_APP_ID either way.
# shellcheck source=scripts/dev/brand.sh
. "$ROOT/scripts/dev/brand.sh"
brand_load
# `core_cargo_features`: the one place that turns the injected registration into a cargo feature,
# shared with every other build front door (BUILDING.md).
# shellcheck source=scripts/dev/lib.sh
. "$ROOT/scripts/dev/lib.sh"

MANIFEST="$HERE/flatpak/org.mailcal.client.yml"
NEUTRAL_APP_ID="org.mailcal.client"
APP_ID="$MAILCAL_APP_ID"
OUT="$ROOT/target/flatpak"

INSTALL=0
RUN=0
BUNDLE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --install) INSTALL=1 ;;
    --run) RUN=1; INSTALL=1 ;;
    --bundle) BUNDLE=1 ;;
    -h | --help) sed -n '2,15p' "$0"; exit 0 ;;
    *) echo "package.sh: unknown option '$1'" >&2; exit 2 ;;
  esac
  shift
done

command -v flatpak-builder >/dev/null ||
  { echo "package.sh: need flatpak-builder (sudo apt install flatpak flatpak-builder)" >&2; exit 1; }

# The OAuth client registrations have to cross into the sandbox, and only the source tree does.
#
# flatpak-builder runs `cargo build` inside `flatpak build`, which forwards **no** host environment
#: the manifest's `build-options.env` is the whole of it, and that is static YAML no secret may be
# written into. What does cross is the `type: dir` source: flatpak-builder copies this checkout, and
# `.env` is not in its `skip:` list. So the file is the way in, and on a developer's machine it is
# already there and already copied, with nothing to do.
#
# CI is the case that needs help: the values arrive as environment variables from repository
# secrets and there is no `.env` on the runner, so without the block below a tagged build would
# produce the artifact users install with Google and Microsoft sign-in silently missing; and
# nothing downstream could tell, because a credential-free build is correct in every other respect.
# It is written only when absent, removed on the way out however this exits, and never readable by
# another user.
MAILCAL_ENV_FILE="$ROOT/.env"
WROTE_ENV_FILE=0
MANIFEST_WITH_REQUIREMENT=""
MANIFEST_WITH_FEATURES=""
BRANDED_MANIFEST=""
cleanup_generated() {
  if [[ "$WROTE_ENV_FILE" == "1" ]]; then
    rm -f "$MAILCAL_ENV_FILE"
  fi
  if [[ -n "$MANIFEST_WITH_REQUIREMENT" ]]; then
    rm -f "$MANIFEST_WITH_REQUIREMENT"
  fi
  if [[ -n "$MANIFEST_WITH_FEATURES" ]]; then
    rm -f "$MANIFEST_WITH_FEATURES"
  fi
  if [[ -n "$BRANDED_MANIFEST" ]]; then
    rm -f "$BRANDED_MANIFEST"
  fi
}
trap cleanup_generated EXIT

# The app id decides the basename of the desktop entry, the icon and the metainfo, and the shell
# ties a window to its launcher by that match; so a branded build cannot use the committed
# manifest as it stands. The copy is a sibling of the original, never under target/: the manifest's
# `sources: path: ../../..` resolves against its own directory.
#
# `flatpak_metadata.py` runs INSIDE the sandbox and reads the brand files out of the copied tree,
# so the names it writes and the names installed here come from one place either way.
if [[ "$APP_ID" != "$NEUTRAL_APP_ID" ]]; then
  BRANDED_MANIFEST="$(dirname "$MANIFEST")/$APP_ID.yml"
  sed "s/$NEUTRAL_APP_ID/$APP_ID/g" "$MANIFEST" > "$BRANDED_MANIFEST"
  grep -q "^app-id: $APP_ID\$" "$BRANDED_MANIFEST" || {
    echo "package.sh: could not put the app id into $MANIFEST: no 'app-id:' line" >&2
    exit 1
  }
  MANIFEST="$BRANDED_MANIFEST"
  echo "==> branding: building as $APP_ID ($MAILCAL_APP_NAME)"
fi

CREDENTIAL_VARS=(
  MAILCAL_GOOGLE_DESKTOP_CLIENT_ID
  MAILCAL_GOOGLE_DESKTOP_CLIENT_SECRET
  MAILCAL_GOOGLE_IOS_CLIENT_ID
  MAILCAL_GOOGLE_ANDROID_CLIENT_ID
  MAILCAL_MS_CLIENT_ID
  MAILCAL_ALLODIA_CLIENT_ID
  MAILCAL_ALLODIA_HOST
  # The consented-analytics relay (docs/analytics.md). Injected the same way and for the same
  # reason: the build inside the sandbox reads only this file, so a name absent here is a bundle
  # that asks for consent and then has nowhere to report.
  ALLODIA_TELEMETRY_URL
  ALLODIA_TELEMETRY_APP_KEY
  MAILCAL_REQUIRE_INJECTED_CONFIG
)
FROM_ENVIRONMENT=()
for var in "${CREDENTIAL_VARS[@]}"; do
  if [[ -n "${!var:-}" ]]; then
    FROM_ENVIRONMENT+=("$var")
  fi
done

if [[ -f "$MAILCAL_ENV_FILE" ]]; then
  echo "==> credentials: using the existing $MAILCAL_ENV_FILE (copied into the sandbox with the tree)"
  # A variable exported for this one build cannot win inside the sandbox, because the build script
  # in there sees the file and no environment at all. Say so rather than letting it be ignored.
  if (( ${#FROM_ENVIRONMENT[@]} )); then
    echo "    note: ${FROM_ENVIRONMENT[*]} set in the environment will be IGNORED: the sandbox" >&2
    echo "          reads only the file. Edit it, or move it aside, to change what this build gets." >&2
  fi
elif (( ${#FROM_ENVIRONMENT[@]} )); then
  echo "==> credentials: writing a temporary $MAILCAL_ENV_FILE so the sandbox can read them"
  ( umask 077; : > "$MAILCAL_ENV_FILE" )
  WROTE_ENV_FILE=1
  for var in "${FROM_ENVIRONMENT[@]}"; do
    printf '%s=%s\n' "$var" "${!var}" >> "$MAILCAL_ENV_FILE"
  done
else
  echo "==> credentials: none given: this bundle will not offer Google or Microsoft sign-in,"
  echo "    and will report no analytics even where a user opts in."
  echo "    That is a supported build (BUILDING.md); set MAILCAL_REQUIRE_INJECTED_CONFIG=1 to"
  echo "    make it a hard error instead, which is what a release build does."
fi

# `--disable-rofiles-fuse`: the build tree lives under target/, and rofiles-fuse cannot mount inside
# a container or on some overlay filesystems; including GitHub's runners. It is a build-time
# optimisation, not a correctness measure.
#
# The state dir is kept under target/ so `cargo clean` and .gitignore already cover it, and so a
# checkout never grows a second untracked directory nobody recognises.
# The Allodia sign-in has to be asked for on the cargo line, because cargo reads features from
# nowhere else: no environment variable, and nothing the manifest's `build-options.env` can carry.
#
# So unlike the credentials, which cross in the tree, this crosses in the manifest: the decision is
# made out here from `core_cargo_features` (the registration in the environment or in `.env`) and
# written into the copy the build runs. A checkout without the registration rewrites nothing and
# the bundle has no sign-in, which is the ordinary build from source.
#
# This is what closes the gap the entitlement contract used to record: for a while the packaged
# bundle deliberately stayed on default features, so a released Linux client had no sign-in even
# where the registration was present.
ALLODIA_FEATURE="$(core_cargo_features)"
if [[ -n "$ALLODIA_FEATURE" ]]; then
  MANIFEST_WITH_FEATURES="$(dirname "$MANIFEST")/.package-sh-with-features.yml"
  # Only the client's own build line: the MCP shim is a separate package that has no such feature,
  # and `-p mailcal-linux` is what tells the two apart.
  sed "s|^\([[:space:]]*- cargo build --release --locked -p mailcal-linux\)\$|\1 --features $ALLODIA_FEATURE|" \
    "$MANIFEST" > "$MANIFEST_WITH_FEATURES"
  # A manifest whose cargo line moved would leave this a silent no-op, and the bundle would ship
  # without the sign-in behind a green build; the same failure the requirement below guards.
  grep -q -- "-p mailcal-linux --features $ALLODIA_FEATURE\$" "$MANIFEST_WITH_FEATURES" || {
    echo "package.sh: could not put --features $ALLODIA_FEATURE on the cargo line in $MANIFEST" >&2
    exit 1
  }
  MANIFEST="$MANIFEST_WITH_FEATURES"
  echo "==> credentials: this bundle carries the Allodia sign-in (--features $ALLODIA_FEATURE)"
fi

# The requirement deliberately travels by a different road than the credentials it guards.
#
# Both would otherwise ride in `.env`, and then a `.env` that failed to cross into the sandbox would
# take the requirement with it: the build script inside would see nothing to enforce, and the bundle
# would ship without Google or Microsoft sign-in behind a green build; the guard unable to fire
# precisely when it was needed. The flag is not a secret, so it can go in the manifest, where
# nothing can drop it silently; a `.env` that then does not arrive is a compile error by name.
if [[ -n "${MAILCAL_REQUIRE_INJECTED_CONFIG:-}" ]]; then
  # Beside the original, never under target/: `sources: path: ../../..` in the manifest resolves
  # relative to the manifest's own directory, so a copy anywhere else would point at the wrong tree.
  MANIFEST_WITH_REQUIREMENT="$(dirname "$MANIFEST")/.package-sh-with-requirement.yml"
  # Inserted as a sibling of CARGO_HOME, at its indentation, so the block stays valid YAML wherever
  # it sits in the file.
  awk -v flag="$MAILCAL_REQUIRE_INJECTED_CONFIG" '
    { print }
    /^[[:space:]]*CARGO_HOME:/ && !done {
      match($0, /^[[:space:]]*/)
      printf "%sMAILCAL_REQUIRE_INJECTED_CONFIG: \"%s\"\n", substr($0, 1, RLENGTH), flag
      done = 1
    }
  ' "$MANIFEST" > "$MANIFEST_WITH_REQUIREMENT"
  # A manifest that stopped setting CARGO_HOME would leave the requirement unset and this whole
  # paragraph pointless, so the substitution is checked rather than assumed.
  grep -q '^[[:space:]]*MAILCAL_REQUIRE_INJECTED_CONFIG:' "$MANIFEST_WITH_REQUIREMENT" || {
    echo "package.sh: could not add the credential requirement to $MANIFEST: no CARGO_HOME line" >&2
    exit 1
  }
  MANIFEST="$MANIFEST_WITH_REQUIREMENT"
  echo "==> credentials: required: the build inside the sandbox will refuse to compile without them"
fi

flatpak-builder \
  --user --force-clean --disable-rofiles-fuse \
  --state-dir "$OUT/state" \
  --repo "$OUT/repo" \
  "$OUT/build" \
  "$MANIFEST"

if [[ "$BUNDLE" == "1" ]]; then
  flatpak build-bundle "$OUT/repo" "$OUT/mailcal.flatpak" "$APP_ID"
  echo "==> wrote $OUT/mailcal.flatpak"
fi

if [[ "$INSTALL" == "1" ]]; then
  # A local repo has no signature, so it needs its own remote; --no-gpg-verify is scoped to this
  # one, never to flathub.
  #
  # The URL is re-pointed on **every** install, not only when the remote is new. `--if-not-exists`
  # alone leaves an existing `mailcal-local` aimed wherever it was aimed before: and a worktree
  # builds into its own `target/`, so the second checkout on a machine installs the *first* one's
  # bundle and says "installed" about it. That is a build nobody can tell apart from the one they
  # just made, which is the whole failure this script exists to prevent elsewhere.
  flatpak remote-add --user --if-not-exists --no-gpg-verify mailcal-local "$OUT/repo"
  # `--url` stores what it is given verbatim, unlike `remote-add`, which tolerates a bare path.
  flatpak remote-modify --user --no-gpg-verify --url="file://$OUT/repo" mailcal-local
  flatpak install --user --noninteractive --assumeyes --reinstall mailcal-local "$APP_ID"
  echo "==> installed $APP_ID ($(flatpak info --show-metadata "$APP_ID" >/dev/null 2>&1 && echo ok))"
fi

if [[ "$RUN" == "1" ]]; then
  # GTK single-instances on the application id, so a debug build already holding it would swallow
  # this launch and exit; and vice versa. Clear the field first.
  pkill -f "$ROOT/target/debug/mailcal-linux" 2>/dev/null || true
  exec flatpak run --user "$APP_ID"
fi
