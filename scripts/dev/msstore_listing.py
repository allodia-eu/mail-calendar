#!/usr/bin/env python3
"""Push the resolved store listing (`branding/<brand>-listing.md`) into the Microsoft Store listing;
all seven languages, one run.

    scripts/dev/msstore_listing.py                          # plan only: diff the console against the doc
    scripts/dev/msstore_listing.py --apply                  # write the draft submission, do NOT submit it
    scripts/dev/msstore_listing.py --apply --screenshots showcase-screenshots/windows
    scripts/dev/msstore_listing.py --apply --release-notes 0.3.0
    scripts/dev/msstore_listing.py --apply --package clients/windows/Mailcal/AppPackages/Mailcal_0.3.0.0_x64_arm64_bundle.msixupload
    scripts/dev/msstore_listing.py --offline --out /tmp/listing   # no network: paste-ready text files

Partner Center asks for a title, a description, up to twenty product features and a screenshot
gallery **per language**, and we ship seven; so a copy change is seven form fills, each of which
can differ from the file it was copied from without anyone noticing. The resolved listing
(`branding/<brand>-listing.md`, via `brand.listing_source()`) is the source of truth for that copy;
this makes the console agree with it.

**Nothing is written without `--apply`, and nothing is submitted without `--commit`.** The default
run reads the current listing, prints a per-language diff, and exits. `--apply` creates (or reuses)
the in-progress submission and writes the copy into it; it stops there, so the last look before
certification is still yours, in Partner Center. `--commit` is the one that sends it, and it is
never implied.

**Read `msstore_api.py`'s docstring before the first run.** It carries the one-time setup (an Entra
ID app with the Manager role, plus one hand-made submission so the app has age ratings) and the rule
that bites: once a submission has been pushed through the API, editing *that* submission by hand in
Partner Center can leave it in a state the API cannot update or commit. Reviewing it and pressing
**Commit with `--commit`, never with Partner Center's Submit button.** Twice on 0.5.0 the console's
Submit discarded a staged listing and committed the copy it had loaded; which predated the API's
`PUT`; so the submission certified carrying the *previous* release's notes and gallery, in all
seven languages, with no error anywhere. The one recorded run that worked (2026-08-03) committed
through the API. Reading the submission in the console is safe; pressing its Submit is what throws
the push away. `--package` exists so a release needs no console visit at all.

**Screenshots are opt-in** (`--screenshots DIR`, holding `<locale>-<screen>.png`; the layout of
`showcase-screenshots/windows/`). Without it, the galleries already in the submission are left
exactly as they are, which is what you want when only the copy changed, and what you need while
only `en` and `nl` have Windows captures. Pass a directory holding one language and only that
language's gallery is replaced. **A replaced gallery is not visible until the submission is
committed**; the images ride along as `PendingUpload` until then; so a screenshot run that ends
at `--apply` is finished in Partner Center by pressing Submit.

**What's new is opt-in too** (`--release-notes X.Y.Z`), read from `docs/changelog/released/`, taking
only the sections whose platforms reach the Microsoft Store. It is separate from the copy because
the two move on different schedules: copy changes when the product does, release notes when a
version ships.
"""

from __future__ import annotations

import argparse
import json
import sys
import tempfile
import time
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from changelog_fragments import DocumentShapeError  # noqa: E402
from check_store_copy_length import LIMITS_PATH, listing_path, parse_limits  # noqa: E402
from msstore_api import Credentials, PartnerCenter, PartnerCenterError  # noqa: E402
from msstore_payload import (  # noqa: E402
    STORE_LANGUAGES,
    ListingError,
    collect_screenshots,
    load_listings,
    load_package,
    measure,
    merge,
    merge_package,
    render_plan,
    resolve_locales,
)

# How long to watch a commit before handing the user back their prompt. The Store takes hours to
# certify; this only waits for the *commit* to be accepted, which is the part that can fail on
# something we did.
POLL_SECONDS = 15
POLL_ATTEMPTS = 8
SETTLED = ("PreProcessing", "Certification", "Release", "Published", "CommitFailed")

