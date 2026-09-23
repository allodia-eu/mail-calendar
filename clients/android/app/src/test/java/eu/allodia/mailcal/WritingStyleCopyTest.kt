// Every word Writing style puts on screen comes from a variant the core named, looked up in the
// catalog here (docs/ai.md: a client words each failure, never a server's sentence). Pinned per
// variant, because a mapping that sends two failures to one sentence reads fine until somebody
// needs the one it hid.
package eu.allodia.mailcal

import android.content.Context
import java.time.LocalDate
import java.time.ZoneId
import java.util.Locale
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.CreditBalance
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.JurisdictionMode
import uniffi.mailcal_bindings.OwnEndpointException
import uniffi.mailcal_bindings.WritingStyleFailure

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private val AMSTERDAM: ZoneId = ZoneId.of("Europe/Amsterdam")

@RunWith(RobolectricTestRunner::class)
class WritingStyleCopyTest {

    @Test
    fun every_failure_has_its_own_sentence() {
        val c = ctx()
        val expected = listOf(
            WritingStyleFailure.Unavailable() to L10n.ai_error_unavailable(c),
            WritingStyleFailure.Busy() to L10n.ai_error_busy(c),
            WritingStyleFailure.NoSentFolder() to L10n.ai_error_no_sent_folder(c),
            WritingStyleFailure.NothingToLearn() to L10n.learn_report_nothing(c),
            WritingStyleFailure.NoStyle() to L10n.ai_error_no_style(c),
            WritingStyleFailure.NotFound() to L10n.ai_error_not_found(c),
            WritingStyleFailure.OutOfCredits() to L10n.ai_error_out_of_credits(c),
            WritingStyleFailure.NotEntitled() to L10n.ai_error_not_entitled(c),
            WritingStyleFailure.RateLimited() to L10n.ai_error_rate_limited(c),
            WritingStyleFailure.Unreachable() to L10n.ai_error_unreachable(c),
            WritingStyleFailure.Malformed() to L10n.ai_error_malformed(c),
            WritingStyleFailure.Cancelled() to L10n.ai_error_cancelled(c),
        )
        for ((failure, sentence) in expected) {
            assertEquals(failure::class.simpleName, sentence, writingStyleFailureText(c, failure, AiRoute.RELAY))
        }
        assertEquals(
            "every sentence is a different one",
            expected.size,
            expected.map { it.second }.toSet().size,
        )
    }

    /** Through the relay the Allodia sign-in was refused; an own endpoint refuses its key. */
    @Test
    fun a_refused_authorisation_names_what_to_fix_for_the_route() {
        val failure = WritingStyleFailure.Unauthorized()
        assertEquals(L10n.ai_error_sign_in_again(ctx()), writingStyleFailureText(ctx(), failure, AiRoute.RELAY))
        assertEquals(
            L10n.ai_error_key_refused(ctx()),
            writingStyleFailureText(ctx(), failure, AiRoute.OWN_ENDPOINT),
        )
    }

    @Test
    fun the_gate_s_refusal_is_worded_by_the_mode_in_force() {
        val native = WritingStyleFailure.Refused(JurisdictionMode.EU_NATIVE, JurisdictionClass.NON_EU)
        val hosted = WritingStyleFailure.Refused(JurisdictionMode.EU_HOSTED, JurisdictionClass.NON_EU)
        assertEquals(L10n.writing_style_refused_eu_native(ctx()), writingStyleFailureText(ctx(), native, null))
        assertEquals(L10n.writing_style_refused_eu_hosted(ctx()), writingStyleFailureText(ctx(), hosted, null))
    }

    @Test
    fun another_status_names_its_code_and_nothing_the_server_said() {
        assertEquals(
            "The AI endpoint answered with an error (503).",
            writingStyleFailureText(ctx(), WritingStyleFailure.Status(503u), AiRoute.OWN_ENDPOINT),
        )
    }

    @Test
    fun anything_but_a_failure_reads_as_an_answer_that_could_not_be_read() {
        assertEquals(WritingStyleFailure.Malformed::class, IllegalStateException().asWritingStyleFailure()::class)
        val busy = WritingStyleFailure.Busy()
        assertEquals(busy, busy.asWritingStyleFailure())
    }

    @Test
    fun each_endpoint_error_names_the_field_to_fix() {
        assertEquals(L10n.ai_endpoint_error_address(ctx()), ownEndpointErrorText(ctx(), OwnEndpointException.InvalidUrl()))
        assertEquals(L10n.ai_endpoint_error_https(ctx()), ownEndpointErrorText(ctx(), OwnEndpointException.NotHttps()))
        assertEquals(L10n.ai_endpoint_error_model(ctx()), ownEndpointErrorText(ctx(), OwnEndpointException.NoModel()))
        // The keystore's own words stay out of the sentence.
        assertEquals(
            L10n.ai_endpoint_error_keystore(ctx()),
            ownEndpointErrorText(ctx(), OwnEndpointException.Keystore("KeyStoreException: 42")),
        )
    }

    /** An empty key field keeps the stored key rather than removing it. */
    @Test
    fun an_empty_key_field_keeps_the_stored_key() {
        assertNull(ownEndpointKeyArgument(""))
        assertEquals("sk-local", ownEndpointKeyArgument("sk-local"))
    }

