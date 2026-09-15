#!/usr/bin/env python3
"""Write the `.appinstaller` that installs and updates a package we host ourselves.

A Windows package distributed outside the Store is an MSIX bundle plus one small XML file. The XML
names the bundle, names every framework package the bundle needs, and says how often an installed
copy should look for a newer one. App Installer remembers the URL it was fetched from, so that file
is the update channel: publish a new bundle, rewrite the XML, and every installation follows.

    python3 scripts/dev/appinstaller.py \
        --main build/Mailcal_0.9.0.0_x64_arm64.msixbundle=https://dl.example/0.9.0/app.msixbundle \
        --dependency runtime-x64.msix=https://dl.example/runtime/2.4.0.0/runtime-x64.msix \
        --dependency runtime-arm64.msix=https://dl.example/runtime/2.4.0.0/runtime-arm64.msix \
        --uri https://dl.example/windows/app.appinstaller \
        --out app.appinstaller

Every identity in the file is **read out of the packages themselves**, never passed in: App Installer
refuses an install where the name, publisher or version it was promised is not the one it fetched,
and a hand-typed publisher is exactly the sort of thing that is right until a certificate renews.
What the caller supplies is where each file will be reachable.

Two gates run before anything is written, and each catches a failure that is otherwise found by a
user rather than by us:

- **the bundle is signed.** An unsigned bundle installs on no machine, and the `.appinstaller`
  around it is what makes that everybody's problem at once.
- **every framework package the bundle asks for is here, for every architecture the bundle
  carries.** A missing one installs fine wherever the runtime happens to be present already, which
  is every developer's machine and not the machine of somebody who has never had the Store.

The 2021 schema is what makes updates arrive at all for a mail client. `AutomaticBackgroundTask`
belongs to it, and without it an installation looks for a newer version only when the app is
launched: a machine that leaves this app open for a fortnight would then go a fortnight without one.
Naming that schema costs nothing, because it wants Windows 10 2004 and the package's own floor is
the same 10.0.19041. A machine that cannot read the file cannot install what it points at.

`OnLaunch` stays beside it, because the two answer different cases: the background task is a check
every eight hours whether or not anyone is using the app, and `OnLaunch` is the check that catches a
machine which was asleep. See docs/windows-channels.md.
"""

from __future__ import annotations

import argparse
import io
import sys
import zipfile
from pathlib import Path
from typing import List, NamedTuple, Tuple
from xml.etree import ElementTree

NAMESPACE = "http://schemas.microsoft.com/appx/appinstaller/2021"
BUNDLE_MANIFEST = "AppxMetadata/AppxBundleManifest.xml"
PACKAGE_MANIFEST = "AppxManifest.xml"
SIGNATURE = "AppxSignature.p7x"

# How long an installed copy may go without asking whether there is a newer one. Eight hours means
# a machine left running over a working day looks once, and a machine restarted in the morning
# looks then. The check is cheap: one request for this file, which is a few hundred bytes.
HOURS_BETWEEN_UPDATE_CHECKS = 8


class Identity(NamedTuple):
    """What a package calls itself, as the package itself says."""

    name: str
    publisher: str
    version: str
    architecture: str


class Dependency(NamedTuple):
    """A framework package one of the bundle's own packages declares it needs."""

    name: str
    architecture: str


class AppInstallerError(RuntimeError):
    """The packages named do not describe something installable."""


def _archive(path: Path) -> zipfile.ZipFile:
    try:
        return zipfile.ZipFile(path)
    except (OSError, zipfile.BadZipFile) as problem:
        raise AppInstallerError("%s is not a package: %s" % (path, problem)) from None


def _element(blob: bytes, what: str) -> ElementTree.Element:
    try:
        return ElementTree.fromstring(blob)
    except ElementTree.ParseError as problem:
        raise AppInstallerError("%s is not readable XML: %s" % (what, problem)) from None