# A draft that Partner Center created can be **read** by this API and not **written** by it: the
# service reports its own submission state as 'None' and refuses every PUT. Proven rather than
# assumed; an exact echo of what GET returned is refused identically to a real edit, so it is the
# submission and not the payload. The service's advice ("delete the submission and create a new
# one") is the one thing not to do blindly: a new submission clones the *last published* one, so
# any package or screenshot staged only in the draft is gone.
UNWRITABLE_DRAFT = "state 'None'"
UNWRITABLE_ADVICE = """
This draft was created in Partner Center, and this API can read it but not write to it.

  Do NOT delete it to "fix" the state unless you have checked what it holds: a replacement is a
  clone of the last PUBLISHED submission, so any package or screenshot staged only in this draft
  is lost with it.

  Finish this one by hand. `--offline --out DIR` writes every field as paste-ready text, and
  submit it. From then on this tool creates the submission itself, and the submissions it creates
  are ones it can update."""


def parse_args(argv=None):
    parser = argparse.ArgumentParser(
        description="Push the resolved store listing into the Microsoft Store listing.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument(
        "-l",
        "--language",
        action="append",
        metavar="LOCALE",
        help=f"only this catalog locale ({'/'.join(STORE_LANGUAGES)}); repeatable. Default: all.",
    )
    parser.add_argument(
        "--screenshots",
        metavar="DIR",
        help="replace galleries from DIR/<locale>-<screen>.png. Omit to leave images untouched.",
    )
    parser.add_argument(
        "--release-notes",
        metavar="X.Y.Z",
        help="also push that release's What's new, from docs/changelog/released/X.Y.Z.md.",
    )
    parser.add_argument(
        "--package",
        metavar="PATH",
        help="also upload this .msixupload, in the same write as the copy. Omit to leave the "
             "submission's packages alone.",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="write the draft submission (still does not submit it for certification).",
    )
    parser.add_argument(
        "--commit",
        action="store_true",
        help="submit the draft for certification. Implies --apply. This is the irreversible one.",
    )
    parser.add_argument(
        "--offline",
        action="store_true",
        help="never touch the network: render the copy and, with --out, write it to files.",
    )
    parser.add_argument(
        "--out",
        metavar="DIR",
        help="write the rendered per-language fields (and the merged submission JSON) here.",
    )
    parser.add_argument("--app-id", metavar="ID", help="Store ID; defaults to $MSSTORE_APP_ID.")
    parser.add_argument(
        "--env-file",
        metavar="PATH",
        help="read the MSSTORE_* credentials from this file instead of the default locations.",
    )
    parser.add_argument(
        "--listing",
        metavar="PATH",
        default=str(listing_path()),
        help="the listing to read. Defaults to whatever brand.listing_source() resolves to.",
    )
    return parser.parse_args(argv)


