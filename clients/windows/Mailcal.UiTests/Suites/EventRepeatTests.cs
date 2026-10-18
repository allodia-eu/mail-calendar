// What an event's Repeat row says on the detail, and what its controls open on in the editor
// (docs/calendar.md §10).
//
// WHY HERE. Mailcal.Tests pins which frame a rule reaches and what a save sends, but mapping a frame
// to its catalog string and seeding the editor's controls are both WinUI: a frame wired to the wrong
// string, or an interval box opening on 1 for a fortnightly series, passes every headless test.
//
// WHY THE HARNESS. The showcase seed carries no recurrence; these rules arrive over JMAP.
//
// NOTHING BELOW MATCHES A LOCALISED STRING, because a harness run comes up in the developer's own
// language. The handles are the seeded event titles, the EventRepeatValue id on the row's value,
// the sentences compared to each other and to the digits in them, and what the editor draws at all.
//
// THE SEEDED RULES (docker/stalwart/seed-calendar-week.sh, the `repeating` block):
//   Team sync        FREQ=WEEKLY;BYDAY=MO,WE
//   Sprint planning  FREQ=WEEKLY;INTERVAL=2
//   Board meeting    FREQ=MONTHLY;BYDAY=1WE
//   Onboarding       FREQ=DAILY;COUNT=10
//   Lunch            (no rule)
// No fixture in the living week has an UNTIL, so a date-bounded rule is proven headless only.

using System.Text.RegularExpressions;
using Allodia.Mailcal.UiTests.Driver;
using Xunit;

namespace Allodia.Mailcal.UiTests.Suites;

[Collection(HarnessCollection.Name)]
public sealed class EventRepeatTests(HarnessApp harness)
{
    private const string Weekly = "Team sync";
    private const string Fortnightly = "Sprint planning";
    private const string MonthlyNth = "Board meeting";
    private const string Counted = "Onboarding";
    private const string Plain = "Lunch";

    private void Case(string name, Action<App, CalendarFlow> body) =>
        harness.Case(name, app => body(app, new CalendarFlow(app)), app => app.CloseDialogs());

    /// <summary>What the Repeat row says for <paramref name="title"/>, found by id because its
    /// caption is localised. Leaves the dialog closed.</summary>
    private static string Sentence(App app, CalendarFlow flow, string title)
    {
        var dialog = flow.OpenDetail(title);
        var value = app.Agent.Find(new Filter { Id = "EventRepeatValue", Type = "Text", Under = dialog })
            ?? throw new InvalidOperationException($"'{title}' shows no repeat row at all; every event says how it repeats, the ones that do not included");
        app.CloseDialogs();
        return value.Name.Length > 0 ? value.Name : throw new InvalidOperationException($"'{title}' has an empty repeat row");
    }

    /// <summary>The choice a picker opens on, read off its selection: its own name is its header.</summary>
    private static string Choice(App app, CalendarFlow flow, string title, string id)
    {
        var dialog = flow.OpenEditor(title);
        var picker = app.Agent.Find(new Filter { Id = id, Under = dialog }) ?? throw new InvalidOperationException($"'{title}' shows no {id} control");
        var chosen = app.Agent.Read(picker).Selection;
        app.CloseDialogs();
        return string.IsNullOrEmpty(chosen) ? throw new InvalidOperationException($"{id} on '{title}' shows no choice, which states no rule at all") : chosen;
    }

    /// <summary>The number a spinner opens on, read off the edit inside it: the spinner's own name
    /// is its localised header.</summary>
    private static string Spinner(App app, CalendarFlow flow, string title, string id)
    {
        var dialog = flow.OpenEditor(title);
        var spinner = app.Agent.Find(new Filter { Id = id, Under = dialog }) ?? throw new InvalidOperationException($"'{title}' shows no {id} control");
        var box = app.Agent.Find(new Filter { Type = "Edit", Under = spinner })
            ?? throw new InvalidOperationException($"{id} on '{title}' has no editable field, so its number cannot be read or typed");
        var value = app.Agent.Read(box).Value ?? "";
        app.CloseDialogs();
        return value;
    }

    /// <summary>How many weekday buttons are ticked: a count, because the names are localised.</summary>
    private static int CheckedWeekdays(App app, CalendarFlow flow, string title)
    {
        var dialog = flow.OpenEditor(title);
        var days = app.Agent.FindAll(new Filter { Id = "EventRepeatWeekday", Under = dialog });
        if (days.Count == 0)
        {
            throw new InvalidOperationException($"'{title}' is a weekly rule and shows no weekday row");
        }
        var ticked = days.Count(day => app.Agent.Read(day).Toggle == "On");
        app.CloseDialogs();
        return ticked;
    }

