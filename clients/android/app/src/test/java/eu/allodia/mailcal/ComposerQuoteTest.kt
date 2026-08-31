// The quoted-original seed the composer injects into the editor on reply/forward. It is assembled
// client-side (the core carries no runtime localisation), so its shape and its guard against
// seeding a body that hasn't arrived are the client's to get right.
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
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.ReadingSnapshot

private val MESSAGE = OpenedMessage(
    account = "acct-1",
    key = "m1",
    subject = "Quarterly report",
    from = "sender@remote.test",
    avatar = stubAvatar(),
    date = "10 July 2026 at 13:34",
)

private fun snapshot(
    key: String = "m1",
    html: String? = "<p>Original</p>",
    plain: String? = "Original",
) = ReadingSnapshot(
    key = key,
    from = "sender@remote.test",
    avatar = stubAvatar(),
    to = "me@allodia.local",
    cc = "",
    bcc = "",
    html = html,
    plain = plain,
    hasRemoteImages = false,
    loadError = false,
    attachments = emptyList(),
    invitation = null,
    pending = false,
)

// Robolectric's own accessor, so the suite needs no androidx.test:core dependency.
private fun ctx(): Context = RuntimeEnvironment.getApplication()

@RunWith(RobolectricTestRunner::class)
class ComposerQuoteTest {

    private fun seed(
        reading: ReadingSnapshot?,
        style: QuoteStyleKind = QuoteStyleKind.INDENTED,
        isForward: Boolean = false,
        initialText: String? = null,
    ): String? = ComposerQuote.seedJson(ctx(), style, MESSAGE, reading, isForward, initialText)

    @Test
    fun there_is_nothing_to_quote_before_the_body_has_loaded() {
        assertNull(seed(null))
    }

    @Test
    fun a_snapshot_for_a_different_message_is_never_quoted() {
        // The reading snapshot lags the open message by a beat; quoting it would paste the
        // PREVIOUS message's body into this reply.
        assertNull(seed(snapshot(key = "some-other-message")))
    }

    @Test
    fun an_empty_body_yields_no_quote() {
        assertNull(seed(snapshot(html = null, plain = null)))
        assertNull(seed(snapshot(html = "", plain = "")))
    }

    @Test
    fun a_reply_carries_the_body_and_a_one_line_attribution() {
        val json = JSONObject(seed(snapshot())!!)

        assertEquals("Indented", json.getString("style"))
        assertEquals("<p>Original</p>", json.getString("body_html"))
        assertEquals("Original", json.getString("body_plain"))
        val line = json.getJSONObject("attribution").getString("line")
        assertTrue("attribution names the sender and date: $line", line.contains("sender@remote.test"))
        assertTrue(line.contains("10 July 2026"))
    }

    @Test
    fun a_forward_replaces_the_attribution_with_the_forwarded_marker() {
        val json = JSONObject(seed(snapshot(), isForward = true)!!)
        val line = json.getJSONObject("attribution").getString("line")

        assertEquals("Forwarded message", line)
    }

    @Test
    fun the_line_and_header_style_carries_a_labeled_header_block() {
        val json = JSONObject(seed(snapshot(), style = QuoteStyleKind.LINE_AND_HEADER)!!)

        assertEquals("LineAndHeader", json.getString("style"))
        val headers = json.getJSONObject("attribution").getJSONArray("headers")
        val labels = (0 until headers.length()).map { headers.getJSONObject(it).getString("label") }
        // Cc is empty on this snapshot, so it is omitted rather than rendered blank.
        assertEquals(listOf("From", "Sent", "To", "Subject"), labels)
    }

    @Test
    fun initial_text_is_omitted_unless_supplied() {
        // Only showcase mode pre-fills the body; a real reply must not carry an `initial_text` key.
        assertFalse(JSONObject(seed(snapshot())!!).has("initial_text"))
        assertEquals(
            "Sounds good",
            JSONObject(seed(snapshot(), initialText = "Sounds good")!!).getString("initial_text"),
        )
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_attribution_is_localized_from_the_shared_catalog() {
        // Proves the l10n codegen wired BOTH catalogs into resources, not just the default one.
        val json = JSONObject(seed(snapshot(), isForward = true)!!)

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
        // keys seedJson uses. Assert they line up against an actual seed rather than hardcoding.
        val example = ComposerQuote.example(ctx())
        val real = JSONObject(seed(snapshot(), style = QuoteStyleKind.LINE_AND_HEADER)!!)
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