def write_files(directory, listings, submission=None):
    """Paste-ready output: one folder per listing language, plus the merged submission.

    The fallback for the day the API is unreachable, an account is mid-migration, or someone simply
    wants to read what would be sent before sending it.
    """
    root = Path(directory)
    root.mkdir(parents=True, exist_ok=True)
    for listing in listings:
        folder = root / listing.store_language
        folder.mkdir(exist_ok=True)
        (folder / "title.txt").write_text(listing.title + "\n", encoding="utf-8")
        (folder / "description.txt").write_text(listing.description + "\n", encoding="utf-8")
        (folder / "features.txt").write_text(
            "\n".join(listing.features) + "\n", encoding="utf-8"
        )
        (folder / "search-terms.txt").write_text(
            "\n".join(listing.search_terms) + "\n", encoding="utf-8"
        )
        (folder / "copyright.txt").write_text(listing.copyright + "\n", encoding="utf-8")
        if listing.release_notes:
            (folder / "release-notes.txt").write_text(
                listing.release_notes + "\n", encoding="utf-8"
            )
        if listing.screenshots:
            (folder / "screenshots.txt").write_text(
                "\n".join(f"{shot.zip_name}\t{shot.width}x{shot.height}" for shot in listing.screenshots)
                + "\n",
                encoding="utf-8",
            )
    if submission is not None:
        # The SAS upload URI is a short-lived write credential for the submission's blob container.
        # It is redacted rather than dumped: this file is written wherever the caller pointed
        # --out, which is as likely to be a repo folder as a temp dir.
        redacted = dict(submission)
        if redacted.get("fileUploadUrl"):
            redacted["fileUploadUrl"] = "<redacted SAS URI>"
        (root / "submission.json").write_text(
            json.dumps(redacted, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    return root


def build_archive(uploads, destination, package=None):
    """Zip the screenshots; and the package; under the names the submission entries refer to.

    One archive, because the submission has one `fileUploadUrl`: every file it is waiting on rides
    in together, images and bundle alike. The bundle is already a compressed container, so it is
    stored rather than deflated again; that is ~150 MB the run does not spend twice.
    """
    destination = Path(destination)
    with zipfile.ZipFile(destination, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for shot in uploads:
            archive.write(shot.path, arcname=shot.zip_name)
        if package is not None:
            archive.write(package.path, arcname=package.zip_name, compress_type=zipfile.ZIP_STORED)
    return destination


def current_submission(client, app):
    """The submission to diff against, and whether it is an editable draft.

    Reading is free of consequence, so the plan runs against whatever exists: the in-progress draft
    if there is one, else the last published listing, else nothing at all for an app that has never
    shipped.
    """
    pending = app.get("pendingApplicationSubmission")
    if pending:
        return client.submission(pending["id"]), True
    published = app.get("lastPublishedApplicationSubmission")
    if published:
        return client.submission(published["id"]), False
    return {"listings": {}}, False


def watch_commit(client, submission_id):
    """Poll until the commit is accepted or rejected; the failure mode we can still act on."""
    for attempt in range(POLL_ATTEMPTS):
        status = client.submission_status(submission_id)
        state = status.get("status", "unknown")
        print(f"  status: {state}")
        if state in SETTLED:
            details = status.get("statusDetails") or {}
            for error in details.get("errors") or []:
                print(f"    error: {error.get('code')} {error.get('details')}", file=sys.stderr)
            return state != "CommitFailed"
        if attempt + 1 < POLL_ATTEMPTS:
            time.sleep(POLL_SECONDS)
    print("  still committing: check Partner Center for the outcome.")
    return True


def run(args) -> int:
    limits = parse_limits(LIMITS_PATH.read_text(encoding="utf-8"))
    locales = resolve_locales(args.language)
    galleries, ignored = (
        collect_screenshots(args.screenshots, locales) if args.screenshots else ({}, [])
    )
    listings = load_listings(
        listing_md=args.listing,
        locales=locales,
        version=args.release_notes,
        screenshots=galleries,
    )

    # Measured before anything is sent. The console would reject an over-long field at submission,
    # after a draft has been written and a person has waited; AGENTS.md's "assert the invariant the
    # remote gate checks before uploading".
    # Loaded here, beside the copy measurement, so a wrong bundle is refused before a draft exists
    # rather than after 150 MB has gone over the wire.
    package = None
    if args.package:
        release = (REPO_ROOT / "VERSION").read_text(encoding="utf-8").strip()
        package = load_package(args.package, expected_version=release)

    problems = [problem for listing in listings for problem in measure(listing, limits)]
    if problems:
        print("Store copy the console would reject:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        print(
            "\nERROR: fix the resolved listing (and re-run scripts/ci/check_store_copy_length.py) "
            "before pushing.",
            file=sys.stderr,
        )
        return 1

    languages = ", ".join(listing.store_language for listing in listings)
    print(f"{listing_path().name} -> Microsoft Store - {len(listings)} language(s): {languages}")
    if args.screenshots:
        empty = [listing.store_language for listing in listings if not listing.screenshots]
        shots = sum(len(listing.screenshots) for listing in listings)
        print(f"Screenshots: {shots} file(s) from {args.screenshots}")
        if empty:
            print(f"  no captures for {', '.join(empty)}: those galleries are left untouched.")
        if ignored:
            # Named rather than counted: a passed-over file is usually a locale outside
            # --language, but it is equally a typo'd name, and from here the two look identical.
            # Listing them is what lets the reader tell one from the other.
            print(f"  ignored {len(ignored)} file(s) this run does not cover: {', '.join(ignored)}")

    if package:
        size_mb = package.path.stat().st_size / 1_048_576
        print(f"Package: {package.path.name} ({size_mb:.0f} MB, version {package.version})")

    if args.offline:
        if args.out:
            print(f"Wrote {write_files(args.out, listings)}")
        else:
            for listing in listings:
                print(f"  {listing.store_language}: {len(listing.description)} chars, "
                      f"{len(listing.features)} features, {len(listing.screenshots)} screenshot(s)")
        print("Offline: nothing was read from or written to Partner Center.")
        return 0

    credentials = Credentials.from_env(args.app_id, args.env_file)
    print(f"Credentials: {credentials.source}")
    client = PartnerCenter(credentials)
    app = client.app()
    print(f"App: {app.get('primaryName', '?')} ({app.get('id')})")

    apply_changes = args.apply or args.commit
    submission, editable = current_submission(client, app)
    if apply_changes and not editable:
        submission = client.create_submission()
        print(f"Opened a new draft submission: {submission.get('id')}")
    elif editable:
        print(f"Draft submission in progress: {submission.get('id')}")
    else:
        print("No draft in progress: the diff below is against the last published listing.")

    merged, plans, uploads = merge(submission, listings)
    package_change = None
    if package:
        merged, package_change = merge_package(merged, package)
    print()
    print(render_plan(plans, images_pushed=bool(args.screenshots)))
    print()

    if args.out:
        print(f"Wrote {write_files(args.out, listings, merged)}")

    changed = [plan for plan in plans if plan.changed]
    if not apply_changes:
        verb = "would change" if changed else "match the console already"
        print(f"Plan only: {len(changed)} of {len(plans)} language(s) {verb}. "
              "Nothing was written. Re-run with --apply to write the draft.")
        return 0
    submission_id = merged.get("id") or submission.get("id")
    nothing_to_write = not changed and not uploads
    if nothing_to_write:
        print("Nothing to write: the draft already says what the document says.")
        # Deliberately NOT a return: `--commit` submits *the draft*, and whether its copy happens
        # to differ from this document is a separate question. Short-circuiting here meant a draft
        # that had already been written by an earlier `--apply` could never be submitted by this
        # tool at all; it reported success and did nothing, which is the failure mode this file
        # exists to avoid.
        if not args.commit:
            return 0
    else:
        try:
            client.update_submission(submission_id, merged)
        except PartnerCenterError as error:
            if UNWRITABLE_DRAFT not in str(error):
                raise
            print(f"ERROR: {error}", file=sys.stderr)
            print(UNWRITABLE_ADVICE, file=sys.stderr)
            return 1
        print(f"Wrote the listing into submission {submission_id}.")

    if uploads or package:
        upload_url = merged.get("fileUploadUrl") or submission.get("fileUploadUrl")
        if not upload_url:
            print(
                "ERROR: the submission carries no fileUploadUrl, so the screenshots and package "
                "cannot be uploaded. Delete the draft in Partner Center and re-run.",
                file=sys.stderr,
            )
            return 1
        # Into a temp dir rather than the working tree: the archive is a build artifact of this
        # run, and a stray zip in the repo root is one `git add -A` away from being committed.
        archive = Path(tempfile.mkdtemp(prefix="mailcal-listing-")) / "listing-assets.zip"
        build_archive(uploads, archive, package=package)
        client.upload(upload_url, archive)
        sent = []
        if uploads:
            sent.append(f"{len(uploads)} screenshot(s)")
        if package:
            sent.append(package.path.name)
        print(f"Uploaded {' and '.join(sent)} ({archive}).")
        if not args.commit:
            print("  They stay pending until the submission is committed: re-run with "
                  "--commit, which is the only path that keeps them.")

    if not args.commit:
        print(
            "\nDraft written, NOT submitted. Read it in Partner Center if you like, then "
            "send it by re-running with --commit. Do NOT use Partner Center's Submit button: it "
            "commits the listing that page loaded rather than the one just staged, and it "
            "reverted 0.5.0's copy and gallery to the previous release's twice, silently."
        )
        return 0

    client.commit_submission(submission_id)
    print(f"Committed submission {submission_id}: it is now with the Store.")
    return 0 if watch_commit(client, submission_id) else 1


def main(argv=None) -> int:
    # A Windows console defaults to cp1252, and this tool's job is to print seven languages of
    # store copy at it. Without this, a plan containing a character cp1252 has no room for kills
    # the run with a UnicodeEncodeError; a listing tool that cannot print its own diff.
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8", errors="replace")
    args = parse_args(argv)
    try:
        return run(args)
    except ListingError as error:
        # Caught before DocumentShapeError so a mistyped path gets "fix the path", not "fix the
        # scraper"; and never the exit code that means "this tool fell behind the document".
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    except DocumentShapeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        print(
            "This tool reads the resolved listing through the same scraper as "
            "scripts/ci/check_store_copy_length.py. If the document changed shape, fix the "
            "scraper rather than the document. See scripts/dev/tests/test_msstore_payload.py.",
            file=sys.stderr,
        )
        return 2
    except PartnerCenterError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
