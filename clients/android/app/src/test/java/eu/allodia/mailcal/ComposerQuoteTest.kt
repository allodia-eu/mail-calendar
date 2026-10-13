// The quoted original's seed, worded client-side (the core carries no runtime localisation) from
// the message the core says a reply or forward quotes (`ComposeOpening.quote`). Its shape and its
// guard against seeding an empty body are the client's to get right.
//
// Robolectric, because the attribution line comes from the generated L10n catalog.
package eu.allodia.mailcal

import android.content.Context
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.ComposeKind
import uniffi.mailcal_bindings.QuoteSource
import uniffi.mailcal_bindings.QuoteStyleKind

// What the reading header shows for the original's date: `localDateTime` output, applied by the
// host before the words are built.
private const val DATE_TEXT = "2026-07-10 15:34"

// Robolectric's own accessor, so the suite needs no androidx.test:core dependency.
private fun ctx(): Context = RuntimeEnvironment.getApplication()

@RunWith(RobolectricTestRunner::class)
class ComposerQuoteTest {

    private fun seed(
        quote: QuoteSource = testQuote(),
        style: QuoteStyleKind = QuoteStyleKind.INDENTED,
        isForward: Boolean = false,
        initialText: String? = null,
    ): String? {
        val kind = if (isForward) ComposeKind.FORWARD else ComposeKind.REPLY
        val words = ComposerQuote.words(ctx(), quote, DATE_TEXT, isForward)
        return ComposeOpeningSeed.quoteSeed(testOpening(kind = kind, quote = quote), words, style, initialText)
    }

    @Test
    fun the_date_the_host_formatted_reaches_the_quote_intact() {
        // The words carry the date as the reading header shows it, in both styles. Formatting it a
        // second time here is the failure this guards: the recipient would get a different date
        // from the one the user read, or the date with its time removed.
        val json = JSONObject(seed()!!)
        val attribution = json.getJSONObject("attribution")
        val line = attribution.getString("line")
        val sent = attribution.getJSONArray("headers").getJSONObject(1).getString("value")
        assertTrue("the time was dropped from the attribution: $line", line.contains(DATE_TEXT))
        assertEquals(DATE_TEXT, sent)
    }

    @Test
    fun an_empty_body_yields_no_quote() {
        assertNull(seed(testQuote(bodyHtml = "", bodyPlain = "")))
    }

    @Test
    fun a_reply_carries_the_body_and_a_one_line_attribution() {
        val json = JSONObject(seed()!!)

        assertEquals("Indented", json.getString("style"))
        assertEquals("<p>Original</p>", json.getString("body_html"))
        assertEquals("Original", json.getString("body_plain"))
        val line = json.getJSONObject("attribution").getString("line")
        assertTrue("attribution names the sender and date: $line", line.contains("sender@remote.test"))
        assertTrue("attribution carries the formatted date: $line", line.contains("2026-07-10"))
    }

    @Test
    fun a_forward_replaces_the_attribution_with_the_forwarded_marker() {
        val json = JSONObject(seed(isForward = true)!!)
        val line = json.getJSONObject("attribution").getString("line")

        assertEquals("Forwarded message", line)
    }

    @Test
    fun the_line_and_header_style_carries_a_labeled_header_block() {
        val json = JSONObject(seed(style = QuoteStyleKind.LINE_AND_HEADER)!!)

        assertEquals("LineAndHeader", json.getString("style"))
        val headers = json.getJSONObject("attribution").getJSONArray("headers")
        val labels = (0 until headers.length()).map { headers.getJSONObject(it).getString("label") }
        // Cc is empty on this original, so it is omitted rather than rendered blank.
        assertEquals(listOf("From", "Sent", "To", "Subject"), labels)
    }

    @Test
    fun a_cc_on_the_original_gets_its_own_header() {
        val words = ComposerQuote.words(ctx(), testQuote(cc = "carol@remote.test"), DATE_TEXT, false)

        assertEquals(listOf("From", "Sent", "To", "Cc", "Subject"), words.headers.map { it.first })
        assertEquals("carol@remote.test", words.headers[3].second)
    }

    @Test
    fun initial_text_is_omitted_unless_supplied() {
        // A prefill (the showcase's sample answer) goes above the quote; a plain reply carries no
        // `initial_text` key.
        assertFalse(JSONObject(seed()!!).has("initial_text"))
        assertFalse(JSONObject(seed(initialText = "")!!).has("initial_text"))
        assertEquals(
            "Sounds good",
            JSONObject(seed(initialText = "Sounds good")!!).getString("initial_text"),
        )
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_attribution_is_localized_from_the_shared_catalog() {
        // Proves the l10n codegen wired BOTH catalogs into resources, not just the default one.
        val json = JSONObject(seed(isForward = true)!!)

        assertEquals("Doorgestuurd bericht", json.getJSONObject("attribution").getString("line"))
    }

    @Test
    fun the_style_tokens_are_the_rust_variant_names_the_editor_switches_on() {
        // These strings are a cross-language contract: the shared editor.html switches its
        // `data-quote-style` on them and the Rust composer deserializes them straight back into
        // its QuoteStyle. Drift here silently falls the editor back to the default style, with no
        // error anywhere, hence pinning them on both sides (mailcal-composer has the twin test).
        assertEquals("Indented", ComposerQuote.token(QuoteStyleKind.INDENTED))
        assertEquals("LineAndHeader", ComposerQuote.token(QuoteStyleKind.LINE_AND_HEADER))
    }

    @Test
    fun the_style_picker_is_hidden_unless_there_is_a_quote_and_the_user_opted_in() {
        // The per-message override is an advanced opt-in: an ordinary reply shows no picker and
        // just uses the app default. A new message has nothing to style, so even opted in it
        // shows none.
        assertTrue(ComposerQuote.showsStylePicker(hasQuote = true, perMessage = true))
        assertFalse(ComposerQuote.showsStylePicker(hasQuote = true, perMessage = false))
        assertFalse(ComposerQuote.showsStylePicker(hasQuote = false, perMessage = true))
        assertFalse(ComposerQuote.showsStylePicker(hasQuote = false, perMessage = false))
    }

    @Test
    fun the_settings_example_reuses_the_same_labels_a_real_quote_carries() {
        // The settings preview exists to show the user what each style looks like, so it must not
        // drift from the real thing: its attribution and header labels come from the same catalog
        // keys a real quote's words use. Assert they line up against an actual seed rather than
        // hardcoding.
        val example = ComposerQuote.example(ctx())
        val real = JSONObject(seed(style = QuoteStyleKind.LINE_AND_HEADER)!!)
        val realHeaders = real.getJSONObject("attribution").getJSONArray("headers")
        val realLabels =
            (0 until realHeaders.length()).map { realHeaders.getJSONObject(it).getString("label") }

        // Same labels, in the same order. (The example has no Cc, which a real quote also drops
        // when empty, so it is a subset of the real vocabulary, never a different one.)
        assertEquals(listOf("From", "Sent", "To", "Subject"), example.headers.map { it.first })
        assertTrue("real quote uses the example's labels: $realLabels", realLabels.containsAll(example.headers.map { it.first }))
        // And the one-line attribution is the same sentence a real reply gets, with sample values.
        assertTrue("example line reads as an attribution: ${example.line}", example.line.contains("Anna Bakker"))
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_settings_example_is_localized_too() {
        val example = ComposerQuote.example(ctx())

        assertEquals(listOf("Van", "Verzonden", "Aan", "Onderwerp"), example.headers.map { it.first })
        assertEquals("Gaat de lunch nog door?", example.body)
    }
}
