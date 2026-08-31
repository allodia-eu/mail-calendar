#!/usr/bin/env bash
# The app's identity; its name and its application id; resolved for this build.
#
# Sourced, never run:
#
#     . "$REPO_ROOT/scripts/dev/brand.sh"
#     brand_load                       # export every brand value
#     brand_value MAILCAL_APP_ID       # or read one
#
# Three places are consulted, first one wins (docs/branding.md):
#
#   the real environment          what the person or the CI job just set
#   branding/allodia.env          Allodia's identity, when that file is present
#   branding/default.env          the neutral default, which is always present
#
# The environment comes first so a one-off build can be re-branded without editing a file, and the
# absent case is the point rather than an error: with no `allodia.env` beside it; the state of the
# public repository; every build is unbranded, and nothing had to be switched off to get there.
#
# Deliberately not `set -a; . branding/*.env`: the product name contains an ampersand, and a shell
# that sources these files is a shell that can be made to run what is written in them.

BRAND_SH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BRAND_DIR="$(cd "$BRAND_SH_DIR/../../branding" && pwd)"
BRAND_DEFAULT_FILE="$BRAND_DIR/default.env"
BRAND_OVERRIDE_FILE="$BRAND_DIR/allodia.env"

# `KEY=value` lines with the comments, blanks, `export ` and surrounding space taken off. Not a
# parser for anything else: no interpolation, no continuation, no command substitution.
_brand_pairs() { # <file>
  [ -f "$1" ] || return 0
  sed -n '
    s/^[[:space:]]*//
    s/^export[[:space:]]\{1,\}//
    /^#/d
    /^$/d
    s/^\([A-Za-z_][A-Za-z0-9_]*\)[[:space:]]*=[[:space:]]*/\1=/p
  ' "$1"
}

# One pair of enclosing quotes, and any trailing space, removed.
_brand_unquote() { # <raw value>
  local value="$1"
  value="${value%"${value##*[![:space:]]}"}"
  case "$value" in
    \"*\") value="${value#\"}"; value="${value%\"}" ;;
    \'*\') value="${value#\'}"; value="${value%\'}" ;;
  esac
  printf '%s\n' "$value"
}

# Every name either file gives a value to, so a key added to the files needs no change here.
brand_keys() {
  {
    _brand_pairs "$BRAND_DEFAULT_FILE"
    _brand_pairs "$BRAND_OVERRIDE_FILE"
  } | sed 's/=.*//' | sort -u
}

brand_value() { # <key>
  local key="$1" line value
  value="${!key-}"
  if [ -n "$value" ]; then printf '%s\n' "$value"; return 0; fi
  local file
  for file in "$BRAND_OVERRIDE_FILE" "$BRAND_DEFAULT_FILE"; do
    # `|| true` on every grep here and below: these are sourced into scripts running under
    # `set -e -o pipefail`, where a grep that simply finds nothing is a failed pipeline that kills
    # the caller. Not finding a key is the ordinary case; it is how a checkout without
    # `allodia.env` resolves every value.
    line="$(_brand_pairs "$file" | { grep "^$key=" || true; } | tail -n 1)"
    if [ -n "$line" ]; then _brand_unquote "${line#*=}"; return 0; fi
  done
  printf '\n'
}

brand_load() {
  local key
  for key in $(brand_keys); do
    export "$key=$(brand_value "$key")"
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

# A source image, resolved the way the values above are: Allodia's if that file is beside it, the
# neutral one otherwise, and an environment override ahead of both so one-off art can be tried
# without moving files around.
#
# Art is not injected the way a name is; no client draws its art at build time, so what ships is
# whatever was committed. The switch is "swap the source, re-run the generators, commit what they
# wrote", and this is the half that decides which source that is.
_brand_source() { # <slot> <override variable>
  local slot="$1" override="${!2-}"
  if [ -n "$override" ]; then printf '%s\n' "$override"; return 0; fi
  if [ -f "$BRAND_DIR/allodia-$slot.png" ]; then printf '%s\n' "$BRAND_DIR/allodia-$slot.png"; return 0; fi
  printf '%s\n' "$BRAND_DIR/default-$slot.png"
}

# The image every launcher icon is cut from.
brand_icon_source() { _brand_source icon MAILCAL_ICON_SOURCE; }

# The illustration the welcome and account-setup screens show. A separate slot from the icon on
# purpose: the neutral one is a copy of the neutral icon today, and swapping in art drawn for the
# screen is then one file, with no launcher icon following it by accident.
brand_welcome_source() { _brand_source welcome MAILCAL_WELCOME_SOURCE; }
