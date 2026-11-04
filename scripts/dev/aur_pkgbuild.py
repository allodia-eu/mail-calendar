#!/usr/bin/env python3
"""Render the AUR's PKGBUILD from clients/linux/aur/PKGBUILD.in.

Called by clients/linux/package-aur.sh, which passes every value in the environment as
`AUR_<FIELD>`, so no quote or backslash in one can reach the code that writes it.

A placeholder is `@FIELD@` and takes the value as it stands, for the fields the script has already
written as PKGBUILD syntax (the source and checksum arrays). `@'FIELD'@` takes it quoted for the
shell, for free text such as the summary. The template's first comment block describes the
template rather than the recipe, and is left out.

`--brand-sources` prints, one per line, the publisher's files the recipe carries beside the source
archive, which leaves them out because git ignores them.
"""

from __future__ import annotations

import os
import re
import shlex
import sys
from collections.abc import Mapping
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import brand  # noqa: E402  (after the path insert, deliberately)

PLACEHOLDER = re.compile(r"@(')?([A-Z0-9_]+)(')?@")
PREFIX = "AUR_"


class RenderError(Exception):
    """A placeholder had no value, or was half quoted, or a brand file cannot travel."""


def brand_sources() -> list[Path]:
    """The publisher's files an AUR build reads: its identity, which the Rust build compiles in,
    and its listing, which the desktop entry and metainfo are written from.

    Never its images: the Linux build installs the committed icons and draws no welcome art, and
    the AUR refuses any file over 256,000 bytes. The neutral `default-` files are tracked, so they
    are in the archive already.
    """
    listing = brand.listing_source()
    if listing.resolve().parent != brand.BRAND_DIR.resolve():
        raise RenderError(
            f"MAILCAL_LISTING_SOURCE names {listing}, which an AUR build cannot see: "
            "put the listing in branding/"
        )
    candidates = (*brand.OVERRIDE_FILES, listing)
    return [path for path in candidates if path.is_file() and not path.name.startswith("default")]


def render(template: str, fields: Mapping[str, str]) -> str:
    """The PKGBUILD for `template` with every placeholder filled from `fields`."""
    _, separator, body = template.partition("\n\n")
    if not separator:
        raise RenderError("the template has no comment block followed by a blank line")
    missing: set[str] = set()

    def fill(match: re.Match[str]) -> str:
        opened, name, closed = match.groups()
        if bool(opened) != bool(closed):
            raise RenderError(f"{match.group(0)} is quoted on one side only")
        if name not in fields:
            missing.add(name)
            return match.group(0)
        return shlex.quote(fields[name]) if opened else fields[name]

    rendered = PLACEHOLDER.sub(fill, body)
    if missing:
        raise RenderError(f"no value for {', '.join(sorted(missing))}")
    return rendered


def main(argv: list[str]) -> int:
    if argv == ["--brand-sources"]:
        try:
            print("".join(f"{path}\n" for path in brand_sources()), end="")
        except RenderError as error:
            print(f"aur-pkgbuild: {error}", file=sys.stderr)
            return 1
        return 0
    if len(argv) != 2:
        print("usage: aur_pkgbuild.py <template> <out> | --brand-sources", file=sys.stderr)
        return 2
    fields = {k[len(PREFIX):]: v for k, v in os.environ.items() if k.startswith(PREFIX)}
    try:
        text = render(Path(argv[0]).read_text(encoding="utf-8"), fields)
    except RenderError as error:
        print(f"aur-pkgbuild: {error}", file=sys.stderr)
        return 1
    Path(argv[1]).write_text(text, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
