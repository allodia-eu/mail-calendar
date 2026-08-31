#!/usr/bin/env python3
"""Push the resolved store listing (`branding/<brand>-listing.md`) into an App Store listing; all
seven languages, one run.

    scripts/dev/appstore_listing.py                                   # plan only: what would change
    scripts/dev/appstore_listing.py --apply                           # write the copy
    scripts/dev/appstore_listing.py --apply --release-notes 0.3.0     # ...and this version's What's new
    scripts/dev/appstore_listing.py --apply --screenshots showcase-screenshots/macos
    scripts/dev/appstore_listing.py --platform IOS --apply \
        --screenshots showcase-screenshots/iphone showcase-screenshots/ipad
    scripts/dev/appstore_listing.py --offline --out /tmp/listing      # no network; paste-ready files

**macOS and iOS/iPadOS are separate App Store *versions*** of one app record, so each is a run of
its own: `--platform` picks which, and it decides both the version the copy lands on and which
`docs/changelog/released/` section becomes What's new. The galleries differ too; the Mac takes one
(`APP_DESKTOP`), iOS takes an iPhone and an iPad one; so `--screenshots` accepts a directory per
gallery and reads the slot off the directory's own name.

App Store Connect asks for a name, a subtitle, promotional text, keywords, a description, What's new
and a screenshot gallery **per language**, and we ship seven; 49 form fields, each of which can
drift from the file it was copied out of without anyone noticing. The resolved listing
(`branding/<brand>-listing.md`, via `brand.listing_source()`) is the source of truth for that copy;
this makes the console agree with it, the way [`msstore_listing.py`](msstore_listing.py) already
does for the Microsoft Store.

**Nothing is written without `--apply`.** The default run measures every field locally, then asks
`asc metadata push --dry-run` what it would change, and prints that. This is AGENTS.md's "a store
upload is a remote gate too" rule: the plan is where a cap or a missing locale surfaces, rather than
in a console that has already taken half the languages.

**The transport is the `asc` CLI** (github.com/rorkai/App-Store-Connect-CLI), not an API client
written here; so auth, retries and the App Store Connect schema stay one dependency's problem. Set
it up once with `asc auth login --key-id … --issuer-id … --private-key AuthKey_*.p8`; `asc auth
status` says whether it is done. This script shells out to it and never sees a credential.

**What it writes, per language:**

| App Store Connect field | Comes from                                                          |
|-------------------------|---------------------------------------------------------------------|
| Name                    | `store-listing.md` -> "Product name / title"                         |
| Subtitle                | `store-listing.md` -> "App Store Connect; Subtitle …"               |
| Promotional text        | idem                                                                 |
| Keywords                | idem                                                                 |
| Description             | "Shared description; <language>" (substituting `{KEYSTORE}` if used) |
| Marketing / Support URL | "Console-side metadata" -> "Shared fields"                           |
| Privacy policy URL      | idem                                                                 |
| Copyright               | idem, minus the `©` (Apple renders the symbol itself)                |
| What's new              | `docs/changelog/released/<VERSION>.md`, the **`--platform`** section  |
| Screenshots             | `--screenshots DIR …`, each holding `<locale>-<screen>.png`          |

**It stops before submitting for review.** Pricing, availability, the age rating, the App Privacy
answers and the build selection are console-side (`store-listing.md` says so), and pressing Submit
is a decision a person makes with the whole page in front of them. Everything here is reversible up
to that point.

**Two Apple-shaped traps this navigates**, both of which fail quietly rather than loudly:

- **A locale code the store does not offer writes nothing and reports nothing.** Italian is `it`,
  never `it-IT` (`APP_STORE_LOCALES` in [`appstore_payload.py`](appstore_payload.py)).
- **An app has two app-info records**; the live one and the editable one; and `asc` refuses to
  guess. This picks the editable one by state, so the name/subtitle edit lands where it can.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
sys.path.insert(0, str(_HERE.parents[1] / "scripts" / "ci"))

import brand  # noqa: E402  (path set above so this runs as a script)
from appstore_payload import (  # noqa: E402
    ASC_PLATFORMS,
    SCREENSHOT_SLOTS,
    ListingError,
    collect_screenshots,
    load_listings,
    measure,
    measure_screenshots,
    resolve_locales,
    shared_fields,
    stage_screenshots,
    write_metadata,
)
from changelog_fragments import DocumentShapeError  # noqa: E402
from check_store_copy_length import LIMITS_PATH, listing_path, parse_limits  # noqa: E402

REPO_ROOT = _HERE.parents[1]
VERSION_PATH = REPO_ROOT / "VERSION"
PROJECT_PATH = REPO_ROOT / "clients" / "apple" / "project.yml"

# The live record is the one users see; the other is the one a submission edits. Apple keeps exactly
# these two, so "not the live one" identifies the editable one without hard-coding an id.
LIVE_STATE = "READY_FOR_DISTRIBUTION"

_BUNDLE_ID = re.compile(r"PRODUCT_BUNDLE_IDENTIFIER:\s*(\S+)")


def bundle_id(project=None) -> str:
    """The app's bundle id, read from `project.yml` rather than mirrored here.

    A hard-coded id is a mirror that can drift, and its failure mode is pushing this app's copy
    onto a different app's listing; not a mistake a review step catches. `project.yml` takes
    PRODUCT_BUNDLE_IDENTIFIER from exactly this (docs/branding.md), so the two cannot disagree; the
    project is still read for the one thing the brand cannot tell us, that it has not gone back to
    a literal.
    """
    path = PROJECT_PATH if project is None else Path(project)
    if not _BUNDLE_ID.search(path.read_text(encoding="utf-8")):
        raise ListingError(f"no PRODUCT_BUNDLE_IDENTIFIER in {path}")
    found = brand.value("MAILCAL_APP_ID")
    if not found:
        raise ListingError(
            "no MAILCAL_APP_ID: branding/default.env names it and is not optional (docs/branding.md)."
        )
    return found


def current_version() -> str:
    """`/VERSION`; the last released version, which is the one a store is being told about."""
    return VERSION_PATH.read_text(encoding="utf-8").strip()


def check_urls(listings, report) -> list:
    """The listing's three URLs, fetched. Returns the ones a store would show as broken.

    Apple opens the support URL during review, so a 404 there is a rejection; and this repo had one
    for a while (`/support`, which never existed) with nothing able to notice: a URL in a markdown
    table is not something `store-copy` can measure. Fetching it is the only check that can fail.

    A transport error is reported but does not stop the run: "DNS did not answer on this laptop" and
    "the page is gone" are different facts, and only the second is the listing's problem.
    """
    from urllib.error import HTTPError, URLError  # noqa: PLC0415; only this path needs them
    from urllib.request import Request, urlopen  # noqa: PLC0415

    seen, broken = set(), []
    for listing in listings:
        for what, url in (
            ("support URL", listing.support_url),
            ("marketing URL", listing.marketing_url),
            ("privacy policy URL", listing.privacy_policy_url),
        ):
            if url in seen:
                continue
            seen.add(url)
            try:
                with urlopen(  # noqa: S310; https URLs straight out of the contract
                    Request(url, headers={"User-Agent": "allodia-store-listing"}), timeout=15
                ) as response:
                    report(f"  {what:<18} {url}  {response.status}")
            except HTTPError as error:
                broken.append(f"{what} {url} answers {error.code}")
            except (URLError, OSError) as error:
                report(f"  {what:<18} {url}  unreachable from here ({error}): not checked")
    return broken


def apple_copyright(fields: dict) -> str:
    """The "Shared fields" copyright, as App Store Connect wants it stated.

    Apple renders the `©` itself and asks for "the year and name of the copyright holder", so the
    doc's `© 2026 Allodia` is pushed as `2026 Allodia`. Handing it the symbol is how a listing ends
    up reading "© © 2026 Allodia"; visible only once it is live.
    """
    return fields["copyright"].lstrip("©").strip()


# -------------------------------------------------------------------------------------------
# Talking to App Store Connect, through `asc`
# -------------------------------------------------------------------------------------------


def run_asc(args, capture=True):
    """Run `asc`, returning parsed JSON (or `None` when the command prints nothing useful)."""
    if shutil.which("asc") is None:
        raise ListingError(
            "the `asc` CLI is not on PATH. Install App-Store-Connect-CLI and run `asc auth login` "
            "once; --offline needs neither and renders every field locally."
        )
    command = ["asc"] + [str(item) for item in args]
    finished = subprocess.run(
        command, capture_output=capture, text=True, check=False  # noqa: S603; fixed argv, no shell
    )
    if finished.returncode != 0:
        detail = (finished.stderr or finished.stdout or "").strip() if capture else ""
        raise ListingError(f"`{' '.join(command)}` failed:\n{detail}")
    if not capture:
        return None
    text = (finished.stdout or "").strip()
    if not text:
        return None
    try:
        return json.loads(text)
    except ValueError:
        return text


def _rows(payload):
    """`asc` returns either a bare list or an API document; both mean the same thing here."""
    if isinstance(payload, dict):
        return payload.get("data") or []
    return payload or []


def resolve_app(identifier) -> str:
    """The App Store Connect app id, from `--app-id` or the bundle id in `project.yml`."""
    if identifier:
        return str(identifier)
    wanted = bundle_id()
    for row in _rows(run_asc(["apps", "list", "--bundle-id", wanted, "--output", "json"])):
        attributes = row.get("attributes", row)
        if attributes.get("bundleId") == wanted:
            return str(row.get("id") or attributes.get("id"))
    raise ListingError(f"no app in App Store Connect with bundle id {wanted}")


def resolve_version(app: str, version: str, platform: str) -> str:
    """The version id for `version` on `platform`; the one this run's copy belongs to."""
    for row in _rows(run_asc(["versions", "list", "--app", app, "--output", "json"])):
        attributes = row.get("attributes", row)
        if attributes.get("versionString") == version and attributes.get("platform") == platform:
            return str(row.get("id") or attributes.get("id"))
    raise ListingError(
        f"App Store Connect has no {platform} version {version} for app {app}. Create it in the "
        "console (or pass --version for one that exists). This script fills a version in, it does "
        "not open one."
    )


