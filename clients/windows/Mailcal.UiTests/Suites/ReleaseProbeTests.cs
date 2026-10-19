// What the app closes, the runtime gets back: a composer, an editor's web view, a reading window and
// the composer window raised from it, a signature editor.
//
// A closed window never freed looks exactly like one that is: its WebView2 renderer still exits, so
// even a process count stays flat, and what grows is the app's own memory, a few megabytes a window.
// MAILCAL_RELEASE_PROBE has the debug build watch what it closes, collect until it is gone and log
// what is still held (Services/ReleaseProbe.cs); this reads that line. The showcase, so CI runs it.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

/// <summary>The showcase with the release probe on.</summary>
public sealed class ReleaseProbeApp : ShowcaseApp
{
    protected override IReadOnlyDictionary<string, string> Env { get; } =
        new Dictionary<string, string> { ["MAILCAL_RELEASE_PROBE"] = "1" };
}

[Trait("Dataset", "showcase")]
public sealed partial class ReleaseProbeTests(ReleaseProbeApp showcase) : IClassFixture<ReleaseProbeApp>
{
    private const int Cycles = 3;

    [GeneratedRegex(@"\[info\] release probe: watching a closed ")]
    private static partial Regex Watch();

    [GeneratedRegex(@"\[info\] release probe: (?<held>\d+) of (?<total>\d+) closed still held(?::(?<which>[ #\d]+))?")]
    private static partial Regex Answer();

    [GeneratedRegex(@"#(\d+)")]
    private static partial Regex Numbered();

    /// <summary>How many closes the probe has been told of this launch. Each is numbered in that
    /// order, so a case's closes are the ones numbered above what this said before it began.</summary>
    private static int Watched() => Watch().Matches(AppLog.NewestSession()).Count;

    /// <summary>
    /// Waits for the probe to answer for every close after <paramref name="before"/>, and fails if
    /// any of them is still held. Judged by the numbers the probe names, not its totals, which run
    /// for the whole launch; and on an answer after the last watch, since an earlier one answered for
    /// less. The probe gives up after about thirty seconds of collections.
    /// </summary>
    private static void AssertReleasedSince(int before, string what)
    {
        Match? answer = null;
        var watches = 0;
        App.Eventually(() =>
        {
            var session = AppLog.NewestSession();
            var watched = Watch().Matches(session);
            var answers = Answer().Matches(session);
            watches = watched.Count;
            answer = watches > before && answers.Count > 0 && answers[^1].Index > watched[^1].Index ? answers[^1] : null;
            return answer is not null;
        }, TimeSpan.FromSeconds(60));
        if (answer is null)
        {
            throw new InvalidOperationException(watches <= before
                ? $"the probe was never told of a close when {what}, so this proved nothing: is MAILCAL_RELEASE_PROBE reaching a Debug build?"
                : $"the probe did not answer within 60 s of the closes when {what}");
        }
        Assert.True(int.Parse(answer.Groups["total"].Value, System.Globalization.CultureInfo.InvariantCulture) == watches,
            "the probe answered for a different number of closes than it logged, so the line is not about this case");
        var held = Numbered().Matches(answer.Groups["which"].Value)
            .Select(m => int.Parse(m.Groups[1].Value, System.Globalization.CultureInfo.InvariantCulture)).Where(n => n > before).ToList();
        Assert.True(held.Count == 0,
            $"{held.Count} of the {watches - before} object(s) closed when {what} are still in memory: something the app keeps, often a handler on a native object, still refers to them (docs/client-traps.md)");
    }

    private static void Compose(App app)
    {
        app.Agent.Invoke(app.Agent.Wait(Filter.ById("ComposeButton"), App.Patience) ?? throw new InvalidOperationException("no ComposeButton"));
        if (app.Agent.Wait(Filter.ById("SendButton"), App.Patience) is null)
        {
            throw new InvalidOperationException("no composer within 30 s of pressing Compose");
        }
    }

    [ShowcaseFact]
    public void AComposerClosedInThePaneIsReleased() => showcase.Case(nameof(AComposerClosedInThePaneIsReleased), app =>
    {
        var before = Watched();
        for (var i = 0; i < Cycles; i++)
        {
            // Seeded, so its web view is a live one when the composer goes.
            new DraftsFlow(app).OpenNewComposer();
            app.CloseComposer();
        }
        AssertReleasedSince(before, $"{Cycles} composers were discarded");
    });

    [ShowcaseFact]
    public void AComposerDiscardedBeforeItsEditorHasLoadedIsReleased() => showcase.Case(nameof(AComposerDiscardedBeforeItsEditorHasLoadedIsReleased), app =>
    {
        // Discarded while the web view may still be starting: nothing waits for the editor first.
        var before = Watched();
        for (var i = 0; i < Cycles; i++)
        {
            Compose(app);
            app.CloseComposer();
        }
        AssertReleasedSince(before, $"{Cycles} composers were discarded at once");
    });

    [ShowcaseFact]
    public void AReadingWindowAndTheComposerWindowRaisedFromItAreReleased() => showcase.Case(nameof(AReadingWindowAndTheComposerWindowRaisedFromItAreReleased), app =>
    {
        var before = Watched();
        for (var i = 0; i < Cycles; i++)
        {
            app.ShowMailbox();
            Input.Click(app.Steady(app.Rows().First(r => r.Enabled)), times: 2);
            AppWindow? reading = null;
            if (!App.Eventually(() => (reading = app.ExtraWindows().FirstOrDefault()) is not null, TimeSpan.FromSeconds(20)))
            {
                throw new InvalidOperationException("double-clicking the first row opened no window");
            }
            app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingReply", "Button") with { Window = reading!.Hwnd }, TimeSpan.FromSeconds(20))
                ?? throw new InvalidOperationException("the reading window never offered Reply"));
            if (!App.Eventually(() => app.ExtraWindows().Count >= 2, TimeSpan.FromSeconds(30)))
            {
                throw new InvalidOperationException("Reply in the reading window opened no composer window");
            }
            // The editor loads, so its web view is a live one when the window closes; a window's own
            // close keeps an untouched reply without asking.
            app.Agent.Wait(Filter.ById("Editor") with { Signal = "seeded" }, App.Patience);
            app.CloseExtraWindows();
        }
        AssertReleasedSince(before, $"{Cycles} reading and composer windows were closed");
    });

    [ShowcaseFact]
    public void ASignatureEditorIsReleasedWhenItIsCancelled() => showcase.Case(nameof(ASignatureEditorIsReleasedWhenItIsCancelled), app =>
    {
        var before = Watched();
        var settings = new SetupFlow(app);
        for (var i = 0; i < Cycles; i++)
        {
            var dialog = settings.OpenCategory("signatures");
            app.Agent.Invoke(app.Agent.Find(new Filter { Type = "Button", Name = Catalog.English("settings_signatures_add"), Under = dialog })
                ?? throw new InvalidOperationException("Settings → Signatures offers no New signature"));
            var cancel = app.Agent.Wait(new Filter { Type = "Button", Name = Catalog.English("action_cancel"), Under = dialog }, TimeSpan.FromSeconds(10))
                ?? throw new InvalidOperationException("the signature editor offers no Cancel");
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromSeconds(1));
            app.Agent.Invoke(cancel);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        }
        app.CloseDialogs();
        AssertReleasedSince(before, $"{Cycles} signature editors were cancelled");
    }, app => app.CloseDialogs());
}
