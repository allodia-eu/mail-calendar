#!/usr/bin/env python3
"""The Microsoft Store submission API, as much of it as a listing update needs.

Partner Center's listing page is one form per language, and we ship seven; title, description,
twenty product features and a screenshot gallery each. Retyping that by hand is not just slow: it
is the exact drift `docs/store-listing.md` exists to prevent, because the console is the one place
a paste can quietly differ from the file it came from.

This module is the transport half of that job; HTTP, auth, and nothing else. The half that reads
the resolved listing and decides what a listing should say lives in
[`msstore_listing.py`](msstore_listing.py), which is where the tests are: everything here needs a
Partner Center account to exercise, so keeping it thin is what keeps the untested surface small.

**Which API this is.** The app ships as MSIX through Partner Center, so this is the
`manage.devcenter.microsoft.com` submission API; the one documented under "Manage app
submissions", whose submission resource carries `listings` keyed by store language. The newer
`api.store.microsoft.com` surface is documented for MSI/EXE installers and does not apply to us.

    https://learn.microsoft.com/windows/uwp/monetize/manage-app-submissions

**Prerequisites** (one-time, all in Partner Center; none of them can be done from here):

1. An Entra ID (Azure AD) application associated with the Partner Center account, holding the
   **Manager** role. Its tenant id, client id and key are what authenticate every call below.
2. The app must already exist, and must have **one completed submission made by hand**, age-rating
   questionnaire included. The API can create submissions for an app; it cannot create the app.

**Two documented refusals to recognise if the first run fails.** The API returns **409** for an app
that uses mandatory app updates or Store-managed consumable add-ons; neither of which we use; and
an app on **Pricing Version 2** reads back an unknown price tier, which makes a whole-resource
update unsafe for the pricing module. This tool only ever changes listing copy, but it PUTs the
submission it was handed back whole, so if a run fails on pricing that is why: the fix is to make
the pricing edit in Partner Center and leave it out of the pushed resource.

**The rule that bites.** Once a submission has been touched through this API, further edits belong
to the API. Changing that same submission in Partner Center can leave it in a state the API may no
longer update or commit, and the fix is to delete it and start a new one. Reviewing it and pressing
Submit is fine; that ends the submission; but do not edit the listing by hand on top of a pushed
draft and expect the next push to work.

**Where the credentials live.** Four values; `MSSTORE_TENANT_ID`, `MSSTORE_CLIENT_ID`,
`MSSTORE_CLIENT_SECRET`, `MSSTORE_APP_ID`; read from the environment, or from a `KEY=value` file
when the environment does not carry them:

    --env-file PATH                     an explicit file; a missing one is an error, never a
                                        silent fall back to the environment
    $MSSTORE_ENV_FILE                   the same, from the environment
    <repo>/.msstore.env                 gitignored, beside the code you are running
    ~/.config/allodia/msstore.env       outside every checkout, one file for every worktree

    # .msstore.env
    MSSTORE_TENANT_ID=00000000-0000-0000-0000-000000000000
    MSSTORE_CLIENT_ID=00000000-0000-0000-0000-000000000000
    MSSTORE_CLIENT_SECRET=...
    MSSTORE_APP_ID=9NBLGGH000000

A real environment variable always beats the file, so a one-off `MSSTORE_APP_ID=… msstore_listing.py`
still overrides it. The file is parsed, not sourced: `#` comments, blank lines, a leading `export `
and surrounding quotes are tolerated, and nothing else; no interpolation and no subshells, because
a credentials file that can run something is a credentials file that can be made to run something.
On POSIX the tool warns if the file is readable by anyone else; that key can publish.

No third-party packages: `urllib` only, so the script runs on whatever `python3` a machine has.
"""

from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path

import envfile

# The audience the token is minted for, and the API root it opens. Both are literal strings in
# Microsoft's docs; the "resource" is not a URL that gets fetched, it names the API.
RESOURCE = "https://manage.devcenter.microsoft.com"
API_ROOT = RESOURCE + "/v1.0/my"
AUTHORITY = "https://login.microsoftonline.com/{tenant}/oauth2/token"

