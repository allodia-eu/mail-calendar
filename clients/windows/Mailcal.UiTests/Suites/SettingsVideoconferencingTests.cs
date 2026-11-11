// Settings → Videoconferencing (docs/settings.md slot 4): the person's Jitsi server
// (docs/conferencing.md, rule 13). What the core accepts is pinned in Rust; what this is for is the
// client wiring: the category missing or out of place, a field that never reaches the core, a
// refusal nobody is told about, or a field that shows what was typed rather than what was stored.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SettingsVideoconferencingTests(ShowcaseApp showcase)
{
    private const string Tag = "videoconferencing";

    private static Element Box(App app, Element dialog) =>
        app.Agent.Find(new Filter { Id = "JitsiServerBox", Under = dialog })
        ?? throw new InvalidOperationException("Settings → Videoconferencing draws no Jitsi server field (docs/conferencing.md, rule 13)");

    private static string Value(App app, Element dialog) => app.Agent.Read(Box(app, dialog)).Value ?? "";

    private static bool Refused(App app, Element dialog) =>
        app.Agent.Find(new Filter { Id = "JitsiServerRefused", Under = dialog }) is { Rendered: true };

    /// <summary>
    /// Types into the field and moves focus off it, which is what commits: a value set on a box
    /// that never had the keyboard raises no LostFocus and sends nothing. The keyboard goes to the
    /// category's own row, somewhere real.
    /// </summary>
    private static void Type(App app, Element dialog, string text)
    {
        var box = Box(app, dialog);
        app.Agent.Focus(box);
        app.Agent.SetText(box, text);
        app.Agent.Focus(app.Agent.Find(new Filter { Id = $"SettingsCategory_{Tag}", Under = dialog })!);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
    }

    [OfflineFact]
    public void SettingsOffersVideoconferencingBetweenCalendarAndReading() => showcase.Case(nameof(SettingsOffersVideoconferencingBetweenCalendarAndReading), app =>
    {
        var categories = new SetupFlow(app).Categories();
        var at = categories.IndexOf(Tag);
        Assert.True(at > 0 && at < categories.Count - 1 && categories[at - 1] == "calendar" && categories[at + 1] == "reading",
            $"Videoconferencing sits between Calendar and Reading on every platform (docs/settings.md); the order is {string.Join(" | ", categories)}");
    }, app => app.CloseDialogs());

    [OfflineFact]
    public void TheFieldStoresWhatTheCoreAcceptsAndSaysWhenItRefuses()
    {
        string? was = null;
        showcase.Case(nameof(TheFieldStoresWhatTheCoreAcceptsAndSaysWhenItRefuses), app =>
        {
            var settings = new SetupFlow(app);
            var dialog = settings.OpenCategory(Tag);
            was = Value(app, dialog);

            // Refused: said under the field, and nothing stored, so coming back shows what was there.
            Type(app, dialog, "http://meet.example.org");
            Assert.True(Refused(app, dialog), "an address the core refuses is said under the field (rule 13)");
            settings.OpenCategory("calendar");
            Assert.Equal(was, Value(app, settings.OpenCategory(Tag)));

            // Accepted: the panel is rebuilt from the core when it is shown again, so this is what
            // the core holds, in its stored form.
            dialog = settings.OpenCategory(Tag);
            Type(app, dialog, "https://meet.example.org/");
            Assert.False(Refused(app, dialog), "an address the core accepted is not refused");
            settings.OpenCategory("calendar");
            Assert.Equal("https://meet.example.org", Value(app, settings.OpenCategory(Tag)));
        }, app =>
        {
            // Leave the preference as the suite found it.
            var settings = new SetupFlow(app);
            var dialog = settings.OpenCategory(Tag);
            if (was is not null && Value(app, dialog) != was)
            {
                Type(app, dialog, was);
            }
            app.CloseDialogs();
        });
    }
}
