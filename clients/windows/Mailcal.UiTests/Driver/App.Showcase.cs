// The showcase launch: the seeded, in-memory dataset a store capture photographs, with the two
// guards showcase.ps1 holds, because "the app is showing fictional mail" is the whole safety
// property of a showcase run and nothing on screen reveals whether it holds. A stale or Release
// build, or a flag that never reached the process, opens the developer's real accounts instead,
// and a suite would then read their mail.
//
//   1. Before launching: the built assembly carries the showcase driver.
//   2. After launching: the core logged that this process entered showcase mode, in the locale
//      asked for. The line comes from the in-memory engine's constructor, so it proves the
//      fictional engine was built, not merely that a flag was read.

using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

namespace Allodia.Mailcal.UiTests.Driver;

/// <summary>What a launch runs on.</summary>
internal enum Dataset
{
    /// <summary>The local Stalwart harness, over a real transport.</summary>
    Harness,

    /// <summary>An emptied store, on the account-setup form.</summary>
    FirstRun,

    /// <summary>The in-memory showcase seed: two accounts, deterministic, per locale.</summary>
    Showcase,
}

internal sealed partial class App
{
    /// <summary>
    /// The showcase switches a launch starts from, each overridable by a fixture's own: English,
    /// on the list, and light. Light rather than unset, because unset means whatever this desktop
    /// is set to, and a suite measuring colour would measure the developer's theme.
    /// </summary>
    private static readonly IReadOnlyDictionary<string, string> ShowcaseDefaults = new Dictionary<string, string>
    {
        ["MAILCAL_SHOWCASE"] = "en",
        ["MAILCAL_SHOWCASE_SCREEN"] = "list",
        ["MAILCAL_APPEARANCE"] = "light",
    };

    /// <summary>
    /// Guard 1: the built assembly knows MAILCAL_SHOWCASE_SCREEN. Searched as raw bytes, because a
    /// .NET string literal sits in the assembly as UTF-16 at no guaranteed alignment; Latin-1 maps
    /// each byte to one char, so an ordinal search of both is an alignment-free byte search. A
    /// Release build drops the driver and its strings with it.
    /// </summary>
    private static void AssertShowcaseDriver()
    {
        var dll = Path.Combine(Path.GetDirectoryName(Exe())!, "Mailcal.dll");
        var haystack = Encoding.Latin1.GetString(File.ReadAllBytes(dll));
        var needle = Encoding.Latin1.GetString(Encoding.Unicode.GetBytes("MAILCAL_SHOWCASE_SCREEN"));
        if (!haystack.Contains(needle, StringComparison.Ordinal))
        {
            throw new InvalidOperationException(
                $"{dll} has no showcase driver in it: it is stale, or a Release build, and launched with MAILCAL_SHOWCASE it would open the real accounts. Rebuild with clients/windows/build-and-run.ps1 -NoRun");
        }
        // The showcase pins a 1440x900 window. Clipped, a screen grab past the edge returns black and
        // a NavigationView below its breakpoint changes display mode, both silently.
        var screen = Screen.PrimaryScreen!.Bounds;
        var scale = GetDpiForSystem() / 96.0;
        if (screen.Width / scale < 1440 || screen.Height / scale < 900)
        {
            throw new InvalidOperationException(
                $"the desktop is {screen.Width / scale:0}x{screen.Height / scale:0} logical pixels, and the showcase suites measure a 1440x900 window. Raise the resolution (uitests/set-desktop-resolution.ps1)");
        }
    }

    /// <summary>
    /// Guard 2, then readiness: the core's marker for this launch's locale, and on the list screen
    /// the rows. The account-less add-account screen logs a marker of its own shape. A launch that
    /// never logs it is stopped at once, since whatever is on screen may be real mail.
    /// </summary>
    private void WaitShowcase(DateTimeOffset launchedAt, Dictionary<string, string?> environment)
    {
        var locale = environment["MAILCAL_SHOWCASE"]!;
        var seeded = char.ToUpperInvariant(locale[0]) + locale[1..];
        var screen = environment["MAILCAL_SHOWCASE_SCREEN"];
        var marker = screen == "add-account"
            ? $"showcase (screenshot) app starting (in-memory engine, no account, {seeded} language)"
            : $"showcase (screenshot) app starting (in-memory engine, seeded {seeded} sample content)";
        if (!Eventually(() => AppLog.Session(launchedAt).Contains(marker, StringComparison.Ordinal), TimeSpan.FromSeconds(30)))
        {
            Dispose();
            throw new InvalidOperationException($"this launch did not enter showcase mode for '{locale}' within 30 s, so it was stopped: whatever it showed may be real mail. Looked for: {marker}");
        }
        if (screen == "list" && (Agent.Wait(Filter.ById("RowsList"), TimeSpan.FromSeconds(60)) is not { } list
            || Agent.Wait(new Filter { Type = "ListItem", Under = list }, TimeSpan.FromSeconds(60)) is null))
        {
            throw new InvalidOperationException("the showcase never listed any mail");
        }
        Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
    }

    [DllImport("user32.dll")] private static extern uint GetDpiForSystem();
}
