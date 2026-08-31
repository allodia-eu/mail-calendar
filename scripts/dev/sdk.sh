#!/usr/bin/env bash
# Build and run the Linux client against the **GNOME runtime it ships on**, rather than against
# whatever GTK the developer's distribution happens to carry.
#
# Sourced by clients/linux/build-and-run.sh and scripts/dev/test-linux-ui.sh; also runnable:
#
#     scripts/dev/sdk.sh version                 # the runtime branch, from the Flatpak manifest
#     scripts/dev/sdk.sh cargo build -p mailcal-linux
#     scripts/dev/sdk.sh test -p mailcal-linux --all-features
#     scripts/dev/sdk.sh exec target/flatpak-sdk/debug/mailcal-linux
#
# Why this exists: the Flatpak links GTK, libadwaita and WebKitGTK from `org.gnome.Platform`, and
# that is what a user gets whatever their distribution ships. The development baseline tracks the
# same GNOME generation, so the gap is now small; but a distribution's GTK and the runtime's are
# two builds on two schedules, only one of which is pinned, and the difference reaches the screen.
# Verifying against the pinned one is the point.
#
# The binary this produces **cannot run on the host**: the runtime's glibc is newer than the
# development baseline's, so it starts only inside the sandbox, through `sdk_exec`.

# The runtime is named once, in the Flatpak manifest, and read from there; a second copy here is
# a version this could silently disagree with.
sdk_runtime_version() {
  # The committed manifest, which carries the unbranded id (docs/branding.md); the
  # runtime version is the same in the branded copy package.sh generates from it.
  local manifest="$REPO_ROOT/clients/linux/flatpak/org.mailcal.client.yml"
  local version
  version="$(sed -n "s/^runtime-version: *['\"]\\([^'\"]*\\)['\"].*/\\1/p" "$manifest" | head -1)"
  [[ -n "$version" ]] || die "no runtime-version in $manifest"
  printf '%s\n' "$version"
}

# `--user` or `--system`, whichever installation carries the SDK.
#
# Not optional: with the SDK present in both; which is what a machine that has ever installed it
# twice looks like; an unqualified ref makes flatpak *prompt*, and a prompt in a script is a hang
# or an "error: No ref chosen to resolve matches".
sdk_installation() {
  local version="${1:-$(sdk_runtime_version)}" scope
  for scope in --user --system; do
    if flatpak info "$scope" "org.gnome.Sdk//$version" >/dev/null 2>&1; then
      printf '%s\n' "$scope"
      return 0
    fi
  done
  return 1
}

sdk_available() {
  command -v flatpak >/dev/null 2>&1 && sdk_installation >/dev/null 2>&1
}

# What to tell someone who has not installed it. The runtime is a ~2 GB one-time download, which is
# why nothing here performs it silently.
sdk_install_hint() {
  local version="${1:-$(sdk_runtime_version)}"
  printf 'flatpak install --user flathub org.gnome.Platform//%s org.gnome.Sdk//%s org.freedesktop.Sdk.Extension.rust-stable//25.08\n' \
    "$version" "$version"
}

# This file's own path, so `sdk_test` can re-enter it inside the session it sets up.
SDK_SH="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/sdk.sh"

# Extra flatpak arguments for `sdk_cargo`, which `sdk_test` sets: a test that opens windows needs a
# display and a session bus that a plain build has no use for.
SDK_CARGO_EXTRA=()

# The target directory for SDK builds. Deliberately **not** the host's: a different libc and a
# different rustc mean cargo would rebuild the world on every switch between the two.
sdk_target_dir() {
  printf '%s\n' "$REPO_ROOT/target/flatpak-sdk"
}

# Runs cargo inside the SDK.
#
# The SDK carries its own rust-stable extension, so `rust-toolchain.toml` does not apply here,
# there is no rustup inside to read it. That also puts `cargo fmt` out of reach: the extension is
# stable, and rustfmt.toml is all nightly options, which stable rustfmt *warns about and ignores*
# rather than refusing. Formatting stays on the host, where it needs no GTK anyway.
sdk_cargo() {
  local version installation
  version="$(sdk_runtime_version)"
  installation="$(sdk_installation "$version")" ||
    die "the GNOME $version SDK is not installed: $(sdk_install_hint "$version")"
  # `flatpak` itself reads XDG_DATA_HOME to find the user installation, and a test session may have
  # redirected it; hand it back the real one.
  env XDG_DATA_HOME="$HOME/.local/share" flatpak run "$installation" --devel \
    --share=network --filesystem=host --filesystem=/tmp \
    --env=CARGO_TARGET_DIR="$(sdk_target_dir)" \
    --env=CARGO_HOME="$HOME/.cargo" \
    ${SDK_CARGO_EXTRA[@]+"${SDK_CARGO_EXTRA[@]}"} \
    --command=sh "org.gnome.Sdk//$version" -c \
    'export PATH=/usr/lib/sdk/rust-stable/bin:$PATH; cd "$1"; shift; exec cargo "$@"' \
    -- "$REPO_ROOT" "$@"
}