# What the credentials are called, wherever they come from.
ENV_TENANT = "MSSTORE_TENANT_ID"
ENV_CLIENT = "MSSTORE_CLIENT_ID"
ENV_SECRET = "MSSTORE_CLIENT_SECRET"
ENV_APP = "MSSTORE_APP_ID"
CREDENTIAL_NAMES = (ENV_TENANT, ENV_CLIENT, ENV_SECRET, ENV_APP)

# Where to look for them, in order, when the environment does not already carry them. The real
# environment always wins, so a one-off `MSSTORE_APP_ID=… scripts/dev/msstore_listing.py` still
# overrides the file rather than being silently ignored by it.
#
# Two locations because they answer different questions. `.msstore.env` sits beside the code you are
# running and is **gitignored**; the convenient one. `~/.config/allodia/msstore.env` is outside
# every checkout, so one file serves every worktree and no `git add -A` can ever reach it; the
# careful one. The key it holds can publish to the Store listing, so prefer the second if you keep
# more than one clone.
ENV_FILE_VAR = "MSSTORE_ENV_FILE"
REPO_ROOT = envfile.REPO_ROOT
ENV_FILE_LOCATIONS = envfile.locations_for("msstore")

# A blob PUT is one shot, so it has to fit Azure's single-request ceiling (256 MiB). The listing
# payload is a handful of PNGs and nowhere near it; the guard exists so the failure, if it ever
# happens, is a sentence rather than a truncated upload.
MAX_UPLOAD_BYTES = 200 * 1024 * 1024

# Retried once per attempt on the statuses that mean "not now" rather than "not ever". Partner
# Centre rate-limits, and a 500 mid-run would otherwise leave a half-updated draft behind.
RETRY_STATUSES = (429, 500, 502, 503, 504)
RETRIES = 3
BACKOFF_SECONDS = 5


class PartnerCenterError(RuntimeError):
    """A call to Partner Center failed, with whatever the service said about why."""


# The file half of credential loading is shared with the other publishing scripts (`envfile`), so
# there is one parser and one search order rather than one per secret. Re-exported under the names
# this module has always used.
parse_env_file = envfile.parse_env_file


def find_env_file(explicit=None):
    """The credentials file this run should read, or `None`. Never raises for a missing default."""
    try:
        return envfile.find_env_file(ENV_FILE_LOCATIONS, ENV_FILE_VAR, explicit)
    except envfile.EnvFileError as error:
        raise PartnerCenterError(str(error))


def _warn_if_readable_by_others(path):
    """A one-line nudge if the file is group/world readable. POSIX only; Windows has no mode."""
    envfile.warn_if_readable_by_others(path, "a key that can publish to the Store")


@dataclass(frozen=True)
class Credentials:
    """What authenticates a run. `app_id` is the Store ID from the app's Partner Center URL."""

    tenant_id: str
    client_id: str
    client_secret: str
    app_id: str
    source: str = "environment"

    @classmethod
    def from_env(cls, app_id=None, env_file=None):
        """Resolve the four values, naming every one that is missing and where it was looked for.

        Named together rather than one at a time: the first run on a new machine is missing all
        four, and four consecutive failures is a worse way to learn that than one list.
        """
        # Every location that exists is read and merged (earlier wins per key), so a shared `.env`
        # holding another tool's keys cannot hide the `MSSTORE_*` ones in `.msstore.env` beside it.
        try:
            from_file, sources = envfile.load(
                ENV_FILE_LOCATIONS,
                ENV_FILE_VAR,
                "a key that can publish to the Store",
                env_file,
            )
        except envfile.EnvFileError as error:
            raise PartnerCenterError(str(error))
        # For the message below: whichever file supplied a credential, else the first that exists.
        path = next(
            (sources[name] for name in CREDENTIAL_NAMES if name in sources),
            find_env_file(env_file),
        )

        def resolve(name):
            # The real environment first: a file is a convenience, not an override of what the
            # person running the command just typed.
            return (os.environ.get(name) or from_file.get(name) or "").strip()

        values = {name: resolve(name) for name in CREDENTIAL_NAMES}
        if app_id:
            values[ENV_APP] = app_id.strip()

        missing = [name for name in CREDENTIAL_NAMES if not values[name]]
        if missing:
            # A typo'd key in the file reads exactly like a missing value, so say what was found
            # there; `MSSTORE_SECRET=` instead of `MSSTORE_CLIENT_SECRET=` is the whole bug.
            stray = sorted(
                key for key in from_file if key.startswith("MSSTORE_") and key not in CREDENTIAL_NAMES
            )
            where = (
                f"Read {path}, which sets: {', '.join(sorted(from_file)) or 'nothing'}."
                if path is not None
                else "No credentials file found. Put one at "
                + " or ".join(str(candidate) for candidate in ENV_FILE_LOCATIONS)
                + f", or point {ENV_FILE_VAR} / --env-file at one."
            )
            raise PartnerCenterError(
                "no Partner Center credentials: missing "
                + ", ".join(missing)
                + ". "
                + where
                + (f" Unrecognised key(s) in it: {', '.join(stray)}." if stray else "")
                + " The tenant id, client id and key come from Partner Center -> Account settings"
                " -> User management -> Azure AD applications; the app id is the Store ID on the"
                " app's overview page. See this module's docstring for the one-time setup."
            )
        return cls(
            tenant_id=values[ENV_TENANT],
            client_id=values[ENV_CLIENT],
            client_secret=values[ENV_SECRET],
            app_id=values[ENV_APP],
            source=str(path) if path is not None else "environment",
        )


