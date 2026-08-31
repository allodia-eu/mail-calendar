// Which sentence a repeat rule gets, over the structured rule the core sends.
//
// Which sentence that is, is pinned in the core (repeat_summary_tests.rs), once for every client.
// What is pinned here is the half that is genuinely this client's: that a rule skipping periods
// reaches a different frame from one repeating every period, that the platform's own weekday and
// month names are used and indexed by the right number, and that the ordinal picks the form the
// weekday agrees with.
//
// It stops at the frame rather than the sentence because L10n.cs cannot be linked into this
// assembly, the same reason InvitationFormat stops where InvitationText begins. Mapping a frame to
// its catalog string is EventRepeatText (Dialogs/), and the rendered sentence is asserted on the
// running app by clients/windows/uitests. The Windows twin of Apple's EventRepeatTextTests and
// Android's EventRecurrenceTextTest.
using System.Globalization;
using System.Linq;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class EventRepeatFormatTests
{
    private static readonly CultureInfo English = new("en-GB");

    private static readonly RecurrenceWeekday[] AllWeekdays =
    [
        RecurrenceWeekday.Monday,
        RecurrenceWeekday.Tuesday,
        RecurrenceWeekday.Wednesday,
        RecurrenceWeekday.Thursday,
        RecurrenceWeekday.Friday,
        RecurrenceWeekday.Saturday,
        RecurrenceWeekday.Sunday,
    ];

    private static RepeatPhrase Phrase(
        RepeatRhythm rhythm,
        CultureInfo? culture = null,
        string alternativeDays = "") =>
        EventRepeatFormat.PhraseOf(rhythm, culture ?? English, alternativeDays);

    [Fact]
    public void A_weekly_rule_names_its_weekdays()
    {
        Assert.Equal(
            "Tuesday",
            Phrase(new RepeatRhythm.Weekly(1, [RecurrenceWeekday.Tuesday])).Days);
        Assert.Equal(
            "Monday, Friday",
            Phrase(new RepeatRhythm.Weekly(
                1,
                [RecurrenceWeekday.Monday, RecurrenceWeekday.Friday])).Days);
    }

    [Fact]
    public void Every_weekday_is_named_by_its_own_name()
    {
        // DayNames is indexed from Sunday and the core counts from Monday, so an off-by-one here
        // would rename every day of the week, and read perfectly plausibly while doing it.
        Assert.Equal(
            "Monday, Tuesday, Wednesday, Thursday, Friday, Saturday, Sunday",
            Phrase(new RepeatRhythm.Weekly(1, AllWeekdays)).Days);
    }

    [Fact]
    public void A_rule_that_skips_periods_does_not_reach_the_every_period_frame()
    {
        // The bug this whole surface exists to fix: a fortnightly meeting reading as "Weekly".
        var fortnightly = Phrase(new RepeatRhythm.Weekly(2, [RecurrenceWeekday.Tuesday]));
        Assert.Equal(RepeatFrame.WeeklyEvery, fortnightly.Frame);
        Assert.Equal(2, fortnightly.Interval);
    }

    [Fact]
    public void Every_rhythm_has_a_frame_for_repeating_every_period_and_one_for_skipping()
    {
        // A Fact over the pairs rather than a Theory: the frames are internal, and an InlineData
        // argument has to be as accessible as the test method xUnit calls.
        (RepeatFrame EveryPeriod, RepeatFrame Skipping)[] pairs =
        [
            (RepeatFrame.Daily, RepeatFrame.DailyEvery),
            (RepeatFrame.Weekly, RepeatFrame.WeeklyEvery),
            (RepeatFrame.MonthlyOnDay, RepeatFrame.MonthlyOnDayEvery),
            (RepeatFrame.MonthlyOnLastDay, RepeatFrame.MonthlyOnLastDayEvery),
            (RepeatFrame.MonthlyOnWeekday, RepeatFrame.MonthlyOnWeekdayEvery),
            (RepeatFrame.YearlyOnDate, RepeatFrame.YearlyOnDateEvery),
            (RepeatFrame.YearlyOnWeekday, RepeatFrame.YearlyOnWeekdayEvery),
        ];
        foreach (var (everyPeriod, skipping) in pairs)
        {
            Assert.Equal(everyPeriod, Phrase(RhythmFor(everyPeriod, 1)).Frame);
            Assert.Equal(skipping, Phrase(RhythmFor(everyPeriod, 3)).Frame);
        }
    }

    [Fact]
    public void A_monthly_rule_counting_a_weekdays_position_spells_the_position_out()
    {
        var phrase = Phrase(
            new RepeatRhythm.MonthlyOnWeekday(1, 4, RecurrenceWeekday.Monday));
        Assert.Equal(RepeatFrame.MonthlyOnWeekday, phrase.Frame);
        Assert.Equal(new RepeatPosition(RepeatOrdinal.Fourth, false, "Monday"), phrase.Position);
    }

    [Fact]
    public void The_position_past_the_fifth_is_the_last_one()
    {
        Assert.Equal(RepeatOrdinal.Fifth, EventRepeatFormat.OrdinalOf(5));
        Assert.Equal(RepeatOrdinal.Last, EventRepeatFormat.OrdinalOf(-1));
    }

    [Fact]
    public void The_alternative_ordinal_is_chosen_by_iso_weekday_number()
    {
        // Italian inflects the ordinal for domenica alone; Portuguese for segunda through sexta.
        // Both sets are written as ISO numbers, so reading them against any other numbering, the
        // Sunday-first one DayNames uses, say, inflects the wrong days.
        Assert.Equal([RecurrenceWeekday.Sunday], AlternativeForm("7"));
        Assert.Equal(
            [
                RecurrenceWeekday.Monday,
                RecurrenceWeekday.Tuesday,
                RecurrenceWeekday.Wednesday,
                RecurrenceWeekday.Thursday,
                RecurrenceWeekday.Friday,
            ],
            AlternativeForm("1,2,3,4,5"));
        // The five languages where the question does not arise say nothing, and nothing inflects.
        Assert.Empty(AlternativeForm(string.Empty));
    }

    [Fact]
    public void The_weekday_and_month_names_come_from_the_readers_language_not_from_ours()
    {
        var dutch = new CultureInfo("nl-NL");
        Assert.Equal(
            "dinsdag",
            Phrase(new RepeatRhythm.Weekly(1, [RecurrenceWeekday.Tuesday]), dutch).Days);
        Assert.Equal("augustus", Phrase(new RepeatRhythm.YearlyOnDate(1, 8, 25), dutch).Month);
    }

    [Fact]
    public void An_end_date_is_written_without_the_weekday_the_sentence_is_already_about()
    {
        // "Weekly on Monday, until Thursday, 3 June 2027" reads as a second weekday in the rule.
        Assert.Equal("3 Jun 2027", EventRepeatFormat.EndDate("2027-06-03", English));
    }

    [Fact]
    public void An_end_date_that_will_not_parse_is_shown_as_it_arrived()
    {
        Assert.Equal("later", EventRepeatFormat.EndDate("later", English));
    }

    private static RecurrenceWeekday[] AlternativeForm(string catalogEntry)
    {
        var iso = EventRepeatFormat.AlternativeWeekdays(catalogEntry);
        return AllWeekdays.Where(day => iso.Contains(EventRepeatFormat.IsoWeekday(day))).ToArray();
    }

    /// <summary>A rhythm reaching <paramref name="everyPeriod"/>'s pair, repeating every
    /// <paramref name="interval"/> periods.</summary>
    private static RepeatRhythm RhythmFor(RepeatFrame everyPeriod, uint interval) => everyPeriod switch
    {
        RepeatFrame.Daily => new RepeatRhythm.Daily(interval),
        RepeatFrame.Weekly => new RepeatRhythm.Weekly(interval, [RecurrenceWeekday.Tuesday]),
        RepeatFrame.MonthlyOnDay => new RepeatRhythm.MonthlyOnDay(interval, 25),
        RepeatFrame.MonthlyOnLastDay => new RepeatRhythm.MonthlyOnLastDay(interval),
        RepeatFrame.MonthlyOnWeekday =>
            new RepeatRhythm.MonthlyOnWeekday(interval, 4, RecurrenceWeekday.Monday),
        RepeatFrame.YearlyOnDate => new RepeatRhythm.YearlyOnDate(interval, 8, 25),
        _ => new RepeatRhythm.YearlyOnWeekday(interval, 4, RecurrenceWeekday.Monday, 8),
    };
}
