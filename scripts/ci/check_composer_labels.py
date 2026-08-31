#!/usr/bin/env python3
"""Fails when a client does not send the shared editor every label it has.

The composer's chrome lives in one bundle shared by four clients, so its strings cannot be baked
per-language. Each client passes its own translations through `window.setComposerLabels`, and
`clients/composer/src/labels.ts` is the list of what there is to pass.

Every way of getting this wrong is SILENT:

  * a client that never calls the hook keeps the bundle's built-in English -- which is what macOS,
    iOS and Windows shipped for two releases with nothing in any build to say so;
  * a key the bundle does not know is dropped by its `mergeLabels`;
  * a key it knows but a client omits keeps that one control's English default, in an otherwise
    translated toolbar.

None of the three throws, logs, or fails a build. They are visible only to someone running that
client in that language and looking at that control -- which is why adding a toolbar button and
translating it everywhere but one client is the easiest mistake in this area to make.

Checks two things per client: that it calls the hook at all, and that the key set it builds is
exactly the bundle's. What the strings SAY is not checked -- codegen already fails the build on a
catalog key that does not exist, and a wrong translation is not a machine's call.

Usage: check_composer_labels.py            (paths are fixed; run from anywhere in the repo)
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

BUNDLE = ROOT / "clients/composer/src/labels.ts"

# Where each client builds its label map, and the pattern that finds a key in it. The four
# languages spell the same dictionary four ways; each regex is anchored on the quoted JS key, which
# is the thing that has to match the bundle.
CLIENTS = {
    "android": (
        Path("clients/android/app/src/main/java/eu/allodia/mailcal/ComposerEditorHost.kt"),
        re.compile(r'put\("([A-Za-z]+)"'),
    ),
    "apple": (
        Path("clients/apple/Packages/MailcalKit/Sources/MailcalUI/ComposerLabels.swift"),
        re.compile(r'"([A-Za-z]+)":\s*L10n\.'),
    ),
    "linux": (
        Path("clients/linux/src/ui/composer.rs"),
        re.compile(r'"([A-Za-z]+)":\s*l10n::'),
    ),
    "windows": (
        Path("clients/windows/Mailcal/Services/ComposerLabels.cs"),
        re.compile(r'\["([A-Za-z]+)"\]\s*=\s*L10n\.'),
    ),
}

# Where to look for the call itself, and what a call looks like there. A client can build a perfect
# map and never send it -- a different bug with the same symptom, so it gets its own check.
#
# Each pattern matches an INJECTION and nothing else. Searching for the bare hook name does not
# work: it appears in the comments above every one of these maps, and on Apple and Windows in the
# helper that *builds* the script; so the check would be satisfied by the very file whose call
# sites had just been deleted. Android and Linux interpolate the JS directly, so `window.<hook>(`
# is their call; Apple and Windows go through `ComposerLabels.script()`, whose definition
# (`static func script()`) these do not match.
CALL_SITES = {
    "android": (
        Path("clients/android/app/src/main/java/eu/allodia/mailcal"),
        re.compile(r"window\.setComposerLabels\("),
    ),
    "apple": (
        Path("clients/apple/Packages/MailcalKit/Sources/MailcalUI"),
        re.compile(r"ComposerLabels\.script\(\)"),
    ),
    "linux": (
        Path("clients/linux/src"),
        re.compile(r"window\.setComposerLabels\("),
    ),
    "windows": (
        Path("clients/windows/Mailcal"),
        re.compile(r"ComposerLabels\.Script\(\)"),
    ),
}

HOOK = "setComposerLabels"


def bundle_keys(text: str) -> set[str]:
    """The fields of the bundle's `Labels` interface -- the list every client is measured against."""
    body = re.search(r"export interface Labels \{(.*?)\n\}", text, re.S)
    if not body:
        raise SystemExit(f"{BUNDLE}: no `export interface Labels` block -- has it been renamed?")
    return set(re.findall(r"^\s*([A-Za-z]+):\s*string;", body.group(1), re.M))


def calls_hook(directory: Path, pattern: re.Pattern[str]) -> bool:
    for path in directory.rglob("*"):
        if path.is_file() and path.suffix in {".kt", ".swift", ".rs", ".cs"}:
            if pattern.search(path.read_text(encoding="utf-8", errors="replace")):
                return True
    return False


def failures_for(
    expected: set[str],
    built: dict[str, tuple[str, set[str]]],
    callers: dict[str, bool],
) -> list[str]:
    """The complaints, given what each client builds and whether it sends it.

    Pure, so the checker's own behaviour is testable without a repo laid out on disk -- the point
    being that a checker nobody can make fail is the same silence it exists to break.
    """
    failures: list[str] = []
    for client, (where, found) in sorted(built.items()):
        missing = expected - found
        unknown = found - expected
        if missing:
            failures.append(
                f"{client} ({where}): does not send {len(missing)} label(s): "
                + ", ".join(sorted(missing))
            )
        if unknown:
            failures.append(
                f"{client} ({where}): sends {len(unknown)} label(s) the bundle ignores: "
                + ", ".join(sorted(unknown))
            )
    for client, sends in sorted(callers.items()):
        if not sends:
            failures.append(f"{client}: never calls window.{HOOK} -- its toolbar stays English")
    return failures


def main() -> int:
    expected = bundle_keys(BUNDLE.read_text(encoding="utf-8"))
    if not expected:
        raise SystemExit(f"{BUNDLE}: `Labels` has no fields -- the pattern must have gone stale.")

    built: dict[str, tuple[str, set[str]]] = {}
    callers: dict[str, bool] = {}
    missing_files: list[str] = []

    for client, (relative, pattern) in CLIENTS.items():
        path = ROOT / relative
        if not path.exists():
            missing_files.append(f"{client}: {relative} is missing -- did the label map move?")
            continue
        built[client] = (str(relative), set(pattern.findall(path.read_text(encoding="utf-8"))))

    for client, (relative, pattern) in CALL_SITES.items():
        directory = ROOT / relative
        if not directory.exists():
            missing_files.append(f"{client}: {relative} is missing")
            continue
        callers[client] = calls_hook(directory, pattern)

    failures = missing_files + failures_for(expected, built, callers)

    if failures:
        print("Editor labels are out of sync with clients/composer/src/labels.ts:\n")
        for failure in failures:
            print(f"  {failure}")
        print(
            "\nAdd the key to every client's label map (and a catalog `editor_*` string for it),"
            "\nor remove it from the bundle's Labels interface."
        )
        return 1

    print(f"Editor labels: {len(expected)} keys, sent by all {len(CLIENTS)} clients.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