def resolve_app_info(app: str) -> str:
    """The **editable** app-info record; where name and subtitle can still be changed.

    An app carries two: the live one, and the one an in-progress submission edits. `asc` refuses to
    guess between them, and guessing wrong is worse than the error: a write to the live record is
    rejected, but reading the wrong one would report the live copy as though it were the draft.
    """
    rows = _rows(run_asc(["apps", "info", "list", "--app", app, "--output", "json"]))
    editable = [
        row
        for row in rows
        if (row.get("attributes", row).get("state") or "").upper() != LIVE_STATE
    ]
    if len(editable) == 1:
        return str(editable[0].get("id"))
    if len(rows) == 1:
        return str(rows[0].get("id"))
    states = ", ".join(
        f"{row.get('id')}[{row.get('attributes', row).get('state')}]" for row in rows
    )
    raise ListingError(
        f"cannot tell which app-info record is editable ({states}). Pass --app-info explicitly."
    )


def push_metadata(app, app_info, version, platform, directory, apply_it, report):
    """`asc metadata push`; a dry run first, then the real one only when asked."""
    base = [
        "metadata", "push",
        "--app", app,
        "--app-info", app_info,
        "--version", version,
        "--platform", platform,
        "--dir", str(directory),
        "--output", "table",
    ]  # fmt: skip
    report(run_asc(base + ["--dry-run"]) or "(asc reported no metadata changes)")
    if apply_it:
        report(run_asc(base) or "(asc reported no metadata changes)")


