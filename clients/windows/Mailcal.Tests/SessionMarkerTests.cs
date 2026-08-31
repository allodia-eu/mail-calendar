// The line the rotating log opens with (Services/Log.cs). It has to name the build, because
// `/VERSION` holds the last *released* version (docs/versioning.md): a dev build and the shipped
// one report the same marketing version, so without the package version a log attached to a
// support request cannot be pinned to a build at all (docs/logging.md → "Session marker, and it
// names the build").
//
// This is the Windows twin of Android's FileLogSnapshotTest and it exists for the same reason: the
// rule is invisible to every other gate. A screenshot does not show a log file, the UI Automation
// suite reads no file, and the PACKAGED branch cannot run outside an actual MSIX, which is why
// the composition takes both versions as parameters instead of reading `Package.Current` itself.
//
// Asserted on SHAPE, never on today's numbers, so cutting a release never turns this red.

using System.Text.RegularExpressions;
using Allodia.Mailcal.Services;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class SessionMarkerTests
{
    private const string Device = "Arm64, Microsoft Windows 10.0.26200";

    [Fact]
    public void The_unpackaged_dev_loop_names_the_marketing_version()
    {
        // AppIdentity.PackageVersion is null when there is no package identity, the dev loop
        // (build-and-run.ps1). The marker still has to carry a version; it just has no second one.
        var marker = Log.SessionMarker("0.2.2", null, Device);

        Assert.Matches(@"^--- session start \(\d+\.\d+\.\d+, " + Regex.Escape(Device) + @"\) ---$", marker);
    }

    [Fact]
    public void A_packaged_run_names_the_package_version_too()
    {
        // The MSIX version is the only number that moves per package, so it is what separates two
        // builds that share a marketing version. `package.ps1 -Sign` auto-increments it.
        var marker = Log.SessionMarker("0.2.2", "0.2.2.17", Device);

        Assert.Matches(
            @"^--- session start \(\d+\.\d+\.\d+ package \d+\.\d+\.\d+\.\d+, " + Regex.Escape(Device) + @"\) ---$",
            marker);
    }

    [Fact]
    public void The_device_string_survives_verbatim()
    {
        // The OS build is half of what a support log is read for; a marker that named only the app
        // version would answer "which build" and lose "on what".
        Assert.Contains(Device, Log.SessionMarker("0.2.2", null, Device), StringComparison.Ordinal);
        Assert.Contains(Device, Log.SessionMarker("0.2.2", "0.2.2.17", Device), StringComparison.Ordinal);
    }
}
