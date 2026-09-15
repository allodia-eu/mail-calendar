"""Tests `scripts/dev/appinstaller.py`, the file that carries a Windows release outside the Store.

Everything this generator gets wrong fails on somebody else's machine. A publisher that does not
match the signature, a framework package nobody hosted, a version that did not move: each installs
perfectly on a developer's box, where the runtime is present and the app is already there, and
refuses on the machine of the person the direct download exists for.

So the packages are built here, in a temporary directory, with the shapes MSIX actually has: a
bundle is a zip holding a bundle manifest, a signature and one `.msix` per architecture, and each of
those is a zip holding a package manifest. That is enough for every rule under test, and it means
the suite runs on any host with no Windows SDK.
"""

from __future__ import annotations

import sys
import unittest
import zipfile
from pathlib import Path
from xml.etree import ElementTree

REPO_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO_ROOT / "scripts" / "dev"))

from appinstaller import (  # noqa: E402
    AppInstallerError,
    Dependency,
    Identity,
    build,
    check_dependencies_covered,
    compose,
    read_bundle,
)

NAMESPACE = "{http://schemas.microsoft.com/appx/appinstaller/2021}"
RUNTIME = "Microsoft.WindowsAppRuntime.2"
MICROSOFT = "CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US"
PUBLISHER = "CN=Example EU, O=Example EU, L=Amsterdam, C=NL"

PACKAGE_MANIFEST = """<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10">
  <Identity Name="{name}" Publisher="{publisher}" Version="{version}"
            ProcessorArchitecture="{architecture}" />
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.19041.0"
                        MaxVersionTested="10.0.26100.0" />
    {dependencies}
  </Dependencies>
</Package>
"""

BUNDLE_MANIFEST = """<?xml version="1.0" encoding="utf-8"?>
<Bundle xmlns="http://schemas.microsoft.com/appx/2013/bundle" SchemaVersion="5.0">
  <Identity Name="{name}" Publisher="{publisher}" Version="{version}" />
  <Packages>
    {packages}
  </Packages>
</Bundle>
"""


def write_package(
    path: Path,
    name: str,
    publisher: str,
    version: str,
    architecture: str,
    needs: list,
    signed: bool = False,
) -> Path:
    """One `.msix`: a zip carrying a package manifest."""
    dependencies = "\n    ".join(
        '<PackageDependency Name="%s" MinVersion="2.4.0.0" Publisher="%s" />' % (one, MICROSOFT)
        for one in needs
    )
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(
            "AppxManifest.xml",
            PACKAGE_MANIFEST.format(
                name=name,
                publisher=publisher,
                version=version,
                architecture=architecture,
                dependencies=dependencies,
            ),
        )
        if signed:
            archive.writestr("AppxSignature.p7x", b"not a real signature")
    return path


def write_bundle(
    path: Path,
    name: str = "Example.MailCalendar",
    publisher: str = PUBLISHER,
    version: str = "0.9.0.0",
    architectures: tuple = ("x64", "arm64"),
    needs: tuple = (RUNTIME,),
    signed: bool = True,
) -> Path:
    """One `.msixbundle`: a zip carrying a bundle manifest, a signature and the inner packages."""
    entries = []
    inner = []
    for architecture in architectures:
        file_name = "%s_%s_%s.msix" % (name, version, architecture)
        entries.append(
            '<Package Type="application" Version="%s" Architecture="%s" FileName="%s" />'
            % (version, architecture, file_name)
        )
        dependencies = "\n    ".join(
            '<PackageDependency Name="%s" MinVersion="2.4.0.0" Publisher="%s" />' % (one, MICROSOFT)
            for one in needs
        )
        inner.append(
            (
                file_name,
                PACKAGE_MANIFEST.format(
                    name=name,
                    publisher=publisher,
                    version=version,
                    architecture=architecture,
                    dependencies=dependencies,
                ),
            )
        )
    # A resource package rides along, as a real bundle's does: it declares nothing and must not be
    # read as an architecture needing a framework of its own.
    entries.append(
        '<Package Type="resource" Version="%s" ResourceId="split.scale-200" '
        'FileName="resources.scale-200.msix" />' % version
    )
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(
            "AppxMetadata/AppxBundleManifest.xml",
            BUNDLE_MANIFEST.format(
                name=name, publisher=publisher, version=version, packages="\n    ".join(entries)
            ),
        )
        for file_name, manifest in inner:
            nested = path.with_name(file_name)
            with zipfile.ZipFile(nested, "w") as package:
                package.writestr("AppxManifest.xml", manifest)
            archive.write(nested, file_name)
            nested.unlink()
        if signed:
            archive.writestr("AppxSignature.p7x", b"not a real signature")
    return path


