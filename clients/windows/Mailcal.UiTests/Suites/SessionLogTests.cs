// The running app writes a session-start line that names the build it came from (docs/logging.md,
// "Session marker, and it names the build"). SessionMarkerTests pins the composition; this is the
// other failure, Log.Init losing the version or never being reached, which every headless gate and
// every screenshot passes. Log.Init runs in Program.Main before the app starts, so the case anchors
// to the live process rather than to the file alone. The showcase, because a suite never opens real
// mail.

using System.Diagnostics;
using System.Globalization;
using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed partial class SessionLogTests(ShowcaseApp showcase)
{
    // `2026-08-01 08:14:25.602 +02:00 [info] --- session start (0.2.2, Arm64, Microsoft Windows ...) ---`
    [GeneratedRegex(@"^(?<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3} [+-]\d{2}:\d{2}) \[info\] --- session start \((?<body>.+)\) ---$")]
    private static partial Regex Marker();

    // `0.2.2` unpackaged, `0.2.2 package 0.2.2.17` inside an MSIX, then the device. A shape, not
    // today's numbers: a release never turns this red.
    [GeneratedRegex(@"^\d+\.\d+\.\d+( package \d+\.\d+\.\d+\.\d+)?, .+$")]
    private static partial Regex Version();

    /// <summary>The newest session-start line, through AppLog: rotation runs before every write, so
    /// a live session's marker can already be in app.log.1.</summary>
    private static (DateTimeOffset When, string Body) NewestMarker()
    {
        var line = AppLog.NewestSession().Split('\n').Select(l => l.TrimEnd('\r')).LastOrDefault(l => l.Contains(" --- session start (", StringComparison.Ordinal))
            ?? throw new InvalidOperationException("the log holds no session-start line at all");
        var marker = Marker().Match(line);
        return marker.Success
            ? (DateTimeOffset.Parse(marker.Groups["ts"].Value, CultureInfo.InvariantCulture), marker.Groups["body"].Value)
            : throw new InvalidOperationException($"the session-start line no longer parses: {line}");
    }

    [OfflineFact]
    public void TheRunningAppStampedASessionMarkerOfItsOwn() => showcase.Case(nameof(TheRunningAppStampedASessionMarkerOfItsOwn), app =>
    {
        // Against the live process, not a wall-clock window: a marker left by an earlier run would
        // otherwise pass against an app that logs nothing. A second of slack for the two clocks.
        var started = new DateTimeOffset(Process.GetProcessById(app.ProcessId).StartTime).AddSeconds(-1);
        var (when, _) = NewestMarker();
        Assert.True(when >= started, $"the newest session marker is from {when}, the running app started at {started}: this process wrote none, so Log.Init was never reached");
    });

    [OfflineFact]
    public void TheSessionMarkerNamesTheBuildTheLogCameFrom() => showcase.Case(nameof(TheSessionMarkerNamesTheBuildTheLogCameFrom), _ =>
    {
        // /VERSION holds the last released version, so a log with no version in it cannot be pinned
        // to a build and is no use as a support artefact.
        var (_, body) = NewestMarker();
        Assert.True(Version().IsMatch(body), $"expected \"<x.y.z>[ package <a.b.c.d>], <device>\", logged: {body}");
    });
}