# Runs a binary built by `sdk_cargo` inside the runtime. Extra flatpak arguments; `--env=`,
# `--filesystem=`, `--no-a11y-bus`; may be passed before the binary path.
#
# Accessibility is left on flatpak's own proxy here, which is what an ordinary launch wants: it
# bridges the app to the session's accessibility bus, and this client has assistive-technology
# behaviour worth being able to try. A run that needs a **private** bus instead; the semantic UI
# acceptance test; passes `--no-a11y-bus` and its own `AT_SPI_BUS_ADDRESS`, because the proxy
# would otherwise point the app at the session's bus and the driver would watch an empty one.
#
# Three of these flags are not decoration; each one is a way this fails without it:
#
#   --filesystem=/tmp   the sandbox otherwise gets its *own* /tmp, and `--filesystem=host` does not
#                       cover it; so a private D-Bus socket, an X authority file or a test fixture
#                       living there is simply absent.
#   --unset-env=__EGL…  a test session pins the host's Mesa ICD by absolute path; inside the
#                       sandbox that path names nothing, and libepoxy aborts before the first window.
#   --device=dri        WebKitGTK falls back to a software rasteriser without it.
sdk_exec() {
  local version installation extra=()
  version="$(sdk_runtime_version)"
  installation="$(sdk_installation "$version")" ||
    die "the GNOME $version SDK is not installed: $(sdk_install_hint "$version")"
  while [[ $# -gt 0 && "$1" == --* ]]; do
    extra+=("$1")
    shift
  done
  [[ $# -gt 0 ]] || die "sdk_exec needs a binary to run"
  local binary="$1"
  shift
  [[ -x "$binary" ]] || die "no SDK build at $binary: run the build first"
  env XDG_DATA_HOME="$HOME/.local/share" flatpak run "$installation" --devel \
    --share=network --share=ipc \
    --socket=x11 --socket=wayland --socket=session-bus \
    --device=dri \
    --filesystem=host --filesystem=/tmp \
    --unset-env=__EGL_VENDOR_LIBRARY_FILENAMES \
    --unset-env=__GLX_VENDOR_LIBRARY_NAME \
    --unset-env=GALLIUM_DRIVER \
    ${extra[@]+"${extra[@]}"} \
    --command="$binary" "org.gnome.Sdk//$version" "$@"
}

# Runs the crate's own tests inside the SDK; the widget assertions, against the toolkit that
# ships rather than the one this distribution happens to carry.
#
# It builds the session itself because two of the crate's tests need one and say so unhelpfully
# when it is missing: `gtk::init` wants a display, and `GtkApplication::register` wants a session
# bus; without the latter the single GTK test dies with a bare
# `GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown`, which reads exactly like a toolkit
# difference and is not one.
#
# Slower than the host run by a wide margin; portal activation and software rendering, not
# compilation; so this is the deliberate check, not the inner loop.
sdk_test() {
  if [[ -z "${SDK_TEST_SESSION:-}" ]]; then
    require_cmd xvfb-run
    require_cmd dbus-run-session
    SDK_TEST_SESSION=1 exec xvfb-run --auto-servernum dbus-run-session -- \
      "$SDK_SH" test ${@+"$@"}
  fi
  SDK_CARGO_EXTRA=(
    --share=ipc
    --socket=x11
    --socket=session-bus
    --env=DISPLAY="$DISPLAY"
    --env=GSK_RENDERER=cairo
    --env=LIBGL_ALWAYS_SOFTWARE=1
  )
  sdk_cargo test ${@+"$@"}
}

# Prints the toolkit versions the runtime carries, so a run says what it was verified against.
sdk_versions() {
  local version installation
  version="$(sdk_runtime_version)"
  installation="$(sdk_installation "$version")" || return 1
  env XDG_DATA_HOME="$HOME/.local/share" flatpak run "$installation" --devel \
    --command=sh "org.gnome.Sdk//$version" -c \
    'printf "GTK %s · libadwaita %s · WebKitGTK %s" \
       "$(pkg-config --modversion gtk4)" \
       "$(pkg-config --modversion libadwaita-1)" \
       "$(pkg-config --modversion webkitgtk-6.0)"' 2>/dev/null
}

# Runnable as well as sourceable.
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  set -euo pipefail
  source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"
  case "${1:-}" in
    version) sdk_runtime_version ;;
    versions) sdk_versions; echo ;;
    available) sdk_available && echo yes || { echo no; exit 1; } ;;
    cargo) shift; sdk_cargo "$@" ;;
    test) shift; sdk_test ${@+"$@"} ;;
    exec) shift; sdk_exec "$@" ;;
    *) sed -n '2,16p' "${BASH_SOURCE[0]}"; exit 1 ;;
  esac
fi
