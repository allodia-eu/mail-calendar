r"""Which `bash` to run, on a host that may have more than one.

Every suite in this repo that exercises a shell script has to spawn one, and on Windows that is not
the one-line `shutil.which("bash")` it looks like.

**Never hand `subprocess` a bare program name on Windows.** It resolves through `CreateProcess`,
which searches the application directory, the current directory and then **`System32`; before
`PATH`**. `System32\bash.exe` is WSL's launcher and ships with Windows whether or not a working
distribution does, so it wins even on a machine whose PATH puts Git Bash first. Measured on this
repo's own dev box: `shutil.which("bash")` answers `C:\Program Files\Git\usr\bin\bash.EXE` while
`subprocess.run(["bash", ...])` in the same interpreter gets WSL's, which then fails before reading
a word of the script:

    wsl: Failed to mount C:\, see dmesg for more details.
    <3>WSL (420 - Relay) ERROR: CreateProcessParseCommon:1023: getpwuid(0) failed 2

That produced errors that look like the scripts under test misbehaving when nothing in this repo was
involved, and it turned `scripts/dev/gate.sh` permanently red on a Windows dev box; which is the
failure mode AGENTS.md keeps warning about, because a gate that cannot run is a gate people stop
running.

**Git Bash is the answer, and it is a prerequisite rather than a fallback.** The gate is itself a
bash script, GitHub Actions offers `shell: bash` on `windows-*` runners (which is Git Bash), and Git
for Windows is already required to have a checkout at all. So this prefers Git Bash on Windows and
falls back to PATH only after that; but the part that actually fixes the bug is that it always
hands back an **absolute path**, so `CreateProcess` never gets to choose. It also verifies the
interpreter it picks actually runs, because presence is not enough: the broken launcher exists.

`MAILCAL_BASH` overrides the choice, for a host that keeps bash somewhere unusual.
"""

from __future__ import annotations

import functools
import os
import shutil
import subprocess
import sys
from pathlib import Path

__all__ = ["find_bash", "bash_problem", "bash_argv", "bash_path"]

_PROBE_TIMEOUT_SECONDS = 30


def _git_bash_candidates() -> list[str]:
    """Git Bash, found the way a machine actually has it rather than by guessing one path."""
    found: list[str] = []

    # Derived from git itself, so a non-default install location still resolves. `git` lands in
    # either `Git\cmd` or `Git\bin`; bash is always in `Git\bin`.
    git = shutil.which("git")
    if git:
        root = Path(git).resolve().parent.parent
        found.append(str(root / "bin" / "bash.exe"))

    for env in ("ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"):
        base = os.environ.get(env)
        if base:
            found.append(str(Path(base) / "Git" / "bin" / "bash.exe"))

    return found


def _candidates() -> list[str]:
    override = os.environ.get("MAILCAL_BASH")
    if override:
        return [override]

    if sys.platform != "win32":
        on_path = shutil.which("bash")
        return [on_path] if on_path else ["/bin/bash"]

    # Git Bash first. `shutil.which` last, because on Windows that is the WSL trap above.
    ordered = _git_bash_candidates()
    on_path = shutil.which("bash")
    if on_path:
        ordered.append(on_path)
    return ordered


def _runs(candidate: str) -> bool:
    """Whether this interpreter can actually execute something. Presence is not enough."""
    if not Path(candidate).exists() and not shutil.which(candidate):
        return False
    try:
        probe = subprocess.run(
            [candidate, "-c", "printf ok"],
            capture_output=True,
            text=True,
            timeout=_PROBE_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    return probe.stdout.strip() == "ok"


@functools.lru_cache(maxsize=1)
def find_bash() -> str | None:
    """The first bash on this host that exists and works, or `None`."""
    for candidate in _candidates():
        if candidate and _runs(candidate):
            return candidate
    return None


def bash_problem() -> str:
    """
    Empty when a usable bash was found; otherwise why not, in a sentence fit for a skip message.

    A skip that NAMES its reason is the honest report; a green tick would not be, and neither would
    an error that blames the script under test.
    """
    if find_bash():
        return ""
    tried = ", ".join(c for c in _candidates() if c) or "nothing"
    if sys.platform == "win32":
        return (
            "no working bash: install Git for Windows (Git Bash), or point MAILCAL_BASH at one. "
            f"Tried: {tried}. Note that the `bash.exe` in System32 is WSL's, not a shell."
        )
    return f"no working bash on this host. Tried: {tried}"


def bash_argv(*args: str) -> list[str]:
    """`[bash, *args]` for `subprocess`, using the interpreter this module chose."""
    chosen = find_bash()
    if chosen is None:
        raise RuntimeError(bash_problem())
    return [chosen, *args]


def bash_path(path: str | Path) -> str:
    r"""
    `path` as the chosen bash resolves it: a POSIX path on Windows, the path itself elsewhere.

    A shell script derives its own directories with `cd … && pwd`, so on Windows it works in
    `/c/Users/…` (or `/tmp/…`) while the Python driving it holds `C:\Users\…`. A test that builds
    an expected path with `str(Path(...))` therefore compares two spellings of the same directory
    and fails on Windows alone, in an assertion that has nothing to do with what it is testing.

    Resolved by asking bash, rather than by rewriting the string here: the mapping is the
    interpreter's own; `/tmp` is not under `/c`; and only it knows the answer.

    The directory must exist, for the same reason `cd` needs it to, and that is checked on every
    host rather than only where the `cd` runs: a helper whose contract changes with the platform is
    the thing this module exists to stop.
    """
    # Absolute first: a relative `cd` leaves bash on the logical PWD it inherited, which on Windows
    # is the Windows one; so a relative argument comes back spelled the way the caller feared.
    # `abspath`, not `resolve()`: bash's own `pwd` is logical, and on macOS the temp directory these
    # suites build in is reached through a symlink (`/var` → `/private/var`). Resolving it here
    # would answer a path the script never names.
    resolved = Path(os.path.abspath(path))
    if not resolved.is_dir():
        raise RuntimeError(f"bash_path needs a directory that exists: {path!r}")
    if sys.platform != "win32":
        return str(resolved)
    done = subprocess.run(
        bash_argv("-c", 'cd -- "$1" && pwd', "_", str(resolved)),
        capture_output=True,
        text=True,
        check=False,
    )
    if done.returncode != 0:
        raise RuntimeError(f"bash could not resolve {path!r}: {done.stderr.strip()}")
    return done.stdout.strip()