class BundleReading(unittest.TestCase):
    def setUp(self) -> None:
        self.here = Path(__file__).resolve().parent / "_appinstaller"
        self.here.mkdir(exist_ok=True)
        self.addCleanup(self._clean)

    def _clean(self) -> None:
        for path in sorted(self.here.rglob("*")):
            path.unlink()
        self.here.rmdir()

    def test_the_identity_comes_from_the_bundle_rather_than_the_caller(self) -> None:
        identity, _ = read_bundle(write_bundle(self.here / "app.msixbundle"))
        self.assertEqual(identity.name, "Example.MailCalendar")
        self.assertEqual(identity.publisher, PUBLISHER)
        self.assertEqual(identity.version, "0.9.0.0")

    def test_a_dependency_is_read_per_architecture(self) -> None:
        """What has to be hosted is one framework build per architecture, not one per name."""
        _, wanted = read_bundle(write_bundle(self.here / "app.msixbundle"))
        self.assertEqual(
            sorted(wanted), sorted([Dependency(RUNTIME, "x64"), Dependency(RUNTIME, "arm64")])
        )

    def test_a_resource_package_asks_for_nothing(self) -> None:
        """A resource package carries no code; reading one as an architecture would demand a
        framework build that does not exist."""
        _, wanted = read_bundle(write_bundle(self.here / "app.msixbundle"))
        self.assertNotIn("neutral", {one.architecture for one in wanted})

    def test_an_unsigned_bundle_is_refused(self) -> None:
        """The failure this catches is total and silent: nobody can install it, and the file
        pointing at it says nothing is wrong."""
        path = write_bundle(self.here / "app.msixbundle", signed=False)
        with self.assertRaises(AppInstallerError) as refused:
            read_bundle(path)
        self.assertIn("not signed", str(refused.exception))

    def test_a_package_that_is_not_a_bundle_is_refused(self) -> None:
        path = write_package(
            self.here / "app.msix",
            "Example.MailCalendar",
            PUBLISHER,
            "0.9.0.0",
            "x64",
            [RUNTIME],
            signed=True,
        )
        with self.assertRaises(AppInstallerError) as refused:
            read_bundle(path)
        self.assertIn("rather than a bundle", str(refused.exception))


class DependencyCoverage(unittest.TestCase):
    def test_a_framework_missing_for_one_architecture_is_refused(self) -> None:
        """The half-published case, which is the one that gets shipped: the machine it was tested
        on had the other architecture."""
        with self.assertRaises(AppInstallerError) as refused:
            check_dependencies_covered(
                [Dependency(RUNTIME, "x64"), Dependency(RUNTIME, "arm64")],
                [Identity(RUNTIME, MICROSOFT, "2.4.0.0", "x64")],
            )
        self.assertIn("arm64", str(refused.exception))

    def test_a_complete_set_passes(self) -> None:
        check_dependencies_covered(
            [Dependency(RUNTIME, "x64"), Dependency(RUNTIME, "arm64")],
            [
                Identity(RUNTIME, MICROSOFT, "2.4.0.0", "x64"),
                Identity(RUNTIME, MICROSOFT, "2.4.0.0", "arm64"),
            ],
        )


