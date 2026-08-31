#!/usr/bin/env python3
"""Fill a Partner Center listing-data CSV export from the resolved store listing.

Partner Center can export every language's listing copy as one CSV and import it back. That is the
route that works when the REST push does not: a submission created in the console answers `GET` but
refuses `PUT` with `InvalidState` ("the state 'None'"), so a draft opened by hand can only be
finished by hand; or by this, which is the same contract read the same way, handed to the console's
own import instead of its API.

**It edits the export in place rather than authoring a CSV.** The export carries far more than the
copy this repo writes down: screenshot URLs Partner Center minted, logo slots, hardware
requirements, the reserved-name Title. Regenerating that from scratch would mean inventing values
for fields no document states; so every row this tool does not own is copied through byte for
byte, and only the cells it does own are written. What it owns is exactly what
[`msstore_payload.py`](msstore_payload.py) owns, and it asks that module rather than re-reading the
document, so the CSV route and the API route cannot drift into telling the Store different things.

**Trailing slots are cleared, not left.** `Feature1..20` and `SearchTerm1..7` are fixed rows, so a
language whose list got shorter would keep its old tail; a feature nobody wrote, sitting under
nineteen that somebody did. Cells past the end of the list are emptied.

Usage:

    python scripts/dev/msstore_csv.py EXPORT.csv --release-notes 0.3.0
    python scripts/dev/msstore_csv.py EXPORT.csv -o FILLED.csv -l de -l es

Without `-o` it writes `<name>-filled.csv` beside the input, because overwriting the file you are
about to re-import is a poor default when the run turns out to be wrong.
"""

from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "ci"))
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

from changelog_fragments import DocumentShapeError  # noqa: E402
from check_store_copy_length import LIMITS_PATH, listing_path, parse_limits  # noqa: E402
from msstore_payload import ListingError, load_listings, measure, resolve_locales  # noqa: E402

# The export's own conventions, established by reading one: a UTF-8 BOM, CRLF between rows, and
# bare LF inside a quoted cell. Python's writer reproduces all three given these two settings, so a
# row this tool does not touch comes out byte-identical to the row that went in.
ENCODING = "utf-8-sig"
ROW_TERMINATOR = "\r\n"

# The first three cells name the row; the fourth is Partner Center's `default` column, which is
# empty throughout our export and is not ours to fill. Language columns start after it.
FIRST_LANGUAGE_COLUMN = 4

# The row names this tool writes, and where the value comes from. Everything else in the export,
# screenshots, logos, hardware requirements, ShortDescription, the reserved-name Title; is left
# exactly as exported: a field the resolved listing does not state is a field a human still owns.
SINGLE_ROWS = (
    ("Description", "description"),
    ("CopyrightTrademarkInformation", "copyright"),
)
LIST_ROWS = (
    ("Feature", "features"),
    ("SearchTerm", "search_terms"),
)
RELEASE_NOTES_ROW = "ReleaseNotes"


class ExportShapeError(RuntimeError):
    """The CSV is not the export this tool knows how to fill.

    Its own class because the fix is never the same as a `DocumentShapeError`'s: that one means the
    document moved, this one means the export did (or is not an export at all).
    """


def read_export(path):
    """`(rows, header)` from the export, with its shape checked before anything is written."""
    with Path(path).open(newline="", encoding=ENCODING) as handle:
        rows = list(csv.reader(handle))
    if not rows:
        raise ExportShapeError(f"{path} is empty")
    header = rows[0]
    if header[:3] != ["Field", "ID", "Type (Type)"]:
        raise ExportShapeError(
            f"{path} does not look like a Partner Center listing export: its first row should "
            f"start Field,ID,Type (Type) but starts {','.join(header[:3])}"
        )
    if len(header) <= FIRST_LANGUAGE_COLUMN:
        raise ExportShapeError(f"{path} carries no language columns")
    return rows, header


def language_columns(header, locales):
    """`locale -> column index`, refusing a locale the export has no column for.

    Matched on the column's own name first and its language prefix second, the same way
    `resolve_store_language` reads a submission: an export labels its columns `en` or `en-us`
    depending on how the listing was created, and picking the wrong one writes a language nobody
    reads.
    """
    available = {}
    for index in range(FIRST_LANGUAGE_COLUMN, len(header)):
        name = header[index].strip().lower()
        if name:
            available.setdefault(name, index)
    resolved = {}
    for locale in locales:
        if locale in available:
            resolved[locale] = available[locale]
            continue
        prefixed = sorted(name for name in available if name.split("-")[0] == locale)
        if not prefixed:
            raise ExportShapeError(
            f"the export has no column for {locale}: add that language to the listing in "
                f"Partner Center and export again (it carries {', '.join(sorted(available))})"
            )
        resolved[locale] = available[prefixed[0]]
    return resolved


def _row_index(rows, name):
    """Where `name`'s row is, or `None`. The export names each row in its first cell."""
    for index, row in enumerate(rows):
        if row and row[0] == name:
            return index
    return None


def _set(rows, row_index, column, value, changes, label, locale):
    """Write one cell, recording whether it actually moved."""
    row = rows[row_index]
    if len(row) <= column:
        row.extend([""] * (column + 1 - len(row)))
    if row[column] != value:
        changes.append((locale, label, row[column], value))
        row[column] = value