def _read_error(error):
    """Partner Center's error body, if it sent one; its `message` is usually the whole story."""
    try:
        payload = json.loads(error.read().decode("utf-8"))
    except Exception:  # noqa: BLE001 - a non-JSON error body is still an error worth reporting
        return ""
    if isinstance(payload, dict):
        parts = [str(payload.get(key)) for key in ("code", "message") if payload.get(key)]
        for entry in payload.get("errors") or []:
            if isinstance(entry, dict) and entry.get("message"):
                parts.append(str(entry["message"]))
        return "; ".join(parts)
    return json.dumps(payload)[:500]


class PartnerCenter:
    """One authenticated session against one app's submissions.

    Every method maps to a single documented endpoint and returns the parsed JSON unchanged. No
    method here decides *what* a listing should say; that is the caller's job, so the two halves
    can be reasoned about separately.
    """

    def __init__(self, credentials, timeout=180):
        self._credentials = credentials
        self._timeout = timeout
        self._token = None

    # -- auth ---------------------------------------------------------------------------------

    def access_token(self):
        """A bearer token, minted once per run. They last an hour; a run takes seconds."""
        if self._token is not None:
            return self._token
        body = urllib.parse.urlencode(
            {
                "grant_type": "client_credentials",
                "client_id": self._credentials.client_id,
                "client_secret": self._credentials.client_secret,
                "resource": RESOURCE,
            }
        ).encode("utf-8")
        request = urllib.request.Request(
            AUTHORITY.format(tenant=self._credentials.tenant_id),
            data=body,
            method="POST",
            headers={"Content-Type": "application/x-www-form-urlencoded; charset=utf-8"},
        )
        try:
            with urllib.request.urlopen(request, timeout=self._timeout) as response:
                payload = json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as error:
            # Deliberately does not echo the request body; it carries the client secret, and a
            # pasted traceback is how a key ends up in an issue thread.
            raise PartnerCenterError(
                f"could not obtain an access token ({error.code}): {_read_error(error)}. Check "
                f"{ENV_TENANT}/{ENV_CLIENT}/{ENV_SECRET}, and that the app has the Manager role "
                "in Partner Center."
            ) from None
        except urllib.error.URLError as error:
            raise PartnerCenterError(f"could not reach {AUTHORITY.format(tenant='…')}: {error.reason}")
        self._token = payload["access_token"]
        return self._token

    # -- plumbing -----------------------------------------------------------------------------

    def _call(self, method, path, body=None):
        """One API call. `path` is relative to `/v1.0/my`, exactly as the docs write it."""
        url = f"{API_ROOT}{path}"
        data = json.dumps(body).encode("utf-8") if body is not None else None
        headers = {
            "Authorization": f"Bearer {self.access_token()}",
            "Content-Type": "application/json",
            "User-Agent": "allodia-mailcal-store-listing/1.0",
        }
        last = None
        for attempt in range(RETRIES):
            request = urllib.request.Request(url, data=data, method=method, headers=headers)
            try:
                with urllib.request.urlopen(request, timeout=self._timeout) as response:
                    raw = response.read().decode("utf-8")
                return json.loads(raw) if raw.strip() else {}
            except urllib.error.HTTPError as error:
                detail = _read_error(error)
                last = PartnerCenterError(f"{method} {path} failed ({error.code}): {detail}")
                if error.code not in RETRY_STATUSES:
                    raise last from None
            except urllib.error.URLError as error:
                last = PartnerCenterError(f"{method} {path} could not be sent: {error.reason}")
            if attempt + 1 < RETRIES:
                time.sleep(BACKOFF_SECONDS * (attempt + 1))
        raise last

    # -- submissions --------------------------------------------------------------------------

    def app(self):
        """The app resource; carries the pending and last-published submission ids."""
        return self._call("GET", f"/applications/{self._credentials.app_id}")

    def submission(self, submission_id):
        """One submission, whole: listings, packages, pricing, and the SAS upload URI."""
        return self._call(
            "GET", f"/applications/{self._credentials.app_id}/submissions/{submission_id}"
        )

    def create_submission(self):
        """Open a new in-progress submission; a copy of the last published one."""
        return self._call("POST", f"/applications/{self._credentials.app_id}/submissions")

    def update_submission(self, submission_id, submission):
        """Replace the in-progress submission's data. The whole resource goes back, not a patch."""
        return self._call(
            "PUT",
            f"/applications/{self._credentials.app_id}/submissions/{submission_id}",
            submission,
        )

    def commit_submission(self, submission_id):
        """Hand the submission to the Store; this is what sends it to certification."""
        return self._call(
            "POST",
            f"/applications/{self._credentials.app_id}/submissions/{submission_id}/commit",
        )

    def submission_status(self, submission_id):
        """Where the commit got to: `CommitStarted` -> `PreProcessing`, or `CommitFailed`."""
        return self._call(
            "GET",
            f"/applications/{self._credentials.app_id}/submissions/{submission_id}/status",
        )

    def delete_submission(self, submission_id):
        """Throw away an in-progress submission. Never called without an explicit flag."""
        return self._call(
            "DELETE", f"/applications/{self._credentials.app_id}/submissions/{submission_id}"
        )

    # -- blob upload --------------------------------------------------------------------------

    def upload(self, sas_url, archive):
        """PUT the submission's asset ZIP; screenshots, and the `.msixupload` when one is sent.

        A plain block-blob PUT rather than the Azure SDK: taking a dependency for one request would
        make this script un-runnable on a machine that has nothing but `python3`. That holds at
        bundle size too; the request is the same shape at 161 MB as it was at 11 MB, it just takes
        longer, which is why `MAX_UPLOAD_BYTES` is the only thing standing between here and a
        block-list upload.
        """
        archive = Path(archive)
        size = archive.stat().st_size
        if size > MAX_UPLOAD_BYTES:
            raise PartnerCenterError(
                f"{archive.name} is {size / 1_048_576:.0f} MB, over this uploader's "
                f"{MAX_UPLOAD_BYTES / 1_048_576:.0f} MB single-request ceiling. The bundle is the "
                "bulk of it. 0.5.0's was 150 MB of a 161 MB archive, so dropping screenshots "
                "buys little: raise the ceiling (Azure takes 256 MiB in one PUT), or teach this "
                "method Azure's block-list upload."
            )
        with archive.open("rb") as handle:
            request = urllib.request.Request(
                sas_url,
                data=handle.read(),
                method="PUT",
                headers={
                    "x-ms-blob-type": "BlockBlob",
                    "Content-Type": "application/zip",
                    "Content-Length": str(size),
                },
            )
            try:
                with urllib.request.urlopen(request, timeout=self._timeout) as response:
                    return response.status
            except urllib.error.HTTPError as error:
                raise PartnerCenterError(
                    f"uploading {archive.name} failed ({error.code}): {_read_error(error)}. The "
                    "SAS URI expires: re-run rather than reusing a stale plan."
                ) from None
