#!/usr/bin/env python3
"""Publish the documentation screenshots the manifest names, and prove they are published.

`scripts/dev/docs_images.py` encodes each capture, names the blob by the SHA-256 of its own bytes
and writes `docs/user/screenshots.json`. Git holds the manifest; it does not hold the images
(`docs/user-docs.md`). So the bytes have to travel: this uploads them to the website's
content-addressed store, which serves each one at `/docs-assets/<sha256>` for a year, immutably.

    python3 scripts/dev/docs_publish.py            # plan; what is missing, what would upload
    python3 scripts/dev/docs_publish.py --apply    # upload the missing blobs
    python3 scripts/dev/docs_publish.py --check    # exit 1 if the manifest names an unpublished image

Plan-by-default is the same shape `msstore_listing.py`, `appstore_listing.py` and `publish_play.py`
use: nothing leaves the machine until you say so.

`--check` is the half that matters after the fact, and it needs no local blobs; only the manifest
and the network. Content addressing makes the manifest a claim about the world ("the image for
`setup-detected` is these bytes"); without something asking the website whether that is true, the
first reader to open the page is the check. **Publish before the pages ship**: a merged doc whose
images are not up yet renders broken.

Where it publishes and what it authenticates with both come from the same credentials file; the
locations every publishing script here uses (`envfile`), `<repo>/.env` first:

    ALLODIA_DOCS_UPLOAD_TOKEN=…          must match the website's DOCS_ASSET_UPLOAD_TOKEN
    ALLODIA_DOCS_BASE_URL=…              optional; defaults to https://allodia.eu

`.env` is gitignored *and* listed in `.worktreeinclude`, so every new worktree gets a copy; the
findable file, right where you are working, that does not go stale.

Precedence is flag, then environment, then file, then the default: a file is a convenience, never
an override of what the person running the command just typed. The **token** is deliberately not a
flag; a secret on a command line lands in shell history and in every `ps` on the box; while the
target is `--base-url`, since pointing a run at a local server is a thing you do once, not a thing
you store.

A target that is not `https://allodia.eu` is **announced**, naming what chose it. A `docs.env` left
over from testing would otherwise send `--apply` somewhere harmless-looking and let `--check` pass
against a site nobody reads, while production still served nothing.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

import envfile

REPO_ROOT = Path(__file__).resolve().parents[2]

BLOBS = REPO_ROOT / "showcase-screenshots" / "docs-web"
MANIFEST = REPO_ROOT / "docs" / "user" / "screenshots.json"

DEFAULT_BASE_URL = "https://allodia.eu"
TOKEN_ENV = "ALLODIA_DOCS_UPLOAD_TOKEN"
BASE_URL_ENV = "ALLODIA_DOCS_BASE_URL"

# Where the token may live besides the environment. `.docs.env` beside the code is the convenient
# one; `~/.config/allodia/docs.env` is outside every checkout, which is what makes it the right
# default here; work happens in `.claude/worktrees/*`, and a repo-local file would need copying
# into each one.
ENV_FILE_VAR = "ALLODIA_DOCS_ENV_FILE"
ENV_FILE_LOCATIONS = envfile.locations_for("docs")

# Generous for a 40 KB image on a hotel connection, short enough that a wedged endpoint fails the
# run instead of hanging it.
TIMEOUT_SECONDS = 30


class PublishError(Exception):
    """The publish could not be completed, with a message meant for a human."""


# ---- what the manifest asks for ------------------------------------------------------------------


def referenced_hashes(manifest: Dict[str, object]) -> List[Tuple[str, str]]:
    """Every `(sha256, where)` the manifest names, de-duplicated, in a stable order.

    One image can be referenced by several ids; two platforms whose screens happen to encode
    identically, or the same screen in two locales when it carries no text. Content addressing
    dedupes those for free, so upload each distinct hash once and keep one human-readable location
    for the error message.
    """
    seen = {}  # type: Dict[str, str]
    images = manifest.get("images")
    if not isinstance(images, dict):
        raise PublishError(
            "the manifest has no `images` object: regenerate it with scripts/dev/docs_images.py"
        )
    for screen in sorted(images):
        platforms = images[screen]
        if not isinstance(platforms, dict):
            raise PublishError("images[%s] is not an object of platforms" % screen)
        for platform in sorted(platforms):
            locales = platforms[platform]
            if not isinstance(locales, dict):
                raise PublishError("images[%s][%s] is not an object of locales" % (screen, platform))
            for locale in sorted(locales):
                entry = locales[locale]
                digest = entry.get("sha256") if isinstance(entry, dict) else None
                if not isinstance(digest, str) or len(digest) != 64:
                    raise PublishError(
                        "images[%s][%s][%s] has no usable sha256" % (screen, platform, locale)
                    )
                seen.setdefault(digest, "%s / %s / %s" % (screen, platform, locale))
    return [(digest, seen[digest]) for digest in sorted(seen)]


# ---- talking to the store -----------------------------------------------------------------------


class HttpTransport:
    """The real thing: `HEAD /docs-assets/<hash>` and `PUT /api/docs-assets`."""

    def __init__(
        self, base_url: str, token: Optional[str] = None, base_url_source: str = "the default"
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        # Carried so `run` can name what chose a non-default target; see the note there.
        self.base_url_source = base_url_source

    def published(self, digest: str) -> bool:
        request = urllib.request.Request(
            "%s/docs-assets/%s" % (self.base_url, digest), method="HEAD"
        )
        try:
            with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
                return 200 <= response.status < 300
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return False
            raise PublishError(
                "HEAD /docs-assets/%s answered HTTP %d %s"
                % (digest[:12], error.code, error.reason)
            )
        except urllib.error.URLError as error:
            raise PublishError("cannot reach %s: %s" % (self.base_url, error.reason))

    def upload(self, data: bytes) -> Dict[str, object]:
        if not self.token:
            raise PublishError(
                "no upload token. It must match the website's DOCS_ASSET_UPLOAD_TOKEN. Put it in\n"
                "  %s\n"
                "as `%s=…` (chmod 600), or set $%s for one run."
                % (ENV_FILE_LOCATIONS[1], TOKEN_ENV, TOKEN_ENV)
            )
        request = urllib.request.Request(
            "%s/api/docs-assets" % self.base_url,
            data=data,
            method="PUT",
            headers={
                "Authorization": "Bearer %s" % self.token,
                "Content-Type": "image/webp",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            detail = error.read().decode("utf-8", "replace").strip()
            if error.code == 401:
                raise PublishError(
                    "the website rejected the upload token (401). $%s does not match its "
                    "DOCS_ASSET_UPLOAD_TOKEN." % TOKEN_ENV
                )
            if error.code == 404:
                raise PublishError(
                    "%s/api/docs-assets does not exist (404). The website 404s the upload endpoint "
                    "when DOCS_ASSET_UPLOAD_TOKEN is unset: configure it there first."
                    % self.base_url
                )
            raise PublishError("upload failed: HTTP %d %s %s" % (error.code, error.reason, detail))
        except urllib.error.URLError as error:
            raise PublishError("cannot reach %s: %s" % (self.base_url, error.reason))


# ---- the work -----------------------------------------------------------------------------------


def missing_hashes(
    transport, wanted: Sequence[Tuple[str, str]]
) -> List[Tuple[str, str]]:
    """Which of `wanted` the store does not hold yet."""
    return [(digest, where) for digest, where in wanted if not transport.published(digest)]


def read_blob(blobs: Path, digest: str, where: str) -> bytes:
    """The local bytes for one hash, re-hashed before they are trusted.

    The filename already claims the hash, so re-hashing looks redundant; it is not. The blob dir is
    a working directory a person can copy into, and a file whose name and content disagree would
    upload cleanly, get stored under its *real* hash, and leave the manifest pointing at nothing.
    """
    path = blobs / ("%s.webp" % digest)
    if not path.exists():
        raise PublishError(
            "no local blob for %s (%s).\n"
            "  The manifest names an image this machine does not have. Recapture and re-encode:\n"
            "    scripts/dev/showcase.sh <platform> --set docs\n"
            "    python3 scripts/dev/docs_images.py" % (where, digest[:12])
        )
    data = path.read_bytes()
    actual = hashlib.sha256(data).hexdigest()
    if actual != digest:
        raise PublishError(
            "%s.webp does not hash to its own name (it is %s). Re-run "
            "scripts/dev/docs_images.py rather than uploading it." % (digest[:12], actual[:12])
        )
    return data


def publish(
    transport, blobs: Path, missing: Iterable[Tuple[str, str]], log=print
) -> int:
    """Upload each missing blob; return how many were newly stored."""
    stored = 0
    for digest, where in missing:
        data = read_blob(blobs, digest, where)
        result = transport.upload(data)
        returned = result.get("hash")
        if returned != digest:
            raise PublishError(
                    "uploaded %s but the store named it %s: the bytes that arrived are not the bytes "
                "the manifest describes." % (digest[:12], str(returned)[:12])
            )
        if result.get("stored"):
            stored += 1
        log("  published %s  %s" % (digest[:12], where))
    return stored


# ---- CLI ----------------------------------------------------------------------------------------


def resolve_token(env_file: Optional[str] = None) -> Optional[str]:
    """The upload token, from the environment or the credentials file, or `None`.

    The environment wins: a file is a convenience, not an override of what the person running the
    command just typed. Resolved even for `--check` and a plain plan, which need no token, so a
    misconfigured file is reported the same way whichever mode you happen to run first.
    """
    return resolve_settings(env_file=env_file).token


class Settings:
    """Where this run publishes, what it authenticates with, and where each came from.

    The provenance is not bookkeeping. A `docs.env` left over from testing against localhost would
    otherwise send `--apply` somewhere harmless-looking and let `--check` pass against a site nobody
    reads, while production still served nothing. So a non-default target is *announced*, naming the
    thing that chose it.
    """

    def __init__(self, base_url: str, base_url_source: str, token: Optional[str]) -> None:
        self.base_url = base_url
        self.base_url_source = base_url_source
        self.token = token

    @property
    def is_default_target(self) -> bool:
        return self.base_url == DEFAULT_BASE_URL


def resolve_settings(base_url: Optional[str] = None, env_file: Optional[str] = None) -> Settings:
    """Resolve the target and the token: flag, then environment, then file, then the default.

    The real environment beats the file, and an explicit flag beats both; a file is a convenience,
    never an override of what the person running the command just typed.
    """
    values, sources = envfile.load(
        ENV_FILE_LOCATIONS, ENV_FILE_VAR, "a token that can publish to the docs site", env_file
    )

    def pick(name: str) -> Tuple[Optional[str], str]:
        """The environment, then the files; the two sources both settings share."""
        from_env = os.environ.get(name, "").strip()
        if from_env:
            return from_env, "$%s" % name
        from_file = values.get(name, "").strip()
        if from_file:
            # The file that supplied *this* key, not merely the first one that existed: after a
            # merge those differ, and only the former is the file someone would go and fix.
            return from_file, str(sources[name])
        return None, "the default"

    if base_url:
        target, source = base_url.strip(), "--base-url"
    else:
        target, source = pick(BASE_URL_ENV)
    token, _ = pick(TOKEN_ENV)
    return Settings(target or DEFAULT_BASE_URL, source, token)


def load_manifest(path: Path) -> Dict[str, object]:
    if not path.exists():
        raise PublishError("no manifest at %s: run scripts/dev/docs_images.py first" % path)
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise PublishError("%s is not valid JSON: %s" % (path, error))


def run(args, transport) -> int:
    # Say so when this is not the live site. A `docs.env` still pointing at localhost from a test
    # run would otherwise let `--apply` upload somewhere nobody reads and `--check` pass against it,
    # while allodia.eu served nothing; and every line of output would look like success.
    source = getattr(transport, "base_url_source", None)
    if source is not None and transport.base_url != DEFAULT_BASE_URL:
        print("Publishing to %s (not the default %s): from %s." % (
            transport.base_url, DEFAULT_BASE_URL, source
        ))

    manifest = load_manifest(args.manifest)
    wanted = referenced_hashes(manifest)

    if not wanted:
        # Not an error: until the guides exist, the manifest is deliberately empty
        # (`docs/user-docs.md` → Known gaps). Say so plainly rather than reporting "0 published",
        # which reads like a successful upload of nothing.
        print(
            "The manifest references no images yet, so there is nothing to publish. "
            "It fills in the same change as the pages that show them."
        )
        return 0

    missing = missing_hashes(transport, wanted)
    print(
        "%d image(s) referenced by the manifest; %d already published at %s."
        % (len(wanted), len(wanted) - len(missing), transport.base_url)
    )

    if args.check:
        if missing:
            print(
                "\nERROR: %d image(s) the docs reference are not published:" % len(missing),
                file=sys.stderr,
            )
            for digest, where in missing:
                print("  %s  %s" % (digest[:12], where), file=sys.stderr)
            print(
                "\nA page that ships before its images renders broken. Publish them:\n"
                "  python3 scripts/dev/docs_publish.py --apply",
                file=sys.stderr,
            )
            return 1
        print("OK: every image the docs reference is published.")
        return 0

    if not missing:
        print("Nothing to do: every referenced image is already published.")
        return 0

    if not args.apply:
        print("\nWould publish %d image(s):" % len(missing))
        for digest, where in missing:
            print("  %s  %s" % (digest[:12], where))
        print("\nRe-run with --apply to upload them.")
        return 0

    print("\nPublishing %d image(s) to %s:" % (len(missing), transport.base_url))
    stored = publish(transport, args.blobs, missing)
    print(
        "Done: %d newly stored, %d already present (an upload is idempotent, so a re-run after a "
        "partial one is safe)." % (stored, len(missing) - stored)
    )
    return 0


def execute(args, transport) -> int:
    """`run`, with its errors turned into a message and an exit code.

    The two halves are split so `run` can raise from wherever the problem is, while every caller,
    the CLI, the tests, a release script; sees the same output and the same status.
    """
    try:
        return run(args, transport)
    except PublishError as error:
        print("ERROR: %s" % error, file=sys.stderr)
        return 1


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument("--blobs", type=Path, default=BLOBS)
    parser.add_argument(
        "--base-url",
        help="the website to publish to (else $%s, else the credentials file, else %s)"
        % (BASE_URL_ENV, DEFAULT_BASE_URL),
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--apply", action="store_true", help="actually upload the missing images")
    mode.add_argument(
        "--check",
        action="store_true",
        help="exit 1 if the manifest names an image the website does not serve",
    )
    parser.add_argument(
        "--env-file",
        metavar="PATH",
        help="credentials file holding $%s (default: %s)"
        % (TOKEN_ENV, " or ".join(str(p) for p in ENV_FILE_LOCATIONS)),
    )
    args = parser.parse_args(argv)

    try:
        settings = resolve_settings(args.base_url, args.env_file)
    except envfile.EnvFileError as error:
        print("ERROR: %s" % error, file=sys.stderr)
        return 1
    return execute(
        args, HttpTransport(settings.base_url, settings.token, settings.base_url_source)
    )


if __name__ == "__main__":
    raise SystemExit(main())