def _identity(root: ElementTree.Element, what: str) -> Identity:
    """The `<Identity>` of a package or a bundle manifest, whichever namespace it is written in."""
    for element in root.iter():
        if element.tag.rsplit("}", 1)[-1] != "Identity":
            continue
        missing = [key for key in ("Name", "Publisher", "Version") if not element.get(key)]
        if missing:
            raise AppInstallerError("%s: <Identity> names no %s" % (what, ", ".join(missing)))
        return Identity(
            name=element.attrib["Name"],
            publisher=element.attrib["Publisher"],
            version=element.attrib["Version"],
            architecture=element.get("ProcessorArchitecture", ""),
        )
    raise AppInstallerError("%s carries no <Identity>" % what)


def read_package(path: Path) -> Identity:
    """What a single `.msix` calls itself. Used for the framework packages the bundle needs."""
    with _archive(path) as archive:
        try:
            blob = archive.read(PACKAGE_MANIFEST)
        except KeyError:
            raise AppInstallerError("%s holds no %s" % (path, PACKAGE_MANIFEST)) from None
    return _identity(_element(blob, str(path)), str(path))


def read_bundle(path: Path) -> Tuple[Identity, List[Dependency]]:
    """What a `.msixbundle` calls itself, and every framework package inside it asks for.

    The dependencies are read from the *inner* packages rather than from the bundle manifest,
    because a bundle manifest does not carry them: it lists the packages, and each package lists
    what it needs. Each answer is tied to the architecture of the package that asked, since that is
    what decides which framework build has to be reachable.
    """
    with _archive(path) as archive:
        names = set(archive.namelist())
        if SIGNATURE not in names:
            raise AppInstallerError(
                "%s is not signed (no %s). An unsigned package installs on no machine, so "
                "publishing an .appinstaller around it would break every installation at once."
                % (path, SIGNATURE)
            )
        try:
            manifest = archive.read(BUNDLE_MANIFEST)
        except KeyError:
            raise AppInstallerError(
                "%s holds no %s, so it is a package rather than a bundle" % (path, BUNDLE_MANIFEST)
            ) from None
        identity = _identity(_element(manifest, str(path)), str(path))

        dependencies: List[Dependency] = []
        for inner, architecture in _application_packages(manifest, str(path)):
            if inner not in names:
                raise AppInstallerError("%s names %s, which is not inside it" % (path, inner))
            with zipfile.ZipFile(io.BytesIO(archive.read(inner))) as package:
                blob = package.read(PACKAGE_MANIFEST)
            for wanted in _package_dependencies(blob, inner):
                dependencies.append(Dependency(wanted, architecture))
    return identity, dependencies


def _application_packages(manifest: bytes, what: str) -> List[Tuple[str, str]]:
    """Each application package in the bundle, as (file name, architecture).

    Resource packages are skipped: they carry no code and declare no framework dependency, and the
    bundle keeps at least one whenever a scale or language was split out.
    """
    found: List[Tuple[str, str]] = []
    for element in _element(manifest, what).iter():
        if element.tag.rsplit("}", 1)[-1] != "Package":
            continue
        if element.get("Type", "application") != "application":
            continue
        file_name = element.get("FileName")
        if not file_name:
            raise AppInstallerError("%s: a <Package> names no FileName" % what)
        found.append((file_name, element.get("Architecture", "neutral")))
    if not found:
        raise AppInstallerError("%s carries no application package" % what)
    return found


def _package_dependencies(manifest: bytes, what: str) -> List[str]:
    """Every framework package one inner package declares it needs."""
    return [
        element.attrib["Name"]
        for element in _element(manifest, what).iter()
        if element.tag.rsplit("}", 1)[-1] == "PackageDependency" and element.get("Name")
    ]


def check_dependencies_covered(wanted: List[Dependency], supplied: List[Identity]) -> None:
    """Refuse a file that would install only where the framework is already there."""
    have = {(one.name, one.architecture) for one in supplied}
    missing = sorted({one for one in set(wanted) if one not in have})
    if missing:
        raise AppInstallerError(
            "the bundle needs framework packages this file does not name: "
            + ", ".join("%s (%s)" % (one.name, one.architecture) for one in missing)
            + ". A machine that already has them would install anyway, which is every machine we "
            "would test this on."
        )


