#!/usr/bin/env bash
# The app's identity; its name and its application id; resolved for this build.
#
# Sourced, never run:
#
#     . "$REPO_ROOT/scripts/dev/brand.sh"
#     brand_load                       # export every brand value
#     brand_value MAILCAL_APP_ID       # or read one
#
# Four places are consulted, first one wins (docs/branding.md):
#
#   the real environment          what the person or the CI job just set
#   branding/brand.env            the publisher's identity, when that file is present
#   branding/allodia.env          the same slot under its older name, still read
#   branding/default.env          the neutral default, which is always present
#
# The environment comes first so a one-off build can be re-branded without editing a file, and the
# absent case is the point rather than an error: with no brand file beside it; the state of the
# public repository; every build is unbranded, and nothing had to be switched off to get there.
#
# Deliberately not `set -a; . branding/*.env`: the product name contains an ampersand, and a shell
# that sources these files is a shell that can be made to run what is written in them.

BRAND_SH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRAND_DIR="$(cd "$BRAND_SH_DIR/../../branding" && pwd)"
BRAND_DEFAULT_FILE="$BRAND_DIR/default.env"
BRAND_OVERRIDE_FILES=("$BRAND_DIR/brand.env" "$BRAND_DIR/allodia.env")

# Nearly every script here sources this file, so reading the brand starts no process: under Git
# Bash each one costs tens of milliseconds, and a pipeline per key made sourcing it take seconds.
# Builtins only from here to `brand_load`; `$(…)` forks too, so values travel through namerefs.

# `KEY=value` lines with the comments, blanks, `export ` and surrounding space taken off, into
# the associative array named <map>; a later line for a key replaces an earlier one. Not a parser
# for anything else: no interpolation, no continuation, no command substitution.
_brand_read() { # <file> <name of an associative array>
  local -n _brand_pairs="$2"
  local _brand_line
  [ -f "$1" ] || return 0
  while IFS= read -r _brand_line || [ -n "$_brand_line" ]; do
    _brand_line="${_brand_line#"${_brand_line%%[![:space:]]*}"}"
    [[ $_brand_line =~ ^export[[:space:]]+(.*)$ ]] && _brand_line="${BASH_REMATCH[1]}"
    [[ $_brand_line =~ ^([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*=[[:space:]]*(.*)$ ]] || continue
    _brand_pairs[${BASH_REMATCH[1]}]="${BASH_REMATCH[2]}"
  done <"$1"
}

# Every key the files name, with the value the first file naming it gives, as written there.
_brand_files() { # <name of an associative array>
  local -n _brand_merged="$1"
  local _brand_file _brand_key
  local -A _brand_one
  for _brand_file in "${BRAND_OVERRIDE_FILES[@]}" "$BRAND_DEFAULT_FILE"; do
    _brand_one=()
    _brand_read "$_brand_file" _brand_one
    for _brand_key in "${!_brand_one[@]}"; do
      [[ -v _brand_merged[$_brand_key] ]] || _brand_merged[$_brand_key]="${_brand_one[$_brand_key]}"
    done
  done
}

# <raw value> with any trailing space and one pair of enclosing quotes removed, into <variable>.
_brand_unquote() { # <variable to set> <raw value>
  local -n _brand_out="$1"
  local _brand_raw="$2"
  _brand_raw="${_brand_raw%"${_brand_raw##*[![:space:]]}"}"
  case "$_brand_raw" in
    \"*\") _brand_raw="${_brand_raw#\"}"; _brand_raw="${_brand_raw%\"}" ;;
    \'*\') _brand_raw="${_brand_raw#\'}"; _brand_raw="${_brand_raw%\'}" ;;
  esac
  _brand_out="$_brand_raw"
}

# <key>'s value into <variable>: the environment's when it is set and not empty, else the files'.
_brand_resolve() { # <variable to set> <key> <name of the array `_brand_files` filled>
  local -n _brand_into="$1" _brand_from="$3"
  local _brand_env="${!2-}"
  if [ -n "$_brand_env" ]; then
    _brand_into="$_brand_env"
  elif [[ -v _brand_from[$2] ]]; then
    _brand_unquote _brand_into "${_brand_from[$2]}"
  else
    _brand_into=""
  fi
}

# Every name either file gives a value to, so a key added to the files needs no change here.
brand_keys() {
  local -A _brand_all=()
  _brand_files _brand_all
  [ "${#_brand_all[@]}" -eq 0 ] || printf '%s\n' "${!_brand_all[@]}" | sort -u
}

brand_value() { # <key>
  local -A _brand_all=()
  local _brand_value
  _brand_files _brand_all
  _brand_resolve _brand_value "$1" _brand_all
  printf '%s\n' "$_brand_value"
}

brand_load() {
  local -A _brand_all=()
  local _brand_key _brand_value
  _brand_files _brand_all
  for _brand_key in "${!_brand_all[@]}"; do
    _brand_resolve _brand_value "$_brand_key" _brand_all
    export "$_brand_key=$_brand_value"
  done
}

# XcodeGen substitutes `${VAR}` from the environment and, when nothing set it, leaves the literal
# text in the generated project; so a project generated without `brand_load` builds an app whose
# bundle id is the string `${MAILCAL_APP_ID}`. It signs, installs and launches; only the keychain,
# the app group and the OAuth redirects quietly address nobody. Nothing downstream notices, so this
# does.
brand_assert_expanded() { # <generated file or directory>
  local target="$1" found
  found="$(grep -ro '\${MAILCAL_[A-Za-z0-9_]*}' "$target" 2>/dev/null | sort -u || true)"
  if [ -n "$found" ]; then
    printf 'error: %s still holds brand placeholders:\n%s\n' "$target" "$found" >&2
    printf '       the brand was not loaded before generating it (docs/branding.md).\n' >&2
    exit 1
  fi
}

# A source image, resolved the way the values above are: the publisher's if that file is beside
# it, the neutral one otherwise, and an environment override ahead of both so one-off art can be tried
# without moving files around.
#
# Art is not injected the way a name is; no client draws its art at build time, so what ships is
# whatever was committed. The switch is "swap the source, re-run the generators, commit what they
# wrote", and this is the half that decides which source that is.
_brand_source() { # <slot> <override variable>
  local slot="$1" override="${!2-}"
  if [ -n "$override" ]; then printf '%s\n' "$override"; return 0; fi
  local brand
  for brand in brand allodia; do
    if [ -f "$BRAND_DIR/$brand-$slot.png" ]; then printf '%s\n' "$BRAND_DIR/$brand-$slot.png"; return 0; fi
  done
  printf '%s\n' "$BRAND_DIR/default-$slot.png"
}

# The image every launcher icon is cut from.
brand_icon_source() { _brand_source icon MAILCAL_ICON_SOURCE; }

# The illustration the welcome and account-setup screens show. A separate slot from the icon on
# purpose: the neutral one is a copy of the neutral icon today, and swapping in art drawn for the
# screen is then one file, with no launcher icon following it by accident.
brand_welcome_source() { _brand_source welcome MAILCAL_WELCOME_SOURCE; }