def push_copyright(version_id, text, apply_it, report):
    """The copyright is a version field, not a localization; so it is its own call."""
    report(f"  copyright  {text!r}")
    if apply_it:
        run_asc(["versions", "update", "--version-id", version_id, "--copyright", text])


def push_screenshots(app, version_id, platform, slot, staged_root, apply_it, report):
    """Replace each locale's gallery for one slot from the staged tree.

    `--replace` rather than an append: a gallery that gained a screen would otherwise end up with
    the old set *and* the new one, in an order nobody chose. It is also the destructive one, which
    is why screenshots are opt-in (`--screenshots DIR`) rather than part of every run.

    `--confirm` rides with `--replace` on the real run, and only there: `asc` refuses to delete an
    existing screenshot without it, while `--replace --dry-run` previews the deletions and must not
    carry it. A gallery that is still empty replaces nothing, so the first push of a version
    succeeds without it and every later one does not; the failure arrives the release *after* the
    one that would have shown it.
    """
    args = [
        "screenshots", "upload",
        "--app", app,
        "--version-id", version_id,
        "--platform", platform,
        "--device-type", slot.device_type,
        "--path", str(staged_root),
        "--replace",
        "--output", "table",
    ]  # fmt: skip
    report(run_asc(args + ["--dry-run"]) or "(asc reported no screenshot changes)")
    if apply_it:
        report(run_asc(args + ["--confirm"]) or "(asc reported no screenshot changes)")


