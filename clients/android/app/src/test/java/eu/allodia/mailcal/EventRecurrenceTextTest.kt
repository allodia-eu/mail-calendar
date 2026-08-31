// The repeat summary as the user reads it, over the sentence parts the core decides.
//
// Which sentence a rule gets is pinned in the core (`repeat_summary_tests.rs`), once for every
// client. What is pinned here is the half that is genuinely this client's: that each arm reaches
// its catalog frame, that the platform's own weekday and month names are used, and that the
// ordinal agrees with the weekday it counts in the two languages where that is not automatic.
package eu.allodia.mailcal

import android.content.Context
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.RuntimeEnvironment.setQualifiers
import uniffi.mailcal_bindings.RecurrenceWeekday
import uniffi.mailcal_bindings.RepeatRhythm
import uniffi.mailcal_bindings.RepeatStop
import uniffi.mailcal_bindings.RepeatSummary

@RunWith(RobolectricTestRunner::class)
class EventRecurrenceTextTest {
    private fun ctx(): Context = RuntimeEnvironment.getApplication()

    private fun summary(
        rhythm: RepeatRhythm,
        stop: RepeatStop = RepeatStop.Never,
        locale: Locale = Locale.UK,
    ) = recurrenceSummary(ctx(), RepeatSummary(rhythm, stop), isRecurring = true, locale = locale)

    /** The fourth [day] of every month. */
    private fun fourthOf(day: RecurrenceWeekday) =
        RepeatRhythm.MonthlyOnWeekday(interval = 1u, nth = 4, day = day)

    private fun weekly(interval: UInt, vararg days: RecurrenceWeekday) =
        RepeatRhythm.Weekly(interval, days.toList())

    @Test
    fun `a weekly rule names its weekdays`() {
        assertEquals(
            L10n.event_repeat_sum_weekly(ctx(), "Tuesday"),
            summary(weekly(1u, RecurrenceWeekday.TUESDAY)),
        )
        assertEquals(
            L10n.event_repeat_sum_weekly(ctx(), "Monday, Friday"),
            summary(weekly(1u, RecurrenceWeekday.MONDAY, RecurrenceWeekday.FRIDAY)),
        )
    }

    @Test
    fun `a rule that skips periods says how many, rather than borrowing the frequency's word`() {
        val text = summary(weekly(2u, RecurrenceWeekday.TUESDAY))
        assertEquals(L10n.event_repeat_sum_weekly_n(ctx(), 2, "Tuesday"), text)
        assertTrue("the interval is on screen: $text", text.contains("2"))
        assertTrue(
            "a fortnightly rule must not read as the weekly one: $text",
            text != L10n.event_repeat_sum_weekly(ctx(), "Tuesday"),
        )
    }

    @Test
    fun `a monthly rule counting a weekday's position spells the position out`() {
        assertEquals(
            L10n.event_repeat_sum_monthly_nth(
                ctx(),
                L10n.event_repeat_nth_fourth(ctx(), "Monday"),
            ),
            summary(fourthOf(RecurrenceWeekday.MONDAY)),
        )
    }

    @Test
    fun `an end is part of the sentence, not dropped from it`() {
        val until = summary(
            RepeatRhythm.Daily(interval = 1u),
            stop = RepeatStop.OnDate("2027-06-03"),
        )
        assertTrue("the end date is stated: $until", until.contains("2027"))
        assertTrue(
            "the rhythm survives beside it: $until",
            until.startsWith(L10n.event_repeat_daily(ctx())),
        )

        assertEquals(
            L10n.event_repeat_sum_times(ctx(), L10n.event_repeat_daily(ctx()), 12),
            summary(RepeatRhythm.Daily(interval = 1u), stop = RepeatStop.AfterCount(12u)),
        )
    }

    @Test
    fun `an event with no summary says it repeats, and one with no rule says it does not`() {
        // The core sends no summary for a rule it will not state exactly, the client must not
        // invent a rhythm for it, and must not call the event a one-off either.
        assertEquals(
            L10n.event_repeat_other(ctx()),
            recurrenceSummary(ctx(), null, isRecurring = true, locale = Locale.UK),
        )
        assertEquals(
            L10n.event_repeat_none(ctx()),
            recurrenceSummary(ctx(), null, isRecurring = false, locale = Locale.UK),
        )
    }

    @Test
    fun `an Italian ordinal agrees with the weekday it counts`() {
        setQualifiers("it")
        val italian = Locale.forLanguageTag("it")
        val monday = summary(fourthOf(RecurrenceWeekday.MONDAY), locale = italian)
        val sunday = summary(fourthOf(RecurrenceWeekday.SUNDAY), locale = italian)
        // "il quarto lunedì" but "la quarta domenica": domenica is the one Italian weekday whose
        // gender differs, so a single ordinal phrase would be wrong for it.
        assertTrue("masculine for lunedì: $monday", monday.contains("il quarto"))
        assertTrue("feminine for domenica: $sunday", sunday.contains("la quarta"))
    }

    @Test
    fun `a Portuguese ordinal agrees with the weekday it counts`() {
        setQualifiers("pt")
        val portuguese = Locale.forLanguageTag("pt")
        val monday = summary(fourthOf(RecurrenceWeekday.MONDAY), locale = portuguese)
        val saturday = summary(fourthOf(RecurrenceWeekday.SATURDAY), locale = portuguese)
        // Most Portuguese weekdays are feminine (segunda-feira … sexta-feira) and the weekend is
        // not, so this is the language where getting it wrong shows up most often.
        assertTrue("feminine for segunda-feira: $monday", monday.contains("na quarta"))
        assertTrue("masculine for sábado: $saturday", saturday.contains("no quarto"))
    }

    @Test
    fun `the weekday and month names come from the device's language, not from ours`() {
        val dutch = Locale.forLanguageTag("nl")
        val text = summary(weekly(1u, RecurrenceWeekday.TUESDAY), locale = dutch)
        assertTrue("a Dutch weekday, not an English one: $text", text.contains("dinsdag"))

        val yearly = summary(
            RepeatRhythm.YearlyOnDate(interval = 1u, month = 8u, day = 25u),
            locale = dutch,
        )
        assertTrue("a Dutch month: $yearly", yearly.contains("augustus"))
    }
}