    @Test
    fun languages_are_named_in_their_own_tongue() {
        assertEquals("Nederlands, English", writingStyleLanguages(ctx(), listOf("nl", "en")))
        assertEquals("sv", writingStyleLanguages(ctx(), listOf("sv")))
    }

    @Test
    fun one_message_takes_the_singular() {
        val utc = ZoneId.of("UTC")
        val date = epochDate(PLAIN_STYLE.learnedAt, utc, Locale.UK)
        val lines = writingStyleRowLines(ctx(), PLAIN_STYLE.copy(messages = 1u), utc, Locale.UK)
        assertEquals("Learned from 1 message on $date", lines[0])
        assertEquals("Languages: English, Nederlands", lines[1])
        assertEquals(
            "Learned from 12 messages on $date",
            writingStyleRowLines(ctx(), PLAIN_STYLE, utc, Locale.UK)[0],
        )
        assertTrue(date, date.contains("2026"))
    }

    /** A style from another device carries no learning date; it says nothing rather than 1970. */
    @Test
    fun a_style_with_no_learning_date_says_nothing_about_it() {
        val lines = writingStyleRowLines(ctx(), PLAIN_STYLE.copy(learnedAt = 0), AMSTERDAM, Locale.UK)
        assertEquals(listOf("Languages: English, Nederlands"), lines)
    }

    @Test
    fun the_report_leaves_out_what_would_say_nothing() {
        assertEquals(
            listOf(
                "Messages you sent: 42",
                "Long enough to learn from: 30",
                "Languages: English",
            ),
            corpusReportLines(ctx(), corpusReport(), AMSTERDAM, Locale.UK),
        )
        val full = corpusReportLines(
            ctx(),
            corpusReport(undetected = 3u, horizon = PLAIN_STYLE.learnedAt),
            AMSTERDAM,
            Locale.UK,
        )
        assertEquals("Left out, in other languages: 3", full[3])
        val date = epochDate(PLAIN_STYLE.learnedAt, AMSTERDAM, Locale.UK)
        assertEquals("This device holds your sent mail back to $date.", full[4])
    }

    /** The core's `until` is inclusive, so the chosen day ends at its last second, local time. */
    @Test
    fun up_to_a_date_ends_at_the_last_second_of_that_day_locally() {
        // 2025-01-01T00:00:00+01:00 is 1735686000.
        assertEquals(1_735_685_999L, endOfDay(LocalDate.of(2024, 12, 31), AMSTERDAM))
    }

    @Test
    fun credits_show_at_most_one_decimal_in_the_locale_s_digits() {
        val balance = CreditBalance(credits = 12.345, asOf = 0)
        assertEquals("12.3 credits left, as of 09:00", writingStyleCreditsText(ctx(), balance, Locale.UK, "09:00"))
        assertEquals(
            "40 credits left, as of 09:00",
            writingStyleCreditsText(ctx(), balance.copy(credits = 40.0), Locale.UK, "09:00"),
        )
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_credits_line_is_dutch_in_dutch() {
        val balance = CreditBalance(credits = 12.345, asOf = 0)
        assertEquals(
            "Nog 12,3 credits, stand van 09:00",
            writingStyleCreditsText(ctx(), balance, Locale.forLanguageTag("nl"), "09:00"),
        )
        assertEquals("Er wordt al een stijl geleerd.", writingStyleFailureText(ctx(), WritingStyleFailure.Busy(), null))
    }

    @Test
    fun an_engine_timestamp_goes_through_the_list_rows_formatter() {
        assertEquals("2026-09-01T10:00:00Z", engineTimestamp(PLAIN_STYLE.learnedAt))
    }

    @Test
    fun the_consent_names_the_endpoint_s_host() {
        assertEquals("api.example.eu", endpointHost("https://api.example.eu/v1"))
        assertEquals("127.0.0.1", endpointHost("http://127.0.0.1:28434/v1"))
    }

    @Test
    fun the_description_is_asked_for_in_the_language_on_screen() {
        assertEquals("en", catalogLocale(ctx()))
    }

    @Test
    @Config(qualifiers = "de")
    fun german_on_screen_asks_for_german() {
        assertEquals("de", catalogLocale(ctx()))
    }

    @Test
    @Config(qualifiers = "sv")
    fun a_language_the_catalog_does_not_ship_asks_for_the_base_one() {
        assertEquals("en", catalogLocale(ctx()))
    }

    @Test
    fun the_reveal_skips_what_the_model_said_nothing_about() {
        val fields = revealFields(ctx(), PLAIN_DETAIL.languages.single(), Locale.UK)
        assertEquals(
            listOf(
                L10n.reveal_greetings(ctx()),
                L10n.reveal_sign_offs(ctx()),
                L10n.reveal_signs_as(ctx()),
                null,
                L10n.reveal_shape(ctx()),
                L10n.reveal_phrases(ctx()),
            ),
            fields.map { it.heading },
        )
        assertEquals(RevealLine("Hi Anna,", "70%"), fields[0].lines.single())
        assertEquals("Usually about 80 words", fields[3].lines.single().text)
    }
}
