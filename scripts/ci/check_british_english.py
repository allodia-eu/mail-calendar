#!/usr/bin/env python3
"""Fail if prose in this repository is written in American English.

`AGENTS.md` → "How we write" states the rule: British English in prose, comments and user-facing
copy alike, and identifiers keep whatever spelling their spec or tool gave them. The tree was swept
once to satisfy it, which is exactly the state a rule rots from: a hundred files agree, nobody
remembers why, and the next `behavior` reads like precedent. This is the machine half.

    scripts/ci/check_british_english.py [--list]

**What counts as prose.** Markdown outside fenced blocks, and comment lines in source. Inline code
spans are cut out before matching, and a hit sitting against an identifier character is ignored, so
`normalizeColor` and `authorization_url` are invisible here whatever file they are in.

**What the rule deliberately does not reach** is `PHRASES` and `SYMBOLS` below: OAuth's and iCalendar's own
vocabulary, toolkit and language keywords, and three words that are the thing's actual name rather
than a spelling choice (`dialog`, `artifact`, `catalog`). A name is not a spelling, and a repository
that "corrected" `ORGANIZER` would be describing a property that does not exist.

**A doc reference is an identifier wearing prose clothes**, and the reason `cref=` is in `SYMBOLS`:
C#'s `<see cref="Maximized"/>` and Kotlin's `[authorizationUrl]` name a symbol from inside a
comment, where nothing backticks them and renaming one resolves to nothing.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# American -> British. Longest first, so `organization` is decided before `organize` can match it.
SPELLINGS = [
    ("behavioral", "behavioural"), ("behaviors", "behaviours"), ("behavior", "behaviour"),
    ("colors", "colours"), ("colored", "coloured"), ("coloring", "colouring"), ("color", "colour"),
    ("organizations", "organisations"), ("organization", "organisation"),
    ("organizers", "organisers"), ("organizer", "organiser"), ("organized", "organised"),
    ("organizing", "organising"), ("organizes", "organises"), ("organize", "organise"),
    ("sanitizations", "sanitisations"), ("sanitization", "sanitisation"),
    ("sanitizers", "sanitisers"), ("sanitizer", "sanitiser"),
    ("sanitized", "sanitised"), ("sanitizing", "sanitising"),
    ("sanitizes", "sanitises"), ("sanitize", "sanitise"),
    ("normalization", "normalisation"), ("normalized", "normalised"),
    ("normalizing", "normalising"), ("normalizes", "normalises"), ("normalize", "normalise"),
    ("centered", "centred"), ("centering", "centring"), ("centers", "centres"), ("center", "centre"),
    ("canceled", "cancelled"), ("canceling", "cancelling"),
    ("analyzed", "analysed"), ("analyzing", "analysing"), ("analyze", "analyse"),
    ("recognized", "recognised"), ("recognizing", "recognising"),
    ("recognizes", "recognises"), ("recognize", "recognise"),
    ("authorization", "authorisation"), ("authorized", "authorised"),
    ("authorizing", "authorising"), ("authorizes", "authorises"), ("authorize", "authorise"),
    ("customization", "customisation"), ("customized", "customised"),
    ("customizes", "customises"), ("customize", "customise"),
    ("optimization", "optimisation"), ("optimized", "optimised"),
    ("optimizing", "optimising"), ("optimizes", "optimises"), ("optimize", "optimise"),
    ("initialization", "initialisation"), ("initialized", "initialised"),
    ("initializing", "initialising"), ("initializes", "initialises"), ("initialize", "initialise"),
    ("localization", "localisation"), ("localized", "localised"),
    ("localizing", "localising"), ("localizes", "localises"), ("localize", "localise"),
    ("summarized", "summarised"), ("summarizing", "summarising"),
    ("summarizes", "summarises"), ("summarize", "summarise"),
    ("utilization", "utilisation"), ("utilized", "utilised"),
    ("utilizing", "utilising"), ("utilizes", "utilises"), ("utilize", "utilise"),
    ("synchronization", "synchronisation"), ("synchronized", "synchronised"),
    ("synchronizing", "synchronising"), ("synchronizes", "synchronises"),
    ("synchronize", "synchronise"),
    ("visualization", "visualisation"), ("visualized", "visualised"),
    ("visualizing", "visualising"), ("visualize", "visualise"),
    ("realized", "realised"), ("realizing", "realising"),
    ("realizes", "realises"), ("realize", "realise"),
    ("minimized", "minimised"), ("minimizing", "minimising"),
    ("minimizes", "minimises"), ("minimize", "minimise"),
    ("maximized", "maximised"), ("maximizing", "maximising"),
    ("maximizes", "maximises"), ("maximize", "maximise"),
    ("categorized", "categorised"), ("categorizes", "categorises"), ("categorize", "categorise"),
    ("prioritized", "prioritised"), ("prioritizes", "prioritises"), ("prioritize", "prioritise"),
    ("generalized", "generalised"), ("generalizes", "generalises"), ("generalize", "generalise"),
    ("apologize", "apologise"),
    ("labeled", "labelled"), ("labeling", "labelling"),
    ("modeling", "modelling"), ("traveling", "travelling"),
    ("honored", "honoured"), ("honoring", "honouring"), ("honors", "honours"), ("honor", "honour"),
    ("favored", "favoured"), ("favoring", "favouring"), ("favors", "favours"), ("favor", "favour"),
    ("defense", "defence"), ("offense", "offence"),
]

# Names somebody else chose. Two lists, because they are matched two different ways.
#
# A PHRASE is vocabulary: compared with case and hyphens flattened, so "authorization URL",
# "Authorization-URL" and "authorization-url" are one entry rather than three. That flattening is
# safe here precisely because these are multi-word terms -- no phrase can swallow an ordinary
# sentence the way a lowercased bare word would.
PHRASES = [
    # OAuth, OpenID and the RFCs that name them.
    "authorization server", "authorization endpoint", "authorization code", "authorization request",
    "authorization grant", "authorization response", "authorization url", "authorization flow",
    "authorization header", "authorization_endpoint", "authorization_code", "authorization_url",
    "rfc 6749", "rfc 8414", "rfc 7591", "rfc 9728",
    # The console a Windows release is submitted to.
    "partner center",
]

# A SYMBOL is a single token, so it is compared exactly: `Color` is a Swift type and `color` is an
# ordinary word, and flattening the case would make the type name excuse every use of the word.
SYMBOLS = [
    # iCalendar, serde and REUSE.
    "ORGANIZER", "invitation_organizer", "organizer_line", "serialize", "deserialize",
    "Serialize", "Deserialize", "SPDX-License-Identifier", "LicenseRef", "LICENSES", "LICENSE",
    # Toolkit, language and platform keywords, and the tools a build actually invokes.
    "authorizationUrl", "Authorization:", "@Synchronized", "synchronized(", "Synchronized",
    "isMaximized", "WindowState", "NSLocalizedString", "LocalizedStringKey", "localizedDescription",
    "LocalizedError", "initializer", "Initializer", "recognizer", "Recognizer", "colorScheme",
    "color:", "Color", "Colors", "colorResource", "grayscale", "upload-artifact",
    "download-artifact", "normalize_platform", "summarize_repeat", "AppCulture",
    # A doc reference names a symbol from inside a comment. See the module docstring.
    "cref=", "paramref name=", "typeparamref name=",
]

# Not ours to rewrite, or not rewritten for a reason stated in AGENTS.md.
EXEMPT = (
    "CODE_OF_CONDUCT.md",              # Contributor Covenant, CC-BY-4.0, upstream text
    "LICENSES/",                       # licence texts, upstream
    "clients/android/gradle",          # the Gradle wrapper, upstream
    "clients/composer/dist/",          # generated: the committed editor bundle
    "docs/changelog/released/",        # history: a released note is what shipped
    "docs/changelog/announcements/",   # the same
    "docs/privacy-policy",             # a published contract; its text moves with a version bump
    "scripts/ci/check_british_english.py",
    "scripts/ci/tests/test_british_english.py",
)

EXTENSIONS = {".md", ".rs", ".swift", ".kt", ".kts", ".cs", ".ts", ".js", ".py", ".sh", ".ps1",
              ".toml", ".yml", ".yaml", ".xml", ".html"}

# `/**` is listed first and separately: `//` needs two slashes and `*` needs the line to start with
# one, so a whole KDoc on a single line matches neither.
COMMENT = re.compile(r"^\s*(?:/\*\*|/\*|///|//!|//|#|\*|--|<!--)\s?(.*)")
CODE_SPAN = re.compile(r"`[^`]*`")
FINDERS = [(re.compile(r"\b%s\b" % american, re.I), american, british)
           for american, british in SPELLINGS]


def prose(path: Path):
    """`(line number, the prose on it)` for the parts of one file the writing rule reaches."""
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return
    markdown = path.suffix == ".md"
    fenced = False
    for number, raw in enumerate(text.splitlines(), 1):
        if markdown and raw.lstrip().startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        line = CODE_SPAN.sub("", raw)
        if not markdown:
            comment = COMMENT.match(line)
            if not comment:
                continue
            line = CODE_SPAN.sub("", comment.group(1))
        yield number, line


def hits(path: Path):
    """Every American spelling in one file's prose, as `(line, found, wanted)`."""
    for number, line in prose(path):
        for pattern, american, british in FINDERS:
            match = pattern.search(line)
            if not match:
                continue
            start, end = match.span()
            # An identifier boundary, not a sentence's: a full stop ends a sentence far more often
            # than it joins `body.center.x`, so it counts only with a word character behind it.
            before, after = line[max(0, start - 1):start], line[end:end + 2]
            if before == "_" or (before == "." and start >= 2 and line[start - 2].isalnum()):
                continue
            if after[:1] in "_(" or (after[:1] == "." and after[1:2].isalnum()):
                continue
            window = line[max(0, start - 30):end + 30]
            if any(symbol in window for symbol in SYMBOLS):
                continue
            flattened = window.lower().replace("-", " ")
            if any(phrase in flattened for phrase in PHRASES):
                continue
            yield number, match.group(0), british