def fill(rows, listings, columns):
    """Write every owned cell for every listing. Returns the changes, in row order."""
    changes = []
    for listing in listings:
        column = columns[listing.locale]
        for name, attribute in SINGLE_ROWS:
            index = _row_index(rows, name)
            if index is None:
                raise ExportShapeError(f"the export has no {name} row")
            _set(rows, index, column, getattr(listing, attribute), changes, name, listing.locale)

        if listing.release_notes is not None:
            index = _row_index(rows, RELEASE_NOTES_ROW)
            if index is None:
                raise ExportShapeError(f"the export has no {RELEASE_NOTES_ROW} row")
            _set(
                rows, index, column, listing.release_notes, changes,
                RELEASE_NOTES_ROW, listing.locale,
            )

        for prefix, attribute in LIST_ROWS:
            values = getattr(listing, attribute)
            slots = _numbered_rows(rows, prefix)
            if not slots:
                raise ExportShapeError(f"the export has no {prefix}1 row")
            if len(values) > len(slots):
                raise ExportShapeError(
                    f"{listing.locale}: the resolved listing states {len(values)} "
                    f"{prefix.lower()}s but the export has only {len(slots)} {prefix} rows"
                )
            # Past the end of the list, deliberately: a shorter list must remove its old tail.
            for number, index in enumerate(slots):
                value = values[number] if number < len(values) else ""
                _set(rows, index, column, value, changes, f"{prefix}{number + 1}", listing.locale)
    return changes


def _numbered_rows(rows, prefix):
    """`[row index]` for `Feature1`, `Feature2`, ... in numeric order."""
    found = {}
    for index, row in enumerate(rows):
        if not row or not row[0].startswith(prefix):
            continue
        suffix = row[0][len(prefix):]
        if suffix.isdigit():
            found[int(suffix)] = index
    return [found[number] for number in sorted(found)]


def write_export(path, rows):
    """Write the CSV back in the export's own dialect."""
    with Path(path).open("w", newline="", encoding=ENCODING) as handle:
        csv.writer(handle, lineterminator=ROW_TERMINATOR).writerows(rows)


def _preview(value, width=68):
    """One line standing in for a cell that may be four thousand characters of prose."""
    flattened = " ".join((value or "").split())
    if not flattened:
        return "(empty)"
    return flattened if len(flattened) <= width else flattened[: width - 1] + "…"


def render(changes, locales):
    """What the run did, per language, in the words the console uses for each field."""
    out = []
    for locale in locales:
        mine = [change for change in changes if change[0] == locale]
        if not mine:
            out.append(f"  {locale}: already matches the contract")
            continue
        out.append(f"  {locale}: {len(mine)} field(s) updated")
        for _, label, before, after in mine:
            out.append(f"    {label:<16} - {_preview(before)}")
            out.append(f"    {'':<16} + {_preview(after)}")
    return "\n".join(out)


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("export", help="the listingData-*.csv Partner Center exported")
    parser.add_argument(
        "-o", "--out", help="where to write it (default: <name>-filled.csv beside the input)"
    )
    parser.add_argument(
        "-l", "--language", action="append", dest="locales",
        help="catalog locale to fill, repeatable (default: every locale in the catalog)",
    )
    parser.add_argument(
        "--release-notes", metavar="X.Y.Z",
        help="also fill What's new from docs/changelog/released/X.Y.Z.md",
    )
    parser.add_argument("--listing", help="read a different store-listing.md (for tests)")
    return parser.parse_args(argv)


def run(args):
    listing_md = Path(args.listing) if args.listing else listing_path()
    limits = parse_limits(LIMITS_PATH.read_text(encoding="utf-8"))
    locales = resolve_locales(args.locales)
    listings = load_listings(
        listing_md=listing_md, locales=locales, version=args.release_notes
    )

    # Measured before a byte is written. A CSV the console rejects on import costs the same
    # round-trip a rejected submission does, and the field it names is one the document could have
    # been told about here.
    problems = [problem for listing in listings for problem in measure(listing, limits)]
    if problems:
        print("The copy does not fit the Microsoft Store's fields:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        return 1

    rows, header = read_export(args.export)
    columns = language_columns(header, locales)
    changes = fill(rows, listings, columns)

    destination = Path(args.out) if args.out else _default_out(args.export)
    write_export(destination, rows)

    print(f"Read  {args.export}")
    print(f"Wrote {destination}")
    print(f"Languages: {', '.join(locales)}")
    if args.release_notes:
        print(f"What's new: {args.release_notes}")
    else:
        print("What's new: left untouched (pass --release-notes X.Y.Z to fill it)")
    print("Left untouched: screenshots, logos, Title, ShortDescription, hardware requirements")
    print()
    print(render(changes, locales))
    return 0


def _default_out(export):
    export = Path(export)
    return export.with_name(f"{export.stem}-filled{export.suffix}")


def main(argv=None):
    # The report quotes listing copy, which is not representable in a Windows console's cp1252.
    # Checked per stream: a caller that captured only one of them (a test, a pipeline) leaves the
    # other a `StringIO`, which has no `reconfigure` and would take the whole run down with it.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    args = parse_args(argv)
    try:
        return run(args)
    except (ListingError, ExportShapeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    except DocumentShapeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
