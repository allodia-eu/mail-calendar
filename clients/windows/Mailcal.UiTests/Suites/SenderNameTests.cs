// The name an account sends its mail under, on both surfaces that show it: the field on the account's
// page in Settings → Accounts, and the composer's From (docs/sending.md, "The sender's name"). The
// field is built in code-behind and commits on losing focus, so a page that wires it to nothing, or
// to the wrong account, draws exactly like one that works; the From's label is a display path, and
// one pointed at the wrong property misnames every account.
//
// The showcase: two accounts, each seeded with the name it sends under, and no provider holding a
// name of its own, so every page offers the field. The read-only card, where an organisation holds
// the name, needs a real Microsoft account; the step that asks for a name after a connect needs a
// real connect, which a shared launch may not make, and staging the prompt would fake the surface
// rather than the input. Both are driven by hand.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(ShowcaseCollection.Name)]
[Trait("Dataset", "showcase")]
public sealed partial class SenderNameTests(ShowcaseApp showcase)
{
    private static readonly string Heading = Catalog.English("settings_sender_name_heading");
    private static readonly string Depth = Catalog.English("settings_sync_depth_label");

    [GeneratedRegex(@"^.+ <[^<>@]+@[^<>]+>$")]
    private static partial Regex NameAndAddress();

    private void Case(string name, Action<App, SetupFlow> body) =>
        showcase.Case(name, app => body(app, new SetupFlow(app)), app =>
        {
            app.CloseComposer();
            app.CloseDialogs();
        });

    /// <summary>The <paramref name="index"/>th account's page, whichever address it has.</summary>
    private static Element Page(SetupFlow settings, int index)
    {
        var rows = settings.AccountRows();
        return index < rows.Count ? settings.OpenAccount(rows[index].Name, rows.Take(index).Count(r => r.Name == rows[index].Name))
            : throw new InvalidOperationException($"Settings → Accounts lists {rows.Count} account(s), not {index + 1}");
    }

    /// <summary>The name field, typed to Edit: the heading above it reads the same.</summary>
    private static List<Element> Boxes(App app, Element dialog) =>
        app.Agent.FindAll(new Filter { Type = "Edit", Name = Heading, Under = dialog }).ToList();

    private static string Value(App app, Element box) => app.Agent.Read(box).Value ?? "";

    /// <summary>
    /// Types into the field and moves focus off it, which is what commits the edit: a value set on a
    /// box that never had the keyboard raises no LostFocus and sends nothing. The keyboard goes to a
    /// picker on the same page, somewhere real.
    /// </summary>
    private static void SetName(App app, Element dialog, string text)
    {
        var box = Boxes(app, dialog)[0];
        app.Agent.Focus(box);
        app.Agent.SetValue(box, text);
        app.Agent.Focus(app.Agent.Find(new Filter { Type = "ComboBox", Name = Depth, Under = dialog })
            ?? throw new InvalidOperationException($"the account page has no '{Depth}' picker to move focus to"));
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
    }

    [OfflineFact]
    public void EveryAccountPageOffersTheFieldBecauseNoProviderHereHoldsTheName() => Case(nameof(EveryAccountPageOffersTheFieldBecauseNoProviderHereHoldsTheName), (app, settings) =>
    {
        foreach (var index in new[] { 0, 1 })
        {
            var dialog = Page(settings, index);
            var boxes = Boxes(app, dialog);
            // Rule 2: offered wherever the name is the account holder's.
            Assert.True(boxes.Count == 1, $"account {index}'s page offers one name field, found {boxes.Count}");
            Assert.True(boxes[0].Enabled, "a field drawn disabled reads as a bug; where the name cannot change the page states it and draws no field");
            Assert.True(Value(app, boxes[0]).Trim().Length > 0, "the showcase seeds each account with a name; an empty field means the seed never reached the core");
            Assert.True(app.Agent.Find(new Filter { Type = "Text", Name = Catalog.English("settings_sender_name_managed"), Under = dialog }) is null,
                "the organisation note belongs only where sender_name_editable is false (rule 2)");
        }
    });