class Document(unittest.TestCase):
    def document(self) -> ElementTree.Element:
        text = compose(
            (Identity("Example.MailCalendar", PUBLISHER, "0.9.0.0", ""), "https://d/a.msixbundle"),
            [(Identity(RUNTIME, MICROSOFT, "2.4.0.0", "x64"), "https://d/r-x64.msix")],
            "https://d/windows/example.appinstaller",
        )
        return ElementTree.fromstring(text)

    def test_the_file_version_is_the_package_version(self) -> None:
        """An update is detected by comparing this against what is installed, so a release that
        moved the package and not this one reaches nobody."""
        self.assertEqual(self.document().get("Version"), "0.9.0.0")

    def test_the_bundle_is_named_exactly_as_the_bundle_names_itself(self) -> None:
        bundle = self.document().find(NAMESPACE + "MainBundle")
        self.assertIsNotNone(bundle)
        self.assertEqual(bundle.get("Publisher"), PUBLISHER)
        self.assertEqual(bundle.get("Uri"), "https://d/a.msixbundle")

    def test_a_dependency_carries_its_architecture(self) -> None:
        package = self.document().find(NAMESPACE + "Dependencies/" + NAMESPACE + "Package")
        self.assertIsNotNone(package)
        self.assertEqual(package.get("ProcessorArchitecture"), "x64")
        self.assertEqual(package.get("Publisher"), MICROSOFT)

    def test_an_installation_is_told_to_look_for_updates(self) -> None:
        """Without this element the file installs the app once and never updates it, which is the
        one thing the Store does for us and this has to replace."""
        launch = self.document().find(NAMESPACE + "UpdateSettings/" + NAMESPACE + "OnLaunch")
        self.assertIsNotNone(launch)
        self.assertTrue(int(launch.get("HoursBetweenUpdateChecks")) > 0)

    def test_it_also_looks_while_the_app_is_open(self) -> None:
        """OnLaunch alone is a check a mail client rarely reaches: this one is left running for
        days, so a launch can be a fortnight apart and the updates with it."""
        settings = NAMESPACE + "UpdateSettings/" + NAMESPACE
        self.assertIsNotNone(self.document().find(settings + "AutomaticBackgroundTask"))

    def test_the_schema_is_the_one_that_has_a_background_task(self) -> None:
        """AutomaticBackgroundTask is unprefixed only under the 2021 namespace, and under an older
        one it is an element App Installer does not know. The package floor is the same 10.0.19041
        the schema wants, so nothing is given up by naming it."""
        self.assertIn("appinstaller/2021", NAMESPACE)


class Arguments(unittest.TestCase):
    def setUp(self) -> None:
        self.here = Path(__file__).resolve().parent / "_appinstaller_args"
        self.here.mkdir(exist_ok=True)
        self.addCleanup(self._clean)

    def _clean(self) -> None:
        for path in sorted(self.here.rglob("*")):
            path.unlink()
        self.here.rmdir()

    def test_a_plain_http_url_is_refused(self) -> None:
        """A package fetched over http is one an intermediary chose."""
        bundle = write_bundle(self.here / "app.msixbundle")
        with self.assertRaises(Exception) as refused:
            build("%s=http://d/a.msixbundle" % bundle, [], "https://d/x.appinstaller")
        self.assertIn("https", str(refused.exception))

    def test_a_windows_path_survives_the_split(self) -> None:
        """`--main` is `<path>=<uri>` and a drive letter carries a colon, not an equals sign, so
        the split is at the first `=` and `D:\\build\\app.msixbundle=https://…` stays whole."""
        bundle = write_bundle(self.here / "app.msixbundle")
        runtime = write_package(
            self.here / "runtime-x64.msix", RUNTIME, MICROSOFT, "2.4.0.0", "x64", []
        )
        arm = write_package(
            self.here / "runtime-arm64.msix", RUNTIME, MICROSOFT, "2.4.0.0", "arm64", []
        )
        document = build(
            "%s=https://d/a.msixbundle" % bundle,
            ["%s=https://d/r-x64.msix" % runtime, "%s=https://d/r-arm64.msix" % arm],
            "https://d/x.appinstaller",
        )
        self.assertIn('Uri="https://d/a.msixbundle"', document)


if __name__ == "__main__":
    unittest.main()
