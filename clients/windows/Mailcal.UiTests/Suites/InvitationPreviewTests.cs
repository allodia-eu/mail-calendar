// "Your calendar that day" opens expanded whenever the calendar was read, on every platform and
// whatever the conflict count (docs/invitations.md, "The preview opens expanded").
//
// WHY TWO INVITATIONS. The rule it replaced was "open only when the count is non-zero", so a test
// that sees only a conflicted day passes under both rules. Telling them apart needs a day with no
// overlap, which is why seed.sh seeds a second invitation on the weekend the living week leaves
// empty; the conflicted day stops a future "open only when it is empty" from looking right.
//
// Nothing below matches a localised string: the expander is found by its class, and the day is read
// as event titles, which come from the seed. Hour labels are told apart by shape, so a 12-hour clock
// does not read as an event called "9 AM".
//
// The preview is withheld until the calendar has been read, which is correct: zero and "we have not
// looked" are different facts (docs/calendar.md §4). The launch waits for the calendar's refresh.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
[Trait("Dataset", "harness")]
public sealed partial class InvitationPreviewTests(HarnessApp harness)
{
    private const string FreeDayInvite = "Weekend walk"; // on the empty weekend day, 11:00 to 12:00
    private const int FreeDayEndHour = 12;
    private const string ConflictedInvite = "Quarterly planning"; // Monday, inside the review and triage overlap

    [GeneratedRegex(@"^\s*(\d{1,2})(?::(\d{2}))?\s*(?:([AaPp])\.?[Mm]\.?)?\s*$")]
    private static partial Regex HourLabel();

    /// <summary>Opens <paramref name="title"/> and returns its invitation card's preview expander.</summary>
    private static Element Preview(App app, string title)
    {
        app.OpenMessage(title);
        IReadOnlyList<Element> found = [];
        App.Eventually(() =>
            (found = app.Agent.FindAll(new Filter { Window = app.Mailbox.Hwnd, OnScreen = true })
                .Where(e => e.Class == "Microsoft.UI.Xaml.Controls.Expander").ToList()).Count > 0, App.Patience);
        return found.Count switch
        {
            1 => found[0],
            0 => throw new InvalidOperationException(
                $"'{title}' showed no invitation preview. In the order worth checking: the harness store is stale (scripts/dev/harness.sh reset); the calendar was never read, so the preview is correctly withheld; or the card was not built (docs/invitations.md)"),
            _ => throw new InvalidOperationException($"expected one expander in the reading pane, found {found.Count}"),
        };
    }

    /// <summary>The expander's texts, split into hour labels (hour to the y they are drawn at) and
    /// event titles with their y.</summary>
    private static (Dictionary<int, double> Hours, List<(string Title, double Top)> Events) Contents(App app, Element expander)
    {
        var hours = new Dictionary<int, double>();
        var events = new List<(string, double)>();
        foreach (var text in app.Agent.FindAll(new Filter { Type = "Text", Under = expander }))
        {
            if (text.Name.Length == 0 || text.Name == expander.Name)
            {
                continue;
            }
            var hour = HourLabel().Match(text.Name);
            if (!hour.Success)
            {
                events.Add((text.Name, text.Y));
                continue;
            }
            var h = int.Parse(hour.Groups[1].Value, System.Globalization.CultureInfo.InvariantCulture);
            var meridiem = hour.Groups[3].Value.ToLowerInvariant();
            h = meridiem == "p" && h < 12 ? h + 12 : meridiem == "a" && h == 12 ? 0 : h;
            hours[h] = text.Y;
        }
        return (hours, events);
    }

    private static string State(App app, Element expander) => app.Agent.Read(expander).Expanded ?? "(no expand state)";

    [HarnessFact]
    public void ThePreviewOpensExpandedOnADayWithNothingElseOnIt() => harness.Case(nameof(ThePreviewOpensExpandedOnADayWithNothingElseOnIt), app =>
    {
        // The discriminating case: under the old rule this was collapsed, the count being zero.
        var expander = Preview(app, FreeDayInvite);
        Assert.True(State(app, expander) == "Expanded", $"a free day is when the picture settles it fastest, so the grid opens unasked; it is {State(app, expander)}");

        // A fixture guard, not a product assertion: the case only tells the rules apart while this
        // day's count is zero. The meeting's own tentative block is drawn and is no clash. A title is
        // drawn at the top of its block, so its y is the event's start, and one starting at or after
        // the meeting ends cannot overlap it; anything earlier is kept, over-reporting being the safe
        // side. With no label for the end hour, every drawn event counts.
        var (hours, events) = Contents(app, expander);
        var others = events.Where(e => e.Title != FreeDayInvite);
        var clashing = (hours.TryGetValue(FreeDayEndHour, out var cutoff) ? others.Where(e => e.Top < cutoff - 1) : others).ToList();
        Assert.True(clashing.Count == 0,
            $"nothing may overlap the seeded weekend meeting, or this case stops telling the rules apart; found: {string.Join(", ", clashing.Select(c => c.Title))}. A fixture problem, not a regression: scripts/dev/harness.sh reset");
    });

    [HarnessFact]
    public void ThePreviewOpensExpandedOnADayThatHasClashesToo() => harness.Case(nameof(ThePreviewOpensExpandedOnADayThatHasClashesToo), app =>
    {
        var expander = Preview(app, ConflictedInvite);
        Assert.True(State(app, expander) == "Expanded", $"the conflicted day opens too, one rule rather than two opposite special cases; it is {State(app, expander)}");
        // A fixture guard: if the Monday invitation stopped landing in the overlap, the two cases
        // would be the same test.
        Assert.True(Contents(app, expander).Events.Count > 0, "the Monday invitation is seeded to overlap the living week, so its preview draws something");
    });
}
