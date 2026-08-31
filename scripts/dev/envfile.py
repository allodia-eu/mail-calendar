#!/usr/bin/env python3
"""Read a credentials file, the way every publishing script in this repo does.

A handful of scripts here push to somewhere real; the Microsoft Store, App Store Connect, Google
Play, the docs asset store; and each needs a secret that must not be committed. They look in the
same three places, in this order:

  <repo>/.env                            the one to use: gitignored, and in `.worktreeinclude`, so
                                         every new worktree gets a copy automatically
  <repo>/.<name>.env                     per-tool, for a secret you want kept apart
  ~/.config/allodia/<name>.env           outside every checkout

**Every existing location is read and merged, first wins per key**; not "first file found wins".
That distinction is the whole reason `.env` can be added in front of files that already work: a
`.env` holding only `ALLODIA_DOCS_*` must not hide the `MSSTORE_*` keys sitting in `.msstore.env`
next to it. Stopping at the first file would do exactly that, silently, and the failure would look
like a credential that had gone missing.

Naming an explicit file (a `--env-file` flag, or the tool's `*_ENV_FILE` variable) means *that file
and nothing else*; asking for one and being handed a merge of three is how you publish with the
wrong account.

The real environment always wins over every file. A file is a convenience, not an override of what
the person running the command just typed.
"""

from __future__ import annotations

import os
import re
import stat
import sys
from pathlib import Path
from typing import Dict, Optional, Sequence, Tuple

REPO_ROOT = Path(__file__).resolve().parents[2]

_ENV_LINE = re.compile(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$")


class EnvFileError(RuntimeError):
    """A credentials file was named and could not be used."""


def locations_for(name: str) -> Tuple[Path, Path, Path]:
    """The three places a `<name>` credentials file is looked for, in precedence order."""
    return (
        REPO_ROOT / ".env",
        REPO_ROOT / (".%s.env" % name),
        Path.home() / ".config" / "allodia" / ("%s.env" % name),
    )


def parse_env_file(text: str) -> Dict[str, str]:
    """`KEY=value` lines into a dict. Comments, blanks, `export ` and quotes are tolerated.

    Deliberately not a shell: no interpolation, no command substitution, no multi-line values. A
    credentials file is a few lines, and a parser that can run something is a parser that can be
    made to run something.
    """
    values = {}  # type: Dict[str, str]
    for line in text.splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        matched = _ENV_LINE.match(line)
        if not matched:
            continue
        value = matched.group(2)
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        values[matched.group(1)] = value
    return values


def named_env_file(
    locations: Sequence[Path], file_var: str, explicit: Optional[str] = None
) -> Optional[Path]:
    """The one file this run was *told* to read, or `None` to search `locations`.

    A named file that does not exist *is* an error; asking for a file and being given the
    environment instead is how you end up publishing with the wrong account.
    """
    if explicit:
        path = Path(explicit)
        if not path.is_file():
            raise EnvFileError("no such credentials file: %s" % path)
        return path
    named = os.environ.get(file_var, "").strip()
    if named:
        path = Path(named)
        if not path.is_file():
            raise EnvFileError("%s points at a file that is not there: %s" % (file_var, path))
        return path
    return None


def find_env_file(
    locations: Sequence[Path], file_var: str, explicit: Optional[str] = None
) -> Optional[Path]:
    """The first credentials file that exists, or `None`. Never raises for a missing default.

    Kept for callers that want a single path to talk about. Prefer `load`, which merges every
    location; this one cannot see a key that lives only in a later file.
    """
    named = named_env_file(locations, file_var, explicit)
    if named is not None:
        return named
    for candidate in locations:
        if candidate.is_file():
            return candidate
    return None


def warn_if_readable_by_others(path: Path, what: str) -> None:
    """A one-line nudge if the file is group/world readable. POSIX only; Windows has no mode."""
    if os.name != "posix":
        return
    mode = path.stat().st_mode
    if mode & (stat.S_IRWXG | stat.S_IRWXO):
        print(
            "warning: %s is readable by other users; it holds %s. chmod 600 %s" % (path, what, path),
            file=sys.stderr,
        )


def load(
    locations: Sequence[Path], file_var: str, what: str, explicit: Optional[str] = None
) -> Tuple[Dict[str, str], Dict[str, Path]]:
    """Credential values, plus which file each key came from.

    A named file (`explicit` or `$<file_var>`) is read alone. Otherwise **every** location that
    exists is read and merged, earlier winning per key; so a shared `.env` can sit in front of a
    per-tool file without hiding the keys only that file has.

    The per-key provenance is returned rather than one path because after a merge there isn't one:
    a message that has to tell someone which file to go and fix needs the file that supplied *this*
    value, not the first one that happened to exist.
    """
    named = named_env_file(locations, file_var, explicit)
    paths = [named] if named is not None else [p for p in locations if p.is_file()]

    values = {}  # type: Dict[str, str]
    sources = {}  # type: Dict[str, Path]
    for path in paths:
        warn_if_readable_by_others(path, what)
        for key, value in parse_env_file(path.read_text(encoding="utf-8")).items():
            if key not in values:  # earlier location wins
                values[key] = value
                sources[key] = path
    return values, sources
