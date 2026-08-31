#!/usr/bin/env python3
"""The app's identity; its name and its application id; resolved for this build.

The Python half of `brand.sh`, reading the same two files by the same rule (docs/branding.md):

    the real environment          what the person or the CI job just set
    branding/allodia.env          Allodia's identity, when that file is present
    branding/default.env          the neutral default, which is always present

Importable::

    from brand import resolve, value
    value("MAILCAL_APP_ID")

and runnable, which is how PowerShell and any other language reaches it without a fourth copy of
the parser::

    python3 scripts/dev/brand.py --json          # {"MAILCAL_APP_ID": "...", ...}
    python3 scripts/dev/brand.py MAILCAL_APP_ID  # one value, no newline games
    python3 scripts/dev/brand.py --icon-source   # the image the launcher icons are cut from
    python3 scripts/dev/brand.py --listing       # the store copy this build describes itself with

The absent case is the design, not a failure: with no `allodia.env`; the state of the public
repository; every build is unbranded, and nothing had to be switched off to get there.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Dict

sys.path.insert(0, str(Path(__file__).resolve().parent))

from envfile import parse_env_file  # noqa: E402  (after the path insert, deliberately)

REPO_ROOT = Path(__file__).resolve().parents[2]
BRAND_DIR = REPO_ROOT / "branding"
DEFAULT_FILE = BRAND_DIR / "default.env"
OVERRIDE_FILE = BRAND_DIR / "allodia.env"


def resolve(environ: Dict[str, str] = None) -> Dict[str, str]:
    """Every brand key with its value, environment first."""
    import os

    env = os.environ if environ is None else environ
    values = {}  # type: Dict[str, str]
    for path in (DEFAULT_FILE, OVERRIDE_FILE):
        if path.is_file():
            values.update(parse_env_file(path.read_text(encoding="utf-8")))
    for key in list(values):
        from_environment = env.get(key, "").strip()
        if from_environment:
            values[key] = from_environment
    return values


def defaults() -> Dict[str, str]:
    """Only `branding/default.env`; the neutral identity, ignoring every override.

    A generator that rewrites a committed file needs both halves: what is written there now (this)
    and what this build wants instead (`resolve`).
    """
    return parse_env_file(DEFAULT_FILE.read_text(encoding="utf-8"))


def value(key: str) -> str:
    """One brand value. Empty for a key no file names; callers treat that as absent."""
    return resolve().get(key, "")


def icon_source() -> Path:
    """The source image every launcher icon is cut from, by the same rule as the values above.

    Art is not injected the way a name is; no client draws its icon at build time, so what ships
    is whatever was committed. The switch is "swap the source, re-run the generators, commit what
    they wrote", and this decides which source that is. The twin of `brand_icon_source` in
    `brand.sh`.
    """
    import os

    override = os.environ.get("MAILCAL_ICON_SOURCE", "").strip()
    if override:
        return Path(override)
    allodia = BRAND_DIR / "allodia-icon.png"
    return allodia if allodia.is_file() else BRAND_DIR / "default-icon.png"


def listing_source() -> Path:
    """The store copy this build describes itself with, by the same rule as `icon_source`.

    A listing is copy rather than an injected value, so it resolves as a whole file: the branded one
    when it is there, the neutral default when it is not. `docs/store-listing.md` holds the rules
    both must obey and is not one of them.
    """
    import os

    override = os.environ.get("MAILCAL_LISTING_SOURCE", "").strip()
    if override:
        return Path(override)
    branded = BRAND_DIR / "allodia-listing.md"
    return branded if branded.is_file() else BRAND_DIR / "default-listing.md"


def main(argv) -> int:
    if len(argv) == 1 and argv[0] == "--listing":
        print(listing_source(), end="")
        return 0
    if len(argv) == 1 and argv[0] == "--icon-source":
        print(icon_source(), end="")
        return 0
    if len(argv) == 1 and argv[0] == "--json":
        print(json.dumps(resolve(), indent=2, sort_keys=True))
        return 0
    if len(argv) == 1 and not argv[0].startswith("-"):
        print(value(argv[0]), end="")
        return 0
    if not argv:
        for key, resolved in sorted(resolve().items()):
            print("%s=%s" % (key, resolved))
        return 0
    print(__doc__.strip().splitlines()[0], file=sys.stderr)
    print("usage: brand.py [--json | --icon-source | --listing | <KEY>]", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
