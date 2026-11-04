#!/usr/bin/env python3
"""Render the AUR's PKGBUILD from clients/linux/aur/PKGBUILD.in.

Called by clients/linux/package-aur.sh, which passes every value in the environment as
`AUR_<FIELD>`, so no quote or backslash in one can reach the code that writes it.

A placeholder is `@FIELD@` and takes the value as it stands, for the fields the script has already
written as PKGBUILD syntax (the source and checksum arrays). `@'FIELD'@` takes it quoted for the
shell, for free text such as the summary. The template's first comment block describes the
template rather than the recipe, and is left out.
"""

from __future__ import annotations

import os
import re
import shlex
import sys
from collections.abc import Mapping
from pathlib import Path

PLACEHOLDER = re.compile(r"@(')?([A-Z0-9_]+)(')?@")
PREFIX = "AUR_"


class RenderError(Exception):
    """A placeholder had no value, or was half quoted."""


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
    if len(argv) != 2:
        print("usage: aur_pkgbuild.py <template> <out>", file=sys.stderr)
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