# -------------------------------------------------------------------------------------------
# CLI
# -------------------------------------------------------------------------------------------


def resolve_slots(directories, platform):
    """Match each `--screenshots DIR` to the gallery it feeds, by the directory's own name.

    The capture directories are named for the device `showcase.sh` photographed (`macos`,
    `iphone`, `ipad`), and that name is the only thing saying which slot a set of PNGs belongs in.
    Guessing is not an option worth having: Apple takes a fixed set of sizes per slot, so the wrong
    one fails on every file, and a gallery left alone is safer than one filled with another device.
    """
    by_name = {slot.capture: slot for slot in SCREENSHOT_SLOTS[platform]}
    resolved = []
    for directory in directories:
        name = Path(directory).name
        if name not in by_name:
            raise ListingError(
            f"{directory} does not name a {platform} gallery: its directory is called "
                f"{name!r}. Point --screenshots at "
                f"{' or '.join(sorted(by_name))} (the capture directories showcase.sh writes)."
            )
        resolved.append((by_name[name], Path(directory)))
    return resolved


def describe(listings) -> str:
    """The human-readable plan: what each language would carry, and how long it is."""
    lines = [f"{len(listings)} language(s):"]
    for listing in listings:
        note = ";" if listing.whats_new is None else f"{len(listing.whats_new)} chars"
        lines.append(
            f"  {listing.store_locale:<6} {listing.subtitle!r} · description "
            f"{len(listing.description)} chars · keywords {len(listing.keywords)} · "
            f"what's new {note}"
        )
    return "\n".join(lines)


def write_paste_files(listings, out, version) -> None:
    """`--out`: one text file per language, for the console fields this does not push."""
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    for listing in listings:
        body = [
            f"Name:         {listing.name}",
            f"Subtitle:     {listing.subtitle}",
            f"Promotional:  {listing.promotional}",
            f"Keywords:     {listing.keywords}",
            f"Marketing:    {listing.marketing_url}",
            f"Support:      {listing.support_url}",
            f"Privacy:      {listing.privacy_policy_url}",
            "",
            "Description:",
            listing.description,
        ]
        if listing.whats_new is not None:
            body += ["", f"What's new ({version}):", listing.whats_new]
        (out / f"{listing.store_locale}.txt").write_text(
            "\n".join(body) + "\n", encoding="utf-8"
        )


def parse_args(argv=None):
    parser = argparse.ArgumentParser(
        description="Push the resolved store listing into an App Store listing (macOS or iOS/iPadOS).",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "-l", "--language", action="append", metavar="LOCALE",
        help="only this catalog locale; repeatable. Default: every language the catalog ships.",
    )  # fmt: skip
    parser.add_argument(
        "--platform", choices=ASC_PLATFORMS, default="MAC_OS",
        help="which App Store version to fill in. Default: MAC_OS.",
    )  # fmt: skip
    parser.add_argument(
        "--screenshots", metavar="DIR", nargs="+",
        help="replace galleries from DIR/<locale>-<screen>.png; repeatable, one directory per "
        "gallery. The directory's name picks the slot (macos; iphone and ipad for IOS). Omit to "
        "leave images untouched.",
    )  # fmt: skip
    parser.add_argument(
        "--release-notes", metavar="X.Y.Z",
        help="also push that release's What's new, from the section matching --platform.",
    )  # fmt: skip
    parser.add_argument("--apply", action="store_true", help="actually write it (default: plan).")
    parser.add_argument("--app-id", metavar="ID", help="App Store Connect app id.")
    parser.add_argument("--app-info", metavar="ID", help="app-info record to edit.")
    parser.add_argument("--version", metavar="X.Y.Z", help="the version to fill in (default: /VERSION).")
    parser.add_argument("--offline", action="store_true", help="never touch the network.")
    parser.add_argument("--out", metavar="DIR", help="write the rendered fields here as text.")
    parser.add_argument(
        "--listing", metavar="PATH", default=str(listing_path()),
        help="the store copy to read. Defaults to the resolved branding/<brand>-listing.md.",
    )  # fmt: skip
    return parser.parse_args(argv)