# `--others --exclude-standard` alongside `--cached` is not optional: without it `git ls-files`
# reads the index, so a file added but not yet staged is invisible and this passes on the very
# change that introduces what it forbids. `check-public-hygiene.sh` already says so about
# `git grep --untracked`, and AGENTS.md says it about `check-file-length.sh`; this checker had the
# same hole and neither. Ignored paths (target/, .env) stay ignored either way.
def tracked(root: Path):
    listing = subprocess.run(["git", "-C", str(root), "ls-files", "--cached", "--others",
                              "--exclude-standard"],
                             capture_output=True, text=True, check=True).stdout.split()
    for name in listing:
        if any(part in name for part in EXEMPT):
            continue
        path = root / name
        if path.suffix in EXTENSIONS and path.is_file():
            yield name, path


def main(argv=None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    listing = "--list" in argv
    found = []
    checked = 0
    for name, path in tracked(REPO_ROOT):
        checked += 1
        for number, was, wanted in hits(path):
            found.append((name, number, was, wanted))
    if listing:
        for name, number, was, wanted in found:
            print("%s:%d: %s -> %s" % (name, number, was, wanted))
        return 0
    if found:
        print("Prose written in American English:", file=sys.stderr)
        for name, number, was, wanted in found:
            print("  %s:%d: %s -> %s" % (name, number, was, wanted), file=sys.stderr)
        print(
            "\nERROR: %d spelling(s). AGENTS.md -> \"How we write\": British English in prose,"
            "\ncomments and user-facing copy. If the word is a name somebody else chose -- an RFC's,"
            "\na toolkit's, a doc reference to a symbol -- backtick it, or add it to PHRASES/SYMBOLS in"
            "\n%s." % (len(found), Path(__file__).relative_to(REPO_ROOT)),
            file=sys.stderr,
        )
        return 1
    print("OK: prose in %d file(s) is British English." % checked)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
