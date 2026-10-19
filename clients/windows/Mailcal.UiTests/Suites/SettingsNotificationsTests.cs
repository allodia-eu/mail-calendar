// Settings → Notifications (docs/settings.md slot 7): whether new mail raises a desktop notification.
// What a pass says is pinned in Mailcal.Tests (NewMailNoticesTests); what this is for is the client
// wiring, the category missing from the list, a panel never reached, or a switch that opens on a
// default rather than on the stored choice. The toast itself is the shell's; NotificationClickTests
// reads it back.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SettingsNotificationsTests(ShowcaseApp showcase)
{
    private void Case(string name, Action<App, SetupFlow> body) =>
        showcase.Case(name, app => body(app, new SetupFlow(app)), app => app.CloseDialogs());

    /// <summary>The new-mail switch, by id: a toggle switch takes its name from its header, and the
    /// heading above it reads the same.</summary>
    private static Element Toggle(App app, SetupFlow settings) =>
        app.Agent.Find(new Filter { Id = "NotificationsToggle", Under = settings.OpenCategory("notifications") })
        ?? throw new InvalidOperationException("the Notifications category draws no new-mail switch (docs/settings.md slot 7)");

    [OfflineFact]
    public void SettingsOffersNotificationsBetweenSignaturesAndPrivacy() => Case(nameof(SettingsOffersNotificationsBetweenSignaturesAndPrivacy), (_, settings) =>
    {
        // The taxonomy's own neighbours rather than an index, which moves whenever a category is
        // added above it.
        var categories = settings.Categories();
        var at = categories.IndexOf("notifications");
        Assert.True(at > 0 && at < categories.Count - 1 && categories[at - 1] == "signatures" && categories[at + 1] == "privacy",
            $"Notifications sits between Signatures and Privacy on every platform (docs/settings.md); the order is {string.Join(" | ", categories)}");
    });

    [OfflineFact]
    public void TheCategoryDrawsASwitchTheUserCanActuallyMove() => Case(nameof(TheCategoryDrawsASwitchTheUserCanActuallyMove), (app, settings) =>
    {
        var toggle = Toggle(app, settings);
        Assert.True(toggle.Enabled, "a switch that cannot be pressed is not a setting");
        Assert.True(toggle.Rendered, "the switch is on screen, not merely in the tree");
    });

    [OfflineFact]
    public void TheChoiceSurvivesLeavingTheCategoryAndComingBack()
    {
        string? original = null;
        showcase.Case(nameof(TheChoiceSurvivesLeavingTheCategoryAndComingBack), app =>
        {
            // The panel is rebuilt per category, so a switch seeded from a default rather than the
            // store looks right until the user comes back to it.
            var settings = new SetupFlow(app);
            original = app.Agent.Read(Toggle(app, settings)).Toggle;
            var flipped = app.Agent.Toggle(Toggle(app, settings));
            settings.OpenCategory("privacy");
            Assert.Equal(flipped, app.Agent.Read(Toggle(app, settings)).Toggle);
        }, app =>
        {
            // Leave the preference as the suite found it.
            var settings = new SetupFlow(app);
            if (original is not null && app.Agent.Read(Toggle(app, settings)).Toggle != original)
            {
                app.Agent.Toggle(Toggle(app, settings));
            }
            app.CloseDialogs();
        });
    }
}
