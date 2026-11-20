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

    /// <summary>The open list's items. An open picker lists its items under itself as well, and
    /// the ones drawn are the items elsewhere that carry those names and are drawn over the picker
    /// itself. The window holds other lists: the Settings navigation, and behind the dialog the
    /// folder pane, whose account rows carry the same addresses. A picker still opening lists
    /// nothing under itself, so both are read once it is open.</summary>
    private static List<Element> OpenItems(App app, Element picker)
    {
        app.Agent.Expand(picker);
        List<Element> items = [];
        Assert.True(App.Eventually(() =>
        {
            var listed = app.Agent.FindAll(new Filter { Type = "ListItem", Under = picker }).ToList();
            var options = listed.Select(e => e.Name).ToHashSet();
            var under = listed.Select(e => e.Handle).ToHashSet();
            items = app.Agent.FindAll(new Filter { Type = "ListItem" })
                .Where(e => !under.Contains(e.Handle) && options.Contains(e.Name) && e.Rendered && Over(picker, e))
                .ToList();
            return listed.Count > 0 && items.Count >= listed.Count;
        }, TimeSpan.FromSeconds(5)), "the picker opens a list");
        return items;
    }

    /// <summary>Whether <paramref name="item"/> is drawn in the picker's column, as its open list is.</summary>
    private static bool Over(Element picker, Element item) =>
        item.X < picker.X + picker.Width && picker.X < item.X + item.Width;

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
