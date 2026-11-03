// Mail's times follow the 12/24-hour setting in Settings → General, as the calendar's do
// (docs/timestamps.md). Mailcal.Tests proves the formatter draws either clock; what only a drawn
// window shows is the wiring: a pick reaches a list that is already on screen, whose rows are
// reconciled rather than rebuilt, a reading header already drawn, and the header a row opens with.
//
// THE FIXTURE is the en showcase seed: "Welcome to Allodia Mail & Calendar" arrives today, so its
// row shows a time of day rather than a weekday (crates/mailcal-bindings/src/showcase_data). The
// showcase has no store, so a pick here is never written; the cleanup puts 24-hour back for the
// suites that share the launch.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed partial class MailClockTests(ShowcaseApp showcase)
{
    private const string Today = "Welcome to Allodia Mail*";
    private const string TodaySubject = "Welcome to Allodia Mail & Calendar";

    [GeneratedRegex(@"^\d{2}:\d{2}$")]
    private static partial Regex TwentyFourHour();

    [GeneratedRegex(@"^\d{1,2}:\d{2} (AM|PM)$")]
    private static partial Regex TwelveHour();

    // The reading header's full date ends in the same time of day.
    [GeneratedRegex(@" \d{2}:\d{2}$")]
    private static partial Regex EndsTwentyFourHour();

    [GeneratedRegex(@" \d{1,2}:\d{2} (AM|PM)$")]
    private static partial Regex EndsTwelveHour();

    /// <summary>The time the row titled <see cref="Today"/> draws: its one text shaped like a
    /// time of day in either clock.</summary>
    private static string? RowTime(App app) =>
        app.Agent.FindAll(new Filter { Type = "Text", Under = app.RowTitled(Today) })
            .Select(text => text.Name)
            .FirstOrDefault(name => TwentyFourHour().IsMatch(name) || TwelveHour().IsMatch(name));

    private static string HeaderDate(App app) =>
        app.Agent.Find(Filter.ById("DateText") with { Window = app.Mailbox.Hwnd, OnScreen = true })?.Name ?? "";

    private static void PickClock(App app, string key)
    {
        var dialog = new SetupFlow(app).OpenCategory("general");
        var choice = app.Agent.Find(new Filter { Type = "RadioButton", Name = Catalog.English(key), Under = dialog })
            ?? throw new InvalidOperationException($"Settings → General offers no '{Catalog.English(key)}'");
        app.Agent.Select(choice);
        app.CloseDialogs();
    }

    private static void Holds(Func<string?> read, Regex shape, string because)
    {
        string? seen = null;
        App.Eventually(() => (seen = read()) is { } value && shape.IsMatch(value), TimeSpan.FromSeconds(5));
        Assert.True(seen is not null && shape.IsMatch(seen), $"read '{seen}': {because}");
    }

    [OfflineFact]
    public void AListRowAlreadyOnScreenRedrawsItsTimeOnTheOtherClock() => showcase.Case(nameof(AListRowAlreadyOnScreenRedrawsItsTimeOnTheOtherClock), app =>
    {
        Holds(() => RowTime(app), TwentyFourHour(), "the launch is on 24-hour, the core's default");
        PickClock(app, "settings_time_format_12");
        Holds(() => RowTime(app), TwelveHour(),
            "the row is reconciled, not rebuilt, so a clock change has to reach the label the row already drew");
        PickClock(app, "settings_time_format_24");
        Holds(() => RowTime(app), TwentyFourHour(), "and picking back has to move it back");
    }, app => PickClock(app, "settings_time_format_24"));

    [OfflineFact]
    public void AnOpenMessagesHeaderRedrawsItsDateOnTheOtherClock() => showcase.Case(nameof(AnOpenMessagesHeaderRedrawsItsDateOnTheOtherClock), app =>
    {
        app.OpenMessage(TodaySubject);
        Holds(() => HeaderDate(app), EndsTwentyFourHour(), "the launch is on 24-hour");
        PickClock(app, "settings_time_format_12");
        Holds(() => HeaderDate(app), EndsTwelveHour(),
            "the header is drawn once, when the message opens, so a clock change has to reach it where it stands");
        PickClock(app, "settings_time_format_24");
        Holds(() => HeaderDate(app), EndsTwentyFourHour(), "and picking back has to move it back");
    }, app => PickClock(app, "settings_time_format_24"));

    [OfflineFact]
    public void AMessageOpenedAfterThePickOpensOnTheNewClock() => showcase.Case(nameof(AMessageOpenedAfterThePickOpensOnTheNewClock), app =>
    {
        app.OpenMessage("Your June usage report");
        PickClock(app, "settings_time_format_12");
        app.OpenMessage(TodaySubject);
        Holds(() => HeaderDate(app), EndsTwelveHour(),
            "a row kept across the pick still has to open on the clock the setting names");
    }, app => PickClock(app, "settings_time_format_24"));
}