def compose(main: Tuple[Identity, str], dependencies: List[Tuple[Identity, str]], uri: str) -> str:
    """The `.appinstaller` document, as text.

    `Version` is the bundle's own, which is what makes an update detectable: App Installer compares
    the version in the file it fetched against the one it last installed, so a release that moved
    the package version and not this one is a release nobody receives.
    """
    identity, bundle_uri = main
    root = ElementTree.Element(
        "AppInstaller",
        {"xmlns": NAMESPACE, "Version": identity.version, "Uri": uri},
    )
    ElementTree.SubElement(
        root,
        "MainBundle",
        {
            "Name": identity.name,
            "Publisher": identity.publisher,
            "Version": identity.version,
            "Uri": bundle_uri,
        },
    )
    if dependencies:
        block = ElementTree.SubElement(root, "Dependencies")
        for one, dependency_uri in dependencies:
            attributes = {
                "Name": one.name,
                "Publisher": one.publisher,
                "Version": one.version,
                "Uri": dependency_uri,
            }
            if one.architecture:
                attributes["ProcessorArchitecture"] = one.architecture
            ElementTree.SubElement(block, "Package", attributes)
    # Order is the schema's, not a preference: OnLaunch, then AutomaticBackgroundTask.
    settings = ElementTree.SubElement(root, "UpdateSettings")
    ElementTree.SubElement(
        settings, "OnLaunch", {"HoursBetweenUpdateChecks": str(HOURS_BETWEEN_UPDATE_CHECKS)}
    )
    ElementTree.SubElement(settings, "AutomaticBackgroundTask")
    ElementTree.indent(root, space="  ")
    body = ElementTree.tostring(root, encoding="unicode")
    return '<?xml version="1.0" encoding="utf-8"?>\n%s\n' % body


def _pair(raw: str, what: str) -> Tuple[Path, str]:
    """`<path>=<uri>`, split at the first `=` so a Windows drive letter survives."""
    path, separator, uri = raw.partition("=")
    if not separator or not path.strip() or not uri.strip():
        raise argparse.ArgumentTypeError("--%s wants <path>=<uri>, got %r" % (what, raw))
    if not uri.startswith("https://"):
        # App Installer will fetch whatever it is told to. A package delivered over plain HTTP is
        # one an intermediary chose, and the signature check that follows only proves it was signed
        # by somebody, not that it is the release we published.
        raise argparse.ArgumentTypeError("%s is not an https URL" % uri)
    return Path(path.strip()), uri.strip()


def build(main: str, dependencies: List[str], uri: str) -> str:
    """Read every package named, check the set is complete, and return the document."""
    main_path, main_uri = _pair(main, "main")
    identity, wanted = read_bundle(main_path)

    supplied: List[Tuple[Identity, str]] = []
    for raw in dependencies:
        path, dependency_uri = _pair(raw, "dependency")
        supplied.append((read_package(path), dependency_uri))
    check_dependencies_covered(wanted, [one for one, _ in supplied])
    return compose((identity, main_uri), supplied, uri)


def main(argv: List[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--main", required=True, metavar="PATH=URI", help="the signed .msixbundle, and its URL"
    )
    parser.add_argument(
        "--dependency",
        action="append",
        default=[],
        metavar="PATH=URI",
        help="a framework package the bundle needs, and its URL; repeatable",
    )
    parser.add_argument("--uri", required=True, help="where this file itself will be reachable")
    parser.add_argument("--out", type=Path, help="where to write (default: stdout)")
    args = parser.parse_args(argv)

    if not args.uri.startswith("https://"):
        print("appinstaller: --uri is not an https URL", file=sys.stderr)
        return 1
    try:
        document = build(args.main, args.dependency, args.uri)
    except AppInstallerError as problem:
        print("appinstaller: %s" % problem, file=sys.stderr)
        return 1
    if args.out is None:
        sys.stdout.write(document)
    else:
        args.out.write_text(document, encoding="utf-8")
        print("wrote %s" % args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