    [HarnessFact]
    public void AFortnightlyRuleDoesNotReadAsTheWeeklyOneAndSaysHowManyWeeksItSkips() => Case(nameof(AFortnightlyRuleDoesNotReadAsTheWeeklyOneAndSaysHowManyWeeksItSkips), (app, flow) =>
    {
        // A frequency word alone cannot tell every week from every second week.
        var weekly = Sentence(app, flow, Weekly);
        var fortnightly = Sentence(app, flow, Fortnightly);
        Assert.True(weekly != fortnightly, $"a rule that repeats every week and one that skips every other must not read alike (both say '{weekly}')");
        Assert.True(fortnightly.Contains('2'), $"a fortnightly rule states its interval; '{fortnightly}' does not contain the number of weeks it skips");
    });

    [HarnessFact]
    public void ARuleThatStopsAfterAFixedNumberOfOccurrencesSaysTheNumber() => Case(nameof(ARuleThatStopsAfterAFixedNumberOfOccurrencesSaysTheNumber), (app, flow) =>
    {
        var counted = Sentence(app, flow, Counted);
        Assert.True(counted.Contains("10", StringComparison.Ordinal), $"a rule seeded COUNT=10 must say how many times it runs; '{counted}' does not contain 10");
    });

    [HarnessFact]
    public void AWeeklyRuleOnTwoWeekdaysNamesBothOfThem() => Case(nameof(AWeeklyRuleOnTwoWeekdaysNamesBothOfThem), (app, flow) =>
    {
        // The join is the client's, ", " in every language, so it is a language-free handle.
        var weekly = Sentence(app, flow, Weekly);
        Assert.True(Regex.IsMatch(weekly, @",\s"), $"a rule seeded BYDAY=MO,WE names both weekdays; '{weekly}' names at most one");
    });

    [HarnessFact]
    public void FourDifferentRulesSayFourDifferentThings() => Case(nameof(FourDifferentRulesSayFourDifferentThings), (app, flow) =>
    {
        // The guard for the whole feature, and for a frame wired to the wrong string, which tends to
        // collapse two sentences into one.
        var sentences = new[] { Weekly, Fortnightly, MonthlyNth, Counted }.ToDictionary(t => t, t => Sentence(app, flow, t));
        Assert.True(sentences.Values.Distinct().Count() == 4,
            "four rules of different shapes must read differently; got: " + string.Join(", ", sentences.Select(s => $"{s.Key}='{s.Value}'")));
    });

    [HarnessFact]
    public void AnEventWithNoRuleSaysItDoesNotRepeatAndSaysItDifferently() => Case(nameof(AnEventWithNoRuleSaysItDoesNotRepeatAndSaysItDifferently), (app, flow) =>
    {
        // The control: without it, every event reading the same word would still pass.
        var plain = Sentence(app, flow, Plain);
        foreach (var title in new[] { Weekly, Fortnightly, MonthlyNth, Counted })
        {
            var repeating = Sentence(app, flow, title);
            Assert.True(plain != repeating, $"'{title}' repeats and '{Plain}' does not, so they cannot read alike (both say '{plain}')");
        }
    });

    [HarnessFact]
    public void TheEditorOpensTheRuleAsControlsNotAsTheSentenceTheDetailShows() => Case(nameof(TheEditorOpensTheRuleAsControlsNotAsTheSentenceTheDetailShows), (app, flow) =>
    {
        // The detail reads a rule and the editor sets one: a reader who can only be told "every 2
        // weeks" has no way to make it every third.
        var dialog = flow.OpenEditor(Fortnightly);
        Assert.True(app.Agent.Find(new Filter { Id = "EventRepeatFrequency", Under = dialog }) is not null,
            "the editor offers no frequency control, so the rule cannot be changed at all");
        Assert.True(app.Agent.Find(new Filter { Id = "EventRepeatValue", Type = "Text", Under = dialog }) is null,
            "the editor states the rule as a read-only sentence, which is the surface it replaced");
    });

