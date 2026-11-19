// Settings → Composing → Default send account, as the core offers it (SendAccountSetting). The
// showcase holds two accounts used for mail, so the row is a choice over both. What the core decides
// is pinned in its own tests; what this is for is the client wiring: a picker that opens on an item
// other than the core's, a choice that is not stored, or one that a rebuild of the page drops.
//
// Nothing below names an address: the items are read off the picker, and the choice is whichever one
// it does not already hold.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed class SettingsSendAccountTests(ShowcaseApp showcase)
{
    private static Element Picker(App app, SetupFlow settings) =>
        app.Agent.Wait(new Filter { Id = "SendAccountPicker", Type = "ComboBox", Under = settings.OpenCategory("composing") }, TimeSpan.FromSeconds(10))
        ?? throw new InvalidOperationException("Settings → Composing draws no send-account picker over the showcase's two accounts");

    private static string? Chosen(App app, SetupFlow settings) => app.Agent.Read(Picker(app, settings)).Selection;

    /// <summary>The open list's items. A closed picker exposes none; an open one lists them under
    /// itself as well, and the ones drawn are those that appeared elsewhere when it opened.</summary>
    private static List<Element> OpenItems(App app, Element picker)
    {
        var before = app.Agent.FindAll(new Filter { Type = "ListItem" }).Select(e => e.Handle).ToHashSet();
        app.Agent.Expand(picker);
        List<Element> items = [];
        Assert.True(App.Eventually(() =>
        {
            var own = app.Agent.FindAll(new Filter { Type = "ListItem", Under = picker }).Select(e => e.Handle).ToHashSet();
            items = app.Agent.FindAll(new Filter { Type = "ListItem" })
                .Where(i => !before.Contains(i.Handle) && !own.Contains(i.Handle))
                .ToList();
            return items.Count > 0;
        }, TimeSpan.FromSeconds(5)), "the picker opens a list");
        return items;
    }

    /// <summary>The addresses the picker offers, in its order.</summary>
    private static List<string> Offered(App app, Element picker)
    {
        var offered = OpenItems(app, picker).Select(e => e.Name).ToList();
        app.Agent.Collapse(picker);
        return offered;
    }

    /// <summary>Presses the item named <paramref name="address"/> in the open list, as a person does:
    /// its items take no selection through their peers.</summary>
    private static void Choose(App app, Element picker, string address)
    {
        var item = OpenItems(app, picker).FirstOrDefault(i => i.Name == address)
            ?? throw new InvalidOperationException($"the open picker offers no {address}");
        Input.Click(app.Steady(item));
    }

    [OfflineFact]
    public void TheChoiceIsStoredAndSurvivesLeavingThePage()
    {
        string? original = null;
        showcase.Case(nameof(TheChoiceIsStoredAndSurvivesLeavingThePage), app =>
        {
            var settings = new SetupFlow(app);
            var offered = Offered(app, Picker(app, settings));
            Assert.True(offered.Count == 2, $"the showcase's two accounts used for mail are offered; the picker holds {string.Join(" | ", offered)}");
            original = Chosen(app, settings);
            Assert.True(original == offered[0], $"with nothing chosen the default is the first account used for mail; the picker opened on '{original}'");

            var other = offered[1];
            Choose(app, Picker(app, settings), other);
            Assert.True(App.Eventually(() => Chosen(app, settings) == other, TimeSpan.FromSeconds(5)), $"the picker took {other}");

            // The panel is rebuilt per category, so a picker seeded from a default rather than the
            // core's answer looks right until the person comes back to it.
            settings.OpenCategory("privacy");
            Assert.Equal(other, Chosen(app, settings));
        }, app =>
        {
            // Leave the preference as the suite found it.
            var settings = new SetupFlow(app);
            if (original is not null && Chosen(app, settings) != original)
            {
                Choose(app, Picker(app, settings), original);
                App.Eventually(() => Chosen(app, settings) == original, TimeSpan.FromSeconds(5));
            }
            app.CloseDialogs();
        });
    }
}
