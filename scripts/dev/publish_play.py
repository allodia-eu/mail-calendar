#!/usr/bin/env python3
"""Push the store listing and the release note to Google Play, from the docs that already own them.

    scripts/dev/publish_play.py --dry-run     # build the payloads, print them, touch no network
    scripts/dev/publish_play.py               # upload + validate against Play, then DISCARD the edit
    scripts/dev/publish_play.py --commit      # ...and actually publish it
    scripts/dev/publish_play.py --commit --screenshots showcase-screenshots/android
    scripts/dev/publish_play.py --show        # read-only: what is live, and how it differs

The resolved store listing is the single source of the description, and `docs/changelog/released/` of
the release note. Both are already parsed and length-checked by `scripts/ci/check_store_copy_length.py`
; which exists because six of the seven description bodies were over Apple's and Play's cap for
several releases, and the only gate that would ever have said so was a console rejecting a
submission. Pasting those same fields into the Play Console by hand puts a human back in the middle
of exactly that: seven languages x three fields, re-typed at the moment you are least inclined to
proofread. This closes the loop. [`play_listing.py`](play_listing.py) builds the payloads; from the
same parsers the CI check uses, so this cannot ship a field that check believed it had cleared; and
this file talks to Play.

**The default run is a rehearsal, not a publish.** AGENTS.md's "a store upload is a remote gate too"
rule says to assert what the remote gate checks *before* uploading, because a rejection burns a
build number and a slow round-trip. So this measures every field locally first (the same limits
table `store-copy` enforces), then; without `--commit`; opens a Play edit, uploads everything,
asks Play to `validate` it, and **deletes the edit**. That is a true server-side dry run: it proves
Play accepts the payload without changing what users see. `--commit` is the only thing that
publishes, and `--dry-run` goes no further than your own machine.

**What it writes, per language:**

| Play field         | Comes from                                                              |
|--------------------|-------------------------------------------------------------------------|
| `title`            | `store-listing.md` -> "Product name / title"                             |
| `shortDescription` | `store-listing.md` -> "Google Play; Short description"                  |
| `fullDescription`  | `store-listing.md` -> "Shared description; <language>", with `{KEYSTORE}`|
|                    | substituted with Play's value (the Android Keystore)                     |
| release notes      | `docs/changelog/released/<VERSION>.md`, the section covering `android`    |
| screenshots        | `--screenshots <dir>`; the capture directory, one gallery per form factor|
| feature graphic    | `feature-graphic-<locale>.png` in that same directory                    |

**Images are opt-in and replace what is there.** `--screenshots showcase-screenshots/android` reads
the flat capture directory `showcase.sh` writes and fills Play's four slots per language: phone,
7-inch tablet, 10-inch tablet, feature graphic. Each slot is **deleted before it is filled**,
because Play *appends*; a second run would otherwise leave twelve screenshots in a gallery that
shows the first eight, half of them from the previous release, and report success. Deletion is per
slot rather than per language, so a phone-only capture cannot empty the tablet galleries; an empty
tablet slot is what makes Play file the app as phone-only on large screens.

Pricing, data-safety answers, content ratings and the track's rollout stay out of scope: none of
them is owned by a document in this repo, so a script that "synced" them would be inventing state
rather than mirroring it.

**Release notes attach to a release, not to the app.** Play has no free-standing "what's new" field
; the text hangs off a release inside a track. So the note is written onto whichever track holds the
release whose `versionCode` matches `/VERSION` (the derived `major*10^7 + minor*10^5 + patch*10^3`
from `docs/versioning.md`). If no track carries that build yet, that is reported and nothing is
uploaded; `--skip-notes` publishes the listing alone.

**Setup.** Play's API needs a Google Cloud service account granted access in Play Console ->
Users and permissions, and the `google-api-python-client` SDK:

    python3 -m pip install google-api-python-client google-auth
    export GOOGLE_PLAY_SERVICE_ACCOUNT_JSON=/path/to/service-account.json

The key is a credential: keep it out of the repo and out of your shell history. Nothing here logs
its contents, and the only thing printed about it is the path you passed.

**Python 3.9-compatible**, like its siblings; `/usr/bin/python3` on a stock macOS is 3.9, and a
release tool that crashes on the host it is written for is a tool nobody runs. The SDK imports are
**deferred into the call that needs them**, so `--dry-run`, `--help` and the unit tests all work on
a machine that has never installed it.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
# Both, explicitly. `play_listing` also puts `scripts/ci` on the path, but relying on that would
# make this file's imports depend on the *order* they happen to run in; and the unit tests set up
# their own path, so the breakage would only ever show up when a human ran the CLI.
sys.path.insert(0, str(_HERE))
sys.path.insert(0, str(_HERE.parents[1] / "scripts" / "ci"))

from changelog_fragments import DocumentShapeError  # noqa: E402
from play_listing import (  # noqa: E402
    LIMITS_PATH,
    SLOT_NAMES,
    SLOT_ORDER,
    ListingError,
    PublishError,
    compare_live,
    android_release_notes,
    android_version_code,
    current_version,
    image_payloads,
    listing_payloads,
    measure,
    listing_path,
    package_name,
    parse_limits,
)

SCOPE = "https://www.googleapis.com/auth/androidpublisher"
KEY_ENV = "GOOGLE_PLAY_SERVICE_ACCOUNT_JSON"


# ---------------------------------------------------------------------------------------------
# Shaping the Play calls; pure, so the track logic is testable without a network
# ---------------------------------------------------------------------------------------------


def select_release(tracks, version_code, requested=None):
    """`(track name, release index)` for the release carrying `version_code`.

    Play has no app-level "what's new": the text belongs to a release inside a track, so the note
    can only be written once a build is somewhere. Searching by `versionCode` rather than taking a
    track name means the note lands on the build `/VERSION` describes; pointing `--track` at a
    track holding a *different* build would otherwise rewrite that build's notes, silently.
    """
    # Play returns `versionCodes` as an array of **strings** (`int64` is not JSON-safe), while the
    # formula in `docs/versioning.md` yields an int. Comparing the two directly never matches, and
    # the failure is a liar: "no release contains versionCode 300000" reads as "the build is not
    # uploaded yet" for a build that is sitting right there. So both sides are normalised.
    wanted = str(version_code)
    candidates = []
    for track in tracks:
        name = track.get("track")
        if requested is not None and name != requested:
            continue
        for index, release in enumerate(track.get("releases") or []):
            if wanted in [str(code) for code in (release.get("versionCodes") or [])]:
                candidates.append((name, index))
    if not candidates:
        scope = f"track '{requested}'" if requested else "any track"
        raise PublishError(
            f"no release in {scope} contains versionCode {version_code}. Upload the build first, "
            "or pass --skip-notes to publish only the listing."
        )
    if len(candidates) > 1:
        where = ", ".join(sorted({name for name, _ in candidates}))
        raise PublishError(
            f"versionCode {version_code} appears in more than one track ({where}). Say which with "
            "--track, so the note is not written to a rollout you did not mean."
        )
    return candidates[0]


def with_release_notes(track, index, notes):
    """`track` with the notes of release `index` replaced. The original is left untouched.

    The whole track object goes back to Play on update, so everything else about the release; its
    status, its `versionCodes`, a staged rollout's `userFraction`; has to survive the round trip
    verbatim. Rebuilding a minimal body instead is how a 20% rollout silently becomes 100%.
    """
    updated = json.loads(json.dumps(track))
    updated["releases"][index]["releaseNotes"] = [
        {"language": tag, "text": text} for tag, text in notes.items()
    ]
    return updated


# ---------------------------------------------------------------------------------------------
# Talking to Play
# ---------------------------------------------------------------------------------------------


def build_service(key_file):
    """An `androidpublisher` v3 client. The SDK is imported here so `--dry-run` never needs it."""
    try:
        from google.oauth2 import service_account
        from googleapiclient.discovery import build
    except ImportError as error:
        raise PublishError(
            "the Google API SDK is not installed. A plain `pip install` fails on a "
            "PEP 668 interpreter (Homebrew, most Linux distros), so use a venv:\n"
            "    python3 -m venv ~/.venvs/allodia-play\n"
            "    ~/.venvs/allodia-play/bin/pip install google-api-python-client google-auth\n"
            "    ~/.venvs/allodia-play/bin/python scripts/dev/publish_play.py ...\n"
            f"(--dry-run needs neither, and builds every payload offline.)  [{error}]"
        )
    path = Path(key_file)
    if not path.is_file():
        raise PublishError(
            f"no service-account key at {path}. Create one in Google Cloud, grant it access in "
            f"Play Console -> Users and permissions, and point {KEY_ENV} at the JSON."
        )
    credentials = service_account.Credentials.from_service_account_file(str(path), scopes=[SCOPE])
    # `cache_discovery=False`: the default file cache warns noisily and is useless for a script
    # that runs a handful of times a release.
    return build("androidpublisher", "v3", credentials=credentials, cache_discovery=False)


def credential_identity(key_file):
    """The service account's email address, for an error message.

    Reads only `client_email`; a public identifier, the thing you paste into Play Console. The
    private key in the same file is never read, printed, or logged.
    """
    try:
        return json.loads(Path(key_file).read_text(encoding="utf-8"))["client_email"]
    except (OSError, ValueError, KeyError, TypeError):
        return None


def explain_api_error(error, package, key_file):
    """The half of an authorization failure that Play does not tell you, or `None`.

    Play answers a service account that has never been granted anything with `403 The caller does
    not have permission`; four words that do not say *which* caller, *which* app, or where the
    grant is made. Enabling the API in Google Cloud and granting the identity access to the app in
    Play Console are two separate steps in two separate consoles, and the first one succeeding is
    what makes the second one easy to believe is done: the request reaches the API, is counted in
    the Cloud metrics, and is then refused by Play. So spell out the step that is actually missing,
    and name the identity to grant.
    """
    status = getattr(getattr(error, "resp", None), "status", None)
    if status not in (401, 403, 404):
        return None
    who = credential_identity(key_file)
    identity = f"'{who}'" if who else "this service account"
    if status == 404:
        return (
            f"Play has no app '{package}' under the developer account {identity} can see. Check the "
            "package name, and that the app exists in the same account the key was granted on."
        )
    return (
        f"{identity} is authenticated but not authorized on '{package}'.\n"
        "Enabling the API in Google Cloud is NOT the grant: that happens in Play Console:\n"
        "  1. Setup -> API access: confirm the linked Cloud project is the key's own project.\n"
        "  2. Grant access to the service account (or Users and permissions -> Invite new users).\n"
        "  3. Add this app, with 'View app information', 'Manage store presence' (the listing,\n"
        "     screenshots and feature graphic) and a release permission (the release notes).\n"
        "  4. Save, then wait a minute. Grants take effect asynchronously."
    )


def upload_images(edits, package, edit_id, images, report):
    """Replace each language's galleries with what the capture directory holds.

    `deleteall` before uploading, per slot, because Play **appends**: uploading six screenshots
    over a gallery that already has six leaves twelve, and the store shows the first eight. And it
    is per slot rather than per language, so a run that carries no tablet captures cannot empty the
    tablet gallery; an empty tablet slot is what makes Play file the app as phone-only.

    Order is upload order: Play has no index field, so the sequence here *is* the gallery.
    """
    from googleapiclient.http import MediaFileUpload  # deferred: --dry-run never needs the SDK

    for tag, slots in images.items():
        for image_type, paths in slots.items():
            edits.images().deleteall(
                packageName=package, editId=edit_id, language=tag, imageType=image_type
            ).execute()
            for path in paths:
                edits.images().upload(
                    packageName=package,
                    editId=edit_id,
                    language=tag,
                    imageType=image_type,
                    media_body=MediaFileUpload(str(path), mimetype="image/png"),
                ).execute()
            report(f"  images   {tag} · {SLOT_NAMES[image_type]}: {len(paths)}")


def live_state(edits, package, edit_id, tags):
    """What Play holds right now, per language: the listing copy and each gallery's size.

    A freshly opened edit reflects the live listing, so reading inside one; and deleting it again,
    reports the truth without writing anything.
    """
    from googleapiclient.errors import HttpError  # deferred: --dry-run never needs the SDK

    state = {}
    for tag in tags:
        try:
            listing = (
                edits.listings().get(packageName=package, editId=edit_id, language=tag).execute()
            )
        except HttpError as error:
            # A language Play has no listing for 404s here, and that is a different fault from an
            # empty gallery: reporting it as "0 screenshots" sends you looking at the capture
            # directory, when what is missing is the listing an image would hang off.
            if error.resp.status != 404:
                raise
            state[tag] = {"missing": True, "listing": None, "images": {}}
            continue
        counts = {}
        for slot in SLOT_ORDER:
            found = (
                edits.images()
                .list(packageName=package, editId=edit_id, language=tag, imageType=slot)
                .execute()
            )
            counts[slot] = len(found.get("images") or [])
        state[tag] = {"missing": False, "listing": listing, "images": counts}
    return state


def show(service, package, listings, images, report):
    """Print the live listing beside what this repo would push. Writes nothing; returns the drift.

    Read-only by construction: the edit it opens to read through is deleted in `finally`, and it is
    never committed. `--commit` is what publishes; this only ever looks.
    """
    edits = service.edits()
    edit_id = edits.insert(packageName=package, body={}).execute()["id"]
    try:
        state = live_state(edits, package, edit_id, list(listings))
    finally:
        try:
            edits.delete(packageName=package, editId=edit_id).execute()
        except Exception:  # noqa: BLE001; cleanup, never the failure worth reporting
            pass
    lines, drift = compare_live(state, listings, images if images else None)
    for line in lines:
        report(line)
    return drift


def upload(service, package, listings, notes, version_code, track, commit, report, images=None):
    """Open an edit, write everything, validate; then commit it or throw it away.

    Discarding by default is the whole safety model: `edits.validate` runs Play's own checks on the
    exact payload, so a rehearsal proves the upload would be accepted without a user ever seeing it.
    Nothing outside an edit is mutated until `commit`, so a crash halfway through leaves the live
    listing untouched.
    """
    edits = service.edits()
    edit_id = edits.insert(packageName=package, body={}).execute()["id"]
    committed = False
    try:
        for tag, payload in listings.items():
            edits.listings().update(
                packageName=package, editId=edit_id, language=tag, body=payload
            ).execute()
            report(f"  listing  {tag}")

        if images:
            upload_images(edits, package, edit_id, images, report)

        if notes:
            tracks = edits.tracks().list(packageName=package, editId=edit_id).execute()
            name, index = select_release(tracks.get("tracks") or [], version_code, track)
            body = with_release_notes(
                next(item for item in tracks["tracks"] if item.get("track") == name), index, notes
            )
            edits.tracks().update(
                packageName=package, editId=edit_id, track=name, body=body
            ).execute()
            report(
                f"  notes    {len(notes)} language(s) -> track '{name}' "
                f"(versionCode {version_code})"
            )

        edits.validate(packageName=package, editId=edit_id).execute()
        report("  validate OK: Play accepts this edit")

        if commit:
            edits.commit(packageName=package, editId=edit_id).execute()
            committed = True
            report("  commit   PUBLISHED")
    finally:
        if not committed:
            # Best-effort: an edit left open expires on its own, and a delete that fails must not
            # mask the real error that brought us into `finally`.
            try:
                edits.delete(packageName=package, editId=edit_id).execute()
            except Exception:  # noqa: BLE001; cleanup, never the failure worth reporting
                pass
    return committed


# ---------------------------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------------------------


def describe(listings, notes, measured, images=None, notes_read=True):
    """The human-readable plan: what goes where, and how much room each field has left."""
    lines = ["", f"{len(listings)} listing(s):"]
    for tag, payload in listings.items():
        lines.append(f"  {tag}: {payload['title']!r} / {len(payload['fullDescription'])} chars")
    lines.append("")
    if images:
        total = sum(len(paths) for slots in images.values() for paths in slots.values())
        lines.append(f"{total} image(s) across {len(images)} language(s):")
        for tag, slots in images.items():
            summary = ", ".join(f"{len(paths)} {SLOT_NAMES[slot]}" for slot, paths in slots.items())
            lines.append(f"  {tag}: {summary}")
        lines.append("")
        lines.append("Each slot is REPLACED: Play appends, so the old gallery is deleted first.")
    else:
        lines.append("No images: pass --screenshots <dir> to replace the galleries too.")
    lines.append("")
    if notes:
        lines.append(f"{len(notes)} release note(s):")
        for tag, note in notes.items():
            first = note.splitlines()[0] if note.splitlines() else ""
            lines.append(f"  {tag}: {len(note)} chars: {first[:60]}...")
    elif not notes_read:
        # `--show` never loads them, so it may not report on what they say. Claiming there is no
        # note would be an assertion about docs this run did not read.
        lines.append("Release note: not read (--show reports the live listing only).")
    else:
        lines.append("No release note: this release has no section reaching Google Play.")
    lines.append("")
    lines.append("Tightest fields:")
    lines.extend(str(item) for item in sorted(measured, key=lambda item: item.margin)[:3])
    return "\n".join(lines)


def parse_args(argv):
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--key", default=os.environ.get(KEY_ENV), help=f"service-account JSON (default: ${KEY_ENV})"
    )
    parser.add_argument("--package", help="applicationId (default: read from build.gradle.kts)")
    parser.add_argument("--version", help="X.Y.Z whose release note to upload (default: /VERSION)")
    parser.add_argument("--track", help="track holding the release (default: found by versionCode)")
    parser.add_argument("--skip-notes", action="store_true", help="publish the listing only")
    parser.add_argument(
        "--screenshots",
        help="capture directory to replace every language's galleries from "
        "(showcase-screenshots/android)",
    )
    parser.add_argument(
        "--dry-run", action="store_true", help="build and print the payloads; no network"
    )
    parser.add_argument(
        "--show",
        action="store_true",
        help="read-only: print what Play holds now and how it differs from these docs",
    )
    parser.add_argument(
        "--commit", action="store_true", help="actually publish (default: validate, then discard)"
    )
    return parser.parse_args(argv)


def main(argv=None):
    args = parse_args(argv)

    if args.show and (args.commit or args.dry_run):
        other = "--commit" if args.commit else "--dry-run"
        print(f"ERROR: --show reads; {other} does not. Pick one.", file=sys.stderr)
        return 2

    try:
        listing = listing_path().read_text(encoding="utf-8")
        limits = parse_limits(LIMITS_PATH.read_text(encoding="utf-8"))
        package = args.package or package_name()
        version = args.version or current_version()
        version_code = android_version_code(version)
        listings = listing_payloads(listing)
        # `--show` reads the listing and the galleries, neither of which involves a release note,
        # so it must not fail on a version that has none yet.
        notes = {} if args.skip_notes or args.show else android_release_notes(version)
        measured = measure(listings, notes, limits)
        images = image_payloads(args.screenshots) if args.screenshots else {}
    except (DocumentShapeError, ListingError, PublishError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2

    over = [item for item in measured if not item.fits]
    if over:
        print("Copy Google Play would reject:", file=sys.stderr)
        for item in sorted(over, key=lambda item: item.where):
            print(item, file=sys.stderr)
        print(
            "\nERROR: nothing was uploaded. Trim the copy in the resolved listing (or the note in "
        "docs/changelog/released/) and re-run. The limits come from docs/store-listing.md's "
            "'Field limits' table, the same numbers CI's store-copy check enforces.",
            file=sys.stderr,
        )
        return 1

    print(f"Google Play: {package}  version {version} (versionCode {version_code})")
    print(describe(listings, notes, measured, images, notes_read=not args.show))

    if args.dry_run:
        print("\n--dry-run: nothing was sent to Google.")
        return 0

    if not args.key:
        print(
            f"\nERROR: no service-account key. Pass --key or set {KEY_ENV}. "
            "(--dry-run builds everything offline and needs neither.)",
            file=sys.stderr,
        )
        return 2

    print("")
    try:
        service = build_service(args.key)
        if args.show:
            print("Live on Google Play:")
            drift = show(service, package, listings, images, print)
            if not drift:
                print("\nThe live listing matches these docs. Nothing to push.")
                return 0
            print("\nDrift: the live listing is not what this repo says it should be:")
            for item in drift:
                print(f"  {item}")
            print(
                "\nRe-run with --commit"
                + (" --screenshots <dir>" if images else "")
                + " to make Play match the docs.  (--show wrote nothing.)"
            )
            return 1
        committed = upload(
            service, package, listings, notes, version_code, args.track, args.commit, print, images
        )
    except PublishError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    except Exception as error:  # noqa: BLE001; the SDK's HttpError carries Play's own message
        print(f"ERROR: Google Play rejected the edit: {error}", file=sys.stderr)
        explanation = explain_api_error(error, package, args.key)
        if explanation:
            print(f"\n{explanation}", file=sys.stderr)
        return 1

    if committed:
        print("\nPublished. Play may take a few hours to show the new copy.")
    else:
        print(
        "\nRehearsal only: the edit validated and was discarded, so nothing changed. "
            "Re-run with --commit to publish it."
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