    [HarnessFact]
    public void TheFrequencyOpensOnTheRuleTheEventHasNotOnTheFirstChoice() => Case(nameof(TheFrequencyOpensOnTheRuleTheEventHasNotOnTheFirstChoice), (app, flow) =>
    {
        // A picker opening on its first choice ("Does not repeat") whatever the event does would
        // seed every control below it from nothing, and a save would stop the series.
        var weekly = Choice(app, flow, Fortnightly, "EventRepeatFrequency");
        var monthly = Choice(app, flow, MonthlyNth, "EventRepeatFrequency");
        var none = Choice(app, flow, Plain, "EventRepeatFrequency");
        Assert.True(weekly != monthly, $"a weekly and a monthly rule cannot open on the same frequency (both say '{weekly}')");
        Assert.True(weekly != none, $"a repeating event cannot open on the choice an event with no rule opens on (both say '{none}')");
        Assert.True(monthly != none, $"a monthly rule cannot open on the choice an event with no rule opens on (both say '{none}')");
    });

    [HarnessFact]
    public void AnEventWithNoRuleOffersTheFrequencyAloneAndNothingToConfigure() => Case(nameof(AnEventWithNoRuleOffersTheFrequencyAloneAndNothingToConfigure), (app, flow) =>
    {
        var dialog = flow.OpenEditor(Plain);
        Assert.True(app.Agent.Find(new Filter { Id = "EventRepeatInterval", Under = dialog }) is null,
            "an event that does not repeat offers an interval, setting a period on a rule that does not exist");
        Assert.True(app.Agent.Find(new Filter { Id = "EventRepeatEnds", Under = dialog }) is null,
            "an event that does not repeat offers an end condition, ending a rule that does not exist");
    });

    [HarnessFact]
    public void TheIntervalBoxOpensOnTheNumberOfPeriodsTheRuleSkips() => Case(nameof(TheIntervalBoxOpensOnTheNumberOfPeriodsTheRuleSkips), (app, flow) =>
    {
        // A fortnightly rule seeded into a box reading 1 is a form proposing to make it weekly.
        Assert.Equal("2", Spinner(app, flow, Fortnightly, "EventRepeatInterval"));
        Assert.True(Spinner(app, flow, Weekly, "EventRepeatInterval") == "1", "a rule with no INTERVAL repeats every period, so its box opens on 1");
    });

    [HarnessFact]
    public void ARuleThatStopsAfterAFixedNumberOpensOnThatNumber() => Case(nameof(ARuleThatStopsAfterAFixedNumberOpensOnThatNumber), (app, flow) =>
    {
        // The count box exists only while Ends is on the after-N choice, so its presence is half of
        // what is asserted.
        var dialog = flow.OpenEditor(Counted);
        Assert.True(app.Agent.Find(new Filter { Id = "EventRepeatEndCount", Under = dialog }) is not null,
            "a rule seeded COUNT=10 opens with no count control, so its end is not on screen at all");
        app.CloseDialogs();
        Assert.True(Spinner(app, flow, Counted, "EventRepeatEndCount") == "10", "a rule seeded COUNT=10 opens on 10, not on a default the control invented");
    });

    [HarnessFact]
    public void TheWeekdayRowBelongsToAWeeklyRuleAndToNoOther() => Case(nameof(TheWeekdayRowBelongsToAWeeklyRuleAndToNoOther), (app, flow) =>
    {
        var weekly = flow.OpenEditor(Weekly);
        Assert.True(app.Agent.FindAll(new Filter { Id = "EventRepeatWeekday", Under = weekly }).Count == 7, "a weekly rule offers the whole week, one button per day");
        app.CloseDialogs();
        var monthly = flow.OpenEditor(MonthlyNth);
        Assert.True(app.Agent.FindAll(new Filter { Id = "EventRepeatWeekday", Under = monthly }).Count == 0,
            "a monthly rule draws the weekday row, offering to set a day its frequency cannot use");
    });

    [HarnessFact]
    public void TheWeekdayRowOpensWithTheRulesOwnDaysTickedAndNeverWithNone() => Case(nameof(TheWeekdayRowOpensWithTheRulesOwnDaysTickedAndNeverWithNone), (app, flow) =>
    {
        // The count separates BYDAY=MO,WE from a single day, and a weekly rule naming no day is one
        // the core refuses: a save that silently does nothing.
        Assert.True(CheckedWeekdays(app, flow, Weekly) == 2, "the rule seeded BYDAY=MO,WE opens with exactly those two days ticked");
        Assert.True(CheckedWeekdays(app, flow, Fortnightly) == 1, "a weekly rule naming no day takes the event's own weekday, so exactly one day is ticked");
    });
}