    [OfflineFact]
    public void TheNameSitsAboveWhatTheAccountStores() => Case(nameof(TheNameSitsAboveWhatTheAccountStores), (app, settings) =>
    {
        // The one control on the page whose effect a stranger sees comes first (docs/settings.md).
        // Scrolled until both are drawn, since what fits depends on the window and the scale.
        var dialog = Page(settings, 0);
        foreach (var percent in new[] { 0, 25, 50, 75, 100 })
        {
            app.Agent.ScrollTo(dialog, percent);
            app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
            var name = Boxes(app, dialog).FirstOrDefault();
            var depth = app.Agent.Find(new Filter { Type = "ComboBox", Name = Depth, Under = dialog });
            if (name is null || depth is null)
            {
                throw new InvalidOperationException("the account page has no name field or no download picker at all");
            }
            if (name.Rendered && depth.Rendered)
            {
                Assert.True(name.Y < depth.Y, $"the name field ({name.Y}) comes above the download picker ({depth.Y})");
                return;
            }
        }
        throw new InvalidOperationException("no scroll position draws both rows, so their order cannot be measured");
    });

    [OfflineFact]
    public void ATypedNameReachesTheCoreAndThePageReadsItBackFromThere()
    {
        string? was = null;
        Case(nameof(ATypedNameReachesTheCoreAndThePageReadsItBackFromThere), (app, settings) =>
        {
            var other = Value(app, Boxes(app, Page(settings, 1))[0]);
            var dialog = Page(settings, 0);
            was = Value(app, Boxes(app, dialog)[0]);
            SetName(app, dialog, "Ada Lovelace");
            // The page is built from the core's settings each time it is shown, so this is what the
            // core holds; the old name means the edit never left the client.
            Assert.Equal("Ada Lovelace", Value(app, Boxes(app, Page(settings, 0))[0]));
            // Per account: the other moving too means the setter was handed the wrong id.
            Assert.Equal(other, Value(app, Boxes(app, Page(settings, 1))[0]));
            // Put back: the launch is shared.
            SetName(app, Page(settings, 0), was);
        });
    }

    [OfflineFact]
    public void TheComposersFromReadsAsTheRecipientWillReadItNameAndAddress() => Case(nameof(TheComposersFromReadsAsTheRecipientWillReadItNameAndAddress), (app, _) =>
    {
        // Rule 7: the From shows the whole of what arrives, `Name <address>`, as the recipient's
        // client shows it.
        new DraftsFlow(app).OpenNewComposer();
        var from = app.Agent.Find(Filter.ById("FromBox", "ComboBox")) ?? throw new InvalidOperationException("the composer has no From picker (#FromBox)");
        // Open, because a closed picker's items are drawn by nothing yet, so its selection can only
        // be told by the bound object rather than by the label it is drawn with.
        app.Agent.Expand(from);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        var selected = app.Agent.Read(from).Selection ?? "";
        Assert.True(NameAndAddress().IsMatch(selected), $"the From label is `Name <address>`; it reads: {selected}");
        // Every item, not only the selected one: a label from the right property in one place and
        // the wrong one in the other misnames every other account.
        var items = app.Agent.FindAll(new Filter { Type = "ListItem", Under = from }).Select(i => i.Name).Distinct().ToList();
        app.Agent.Collapse(from);
        Assert.True(items.Count >= 2, $"the showcase holds two accounts, so the picker offers both; it offers {string.Join(" | ", items)}");
        foreach (var item in items)
        {
            Assert.True(NameAndAddress().IsMatch(item), $"every account in the picker carries its name; this one reads: {item}");
        }
    });

    [OfflineFact]
    public void ClearingTheFieldIsAnAnswerNotANoOpTheAccountGoesBackToABareAddress()
    {
        string? was = null;
        Case(nameof(ClearingTheFieldIsAnAnswerNotANoOpTheAccountGoesBackToABareAddress), (app, settings) =>
        {
            var dialog = Page(settings, 0);
            was = Value(app, Boxes(app, dialog)[0]);
            Assert.True(was.Length > 0, "this needs a name to delete, and the seeded page carries one");
            SetName(app, dialog, "");
            // Rule 3: an empty name is a real answer; treated as nothing to do, the old name stays on
            // the wire after the user deleted it.
            Assert.Equal("", Value(app, Boxes(app, Page(settings, 0))[0]));
            SetName(app, Page(settings, 0), was);
        });
    }
}