def main(argv=None) -> int:
    args = parse_args(argv)
    version = args.version or current_version()

    try:
        limits = parse_limits(LIMITS_PATH.read_text(encoding="utf-8"))
        locales = resolve_locales(args.language)
        listings = load_listings(
            listing_md=args.listing,
            locales=locales,
            version=args.release_notes,
            platform=args.platform,
        )
        # One set of listings per gallery: the same copy, carrying that slot's captures. Each is
        # measured against its own slot's sizes, because Apple's are per slot.
        galleries = []
        for slot, directory in resolve_slots(args.screenshots or (), args.platform):
            shots, skipped = collect_screenshots(directory, locales)
            if skipped:
                print(f"Ignored in {directory} (not <locale>-<screen>.png): {', '.join(skipped)}")
            galleries.append((slot, load_listings(
                listing_md=args.listing,
                locales=locales,
                version=args.release_notes,
                screenshots=shots,
                platform=args.platform,
            )))
        rights = apple_copyright(shared_fields(text))
    except (DocumentShapeError, ListingError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    problems = [problem for listing in listings for problem in measure(listing, limits)]
    problems += [
        problem
        for slot, staged in galleries
        for listing in staged
        for problem in measure_screenshots(listing, slot)
    ]
    if problems:
        print("Copy App Store Connect would refuse:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(
            "\nERROR: nothing was pushed. Trim the copy in the resolved listing (or the note in "
            "docs/changelog/released/) and re-run. The limits come from docs/store-listing.md's "
            "'Field limits' table, the same numbers the store-copy CI job enforces.",
            file=sys.stderr,
        )
        return 1

    print(f"App Store listing ({args.platform}): version {version}")
    print(describe(listings))
    for slot, staged in galleries:
        counts = ", ".join(
            f"{listing.store_locale} {len(listing.screenshots)}" for listing in staged
        )
        print(f"  {slot.device_type} ({slot.capture}): {counts}")
    if args.out:
        write_paste_files(listings, args.out, version)
        print(f"\nWrote the rendered fields to {args.out}")
    if args.offline:
        print("\n--offline: nothing was sent to Apple.")
        return 0

    print("")
    broken = check_urls(listings, print)
    if broken:
        for problem in broken:
            print(f"ERROR: {problem}", file=sys.stderr)
        print(
            "\nNothing was pushed. Apple opens the support URL during review, so a listing that "
            "names a dead page is a rejection. Fix the 'Shared fields' table in "
            "the resolved listing, or publish the page.",
            file=sys.stderr,
        )
        return 1

    try:
        app = resolve_app(args.app_id)
        app_info = args.app_info or resolve_app_info(app)
        version_id = resolve_version(app, version, args.platform)
        print(f"\napp {app} · app-info {app_info} · version {version_id}\n")
        with tempfile.TemporaryDirectory(prefix="appstore-metadata-") as scratch:
            write_metadata(listings, scratch, version)
            push_metadata(app, app_info, version, args.platform, scratch, args.apply, print)
            push_copyright(version_id, rights, args.apply, print)
            for slot, staged_listings in galleries:
                staged = Path(scratch) / "screenshots" / slot.device_type
                stage_screenshots(staged_listings, staged)
                print(f"\n{slot.device_type}:")
                push_screenshots(
                    app, version_id, args.platform, slot, staged, args.apply, print
                )
    except ListingError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1

    if args.apply:
        print(
            "\nWritten. Review it in App Store Connect and press Submit there. Pricing, the age "
            "rating, the App Privacy answers and the build selection stay console-side."
        )
    else:
        print("\nPlan only: nothing was written. Re-run with --apply.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
