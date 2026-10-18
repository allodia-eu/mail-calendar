// The reading pane's action row and the overflow menu at the end of it (docs/reading-actions.md):
// the overflow sits last, it is the same control as the buttons beside it at both of the row's
// widths, and the export behind it writes the message that arrived.
//
// THE REGRESSION. The overflow is the one control in the row carrying a glyph and no label, and the
// default button style centres its content rather than stretching it, so it came up at the glyph's
// height while its labelled neighbours stood at their title's line box: 54px against 64px on a 200%
// display. The selection bar guards the same defect, with the same fix (VerticalAlignment="Stretch").
//
// WHY BOTH WIDTHS. The row collapses to icon-only on a narrow pane, and there every button is the
// glyph's height, so the overflow matches by accident and a single-width assertion passes over the
// defect. The labelled width is reached by collapsing the navigation pane, and a display too narrow
// to reach it fails rather than skipping: the suite would otherwise measure the icon-only row twice.
//
// WHY THE HARNESS. The export asserts on written bytes, and the showcase engine performs no real
// mail read. 01-plain.eml from the seeder is the comparison, line endings normalised because the
// server normalises them on delivery.

using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class ReadingActionRowTests(HarnessApp harness)
{
    private const string Plain = "Harness baseline message";

    // In the order the markup lays them out, named rather than swept off the container, so a button
    // that stops rendering fails here instead of leaving the comparison to its siblings.
    private static readonly string[] RowButtons =
    [
        Filter.ById("ReadingReply").Id!, Filter.ById("ReadingReplyAll").Id!, Filter.ById("ReadingForward").Id!,
        Filter.ById("ReadingArchive").Id!, Filter.ById("ReadingDelete").Id!, Filter.ById("ReadingOverflow").Id!,
    ];

    // Physical pixels. Slack for layout rounding, not for a design difference: the labelled buttons
    // agree with each other exactly, and the defect was 10px.
    private const double Tolerance = 1.5;

    private static readonly string Seed = Path.Combine(App.Repo, "docker", "stalwart", "seed", "mail", "01-plain.eml");

    private void Case(string name, Action<App> body) => harness.Case(name, body, app => SetLabelled(app, false));

    /// <summary>The plain seeded message, open, and each action-row button as drawn.</summary>
    private static Dictionary<string, Element> Buttons(App app)
    {
        app.OpenMessage(Plain);
        app.Agent.Wait(Filter.ById("ReadingOverflow") with { OnScreen = true }, App.Patience);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300));
        return RowButtons.ToDictionary(id => id, id =>
            app.Agent.Find(new Filter { Id = id, Window = app.Mailbox.Hwnd }) is { Rendered: true } button
                ? button
                : throw new InvalidOperationException($"the action row's #{id} is not drawn"));
    }

    /// <summary>Whether the row draws its labels. Read off a label rather than a button width, which
    /// moves with the locale; a collapsed label leaves the tree.</summary>
    private static bool Labelled(App app) => app.Agent.Find(Filter.ById("ReplyLabel")) is { Rendered: true };

    /// <summary>
    /// Puts the row in its labelled or its icon-only variant by the navigation-pane toggle, the one
    /// lever there is: the list and reading divider is a pointer gesture. Which variant a pane width
    /// gives depends on the display and the locale, so this reports, and flips back when it cannot
    /// deliver, since a collapsed pane would follow every later suite.
    /// </summary>
    private static bool SetLabelled(App app, bool labelled)
    {
        if (Labelled(app) == labelled)
        {
            return true;
        }
        var toggle = app.Agent.Find(Filter.ById("PART_PaneToggleButton", "Button"))
            ?? throw new InvalidOperationException("no navigation-pane toggle in the caption, so neither of the row's widths can be chosen");
        app.Agent.Invoke(toggle);
        if (App.Eventually(() => Labelled(app) == labelled, TimeSpan.FromSeconds(3)))
        {
            return true;
        }
        app.Agent.Invoke(toggle);
        app.Agent.Settle(quiet: TimeSpan.FromMilliseconds(300), floor: TimeSpan.FromMilliseconds(500));
        return false;
    }

    /// <summary>One height, one top edge, one bottom edge.</summary>
    private static void AssertOneRow(Dictionary<string, Element> buttons, string width)
    {
        static double Spread(IEnumerable<double> values) => values.Max() - values.Min();
        var edges = string.Join(" | ", buttons.Select(b => $"{b.Key}={b.Value.Y:0.#}..{b.Value.Y + b.Value.Height:0.#}"));
        Assert.True(Spread(buttons.Values.Select(b => b.Height)) <= Tolerance,
            $"at the {width} width the row's controls are one height: a reader tells the overflow from its neighbours by its glyph alone. Edges: {edges}");
        // Equal heights alone would also be met by buttons stepping down the pane.
        Assert.True(Spread(buttons.Values.Select(b => b.Y)) <= Tolerance, $"at the {width} width the row shares a top edge. Edges: {edges}");
        Assert.True(Spread(buttons.Values.Select(b => b.Y + b.Height)) <= Tolerance, $"at the {width} width the row shares a bottom edge. Edges: {edges}");
    }

    [HarnessFact]
    public void TheOverflowIsLastOfAllAfterBothMailboxActions() => Case(nameof(TheOverflowIsLastOfAllAfterBothMailboxActions), app =>
    {
        var buttons = Buttons(app);
        var overflow = buttons["ReadingOverflow"].X;
        foreach (var (id, button) in buttons.Where(b => b.Key != "ReadingOverflow"))
        {
            Assert.True(button.X < overflow,
                $"#{id} is drawn at {button.X} and the overflow at {overflow}; the overflow ends the row on every platform, which is what makes it findable without a label");
        }
    });

    [HarnessFact]
    public void TheRowIsOneRowAtTheIconOnlyWidth() => Case(nameof(TheRowIsOneRowAtTheIconOnlyWidth), app =>
    {
        Buttons(app);
        Assert.True(SetLabelled(app, false),
            "the reading pane never reached its icon-only width, even with the navigation pane open, so this would measure the labelled row and prove nothing about the narrow one");
        AssertOneRow(Buttons(app), "icon-only");
    });

    [HarnessFact]
    public void TheRowIsOneRowAtTheLabelledWidthTooWhichIsWhereACentredGlyphShows() => Case(nameof(TheRowIsOneRowAtTheLabelledWidthTooWhichIsWhereACentredGlyphShows), app =>
    {
        Buttons(app);
        Assert.True(SetLabelled(app, true),
            "the reading pane never reached its labelled width, even with the navigation pane collapsed, so this would measure the icon-only row again. Widen the window, or run it on a larger display");
        AssertOneRow(Buttons(app), "labelled");
    });

    [HarnessFact]
    public void TheMenuBehindItOffersTheExportUnderASpokenName() => Case(nameof(TheMenuBehindItOffersTheExportUnderASpokenName), app =>
    {
        var overflow = Buttons(app)["ReadingOverflow"];
        // A spoken name, not which one: the catalog string is localised. A WinUI button derives no
        // name from an icon-only content, and the fallback reads as the id to a screen reader.
        Assert.True(overflow.Name.Length > 0 && overflow.Name != "ReadingOverflow",
            $"the overflow carries no label at any width, so its accessible name is all a screen reader has; it announced '{overflow.Name}'");
        app.Agent.Expand(overflow);
        try
        {
            Assert.True(app.Agent.Wait(Filter.ById("ReadingExportEml"), TimeSpan.FromSeconds(10)) is not null, "the overflow offers the .eml export");
        }
        finally
        {
            app.Agent.Collapse(overflow);
        }
    });

    [HarnessFact]
    public void ExportingWritesTheBytesThatWereDeliveredAtThePathThePickerWasGiven()
    {
        var destination = Path.Combine(Path.GetTempPath(), $"mailcal-uitest-export-{Guid.NewGuid():N}.eml");
        harness.Case(nameof(ExportingWritesTheBytesThatWereDeliveredAtThePathThePickerWasGiven), app =>
        {
            var overflow = Buttons(app)["ReadingOverflow"];
            app.Agent.Expand(overflow);
            app.Agent.Invoke(app.Agent.Wait(Filter.ById("ReadingExportEml"), TimeSpan.FromSeconds(10))
                ?? throw new InvalidOperationException("the overflow offered no export item"));
            var suggested = SaveDialogs.Save(app.ProcessId, destination);
            // The name the picker opened on is the only evidence of which string the client handed
            // the core: the wrong field compiles, saves the right bytes and files them under a name
            // nobody can find again.
            Assert.Equal($"{Plain}.eml", suggested);
            var written = File.ReadAllText(destination).Replace("\r\n", "\n");
            var delivered = File.ReadAllText(Seed).Replace("\r\n", "\n");
            Assert.True(delivered == written,
                "the exported file is the message that arrived, not a rebuild of the reading view (docs/reading-actions.md)");
        }, app =>
        {
            SaveDialogs.CloseAll(app.ProcessId);
            File.Delete(destination);
        });
    }
}
