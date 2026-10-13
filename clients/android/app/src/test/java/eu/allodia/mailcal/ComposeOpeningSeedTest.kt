// How a composer opens from the core's `ComposeOpening`: which kind it is, which one seed the editor
// gets, whether it may touch the signature, the attribution a reopened quote is completed with, the
// style a reopened quote's picker opens on, and the account From opens on (docs/drafts.md).
//
// Each fails silently on screen: a reply titled as a new message, a second signature under a
// stored one, a picker showing the wrong style. Robolectric, for org.json and the L10n catalog.
package eu.allodia.mailcal

import android.content.Context
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposeKind
import uniffi.mailcal_bindings.ComposeOpening
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private const val LABELS = """{"placeholder":"Write a message"}"""

@RunWith(RobolectricTestRunner::class)
class ComposeOpeningSeedTest {

    private fun contextFor(opening: ComposeOpening) = composeContext(
        ctx = ctx(),
        opening = opening,
        initialFrom = null,
        quoteSettings = QuoteSettings(QuoteStyleKind.INDENTED, perMessage = true),
        dateText = { "2026-07-10 15:34" },
    )

    private fun account(id: String) = AccountRow(id = id, email = "$id@example.test", name = "", expanded = false)

    @Test
    fun `the heading follows the kind the core names, a resumed reply included`() {
        assertEquals(RichComposeMode.New, ComposeOpeningSeed.modeOf(ComposeKind.NEW))
        assertEquals(RichComposeMode.Reply, ComposeOpeningSeed.modeOf(ComposeKind.REPLY))
        assertEquals(RichComposeMode.ReplyAll, ComposeOpeningSeed.modeOf(ComposeKind.REPLY_ALL))
        assertEquals(RichComposeMode.Forward, ComposeOpeningSeed.modeOf(ComposeKind.FORWARD))
        val resumed = testOpening(kind = ComposeKind.REPLY, bodyHtml = "<p>Hi</p>", quote = testQuote())
        assertEquals(RichComposeMode.Reply, contextFor(resumed).mode)
    }

    @Test
    fun `a fresh reply seeds its quote with what a prefill brought above it`() {
        val request = contextFor(
            testOpening(kind = ComposeKind.REPLY, bodyText = "Sounds good", quote = testQuote()),
        )

        val quote = JSONObject(checkNotNull(request.quote))
        assertEquals("Sounds good", quote.getString("initial_text"))
        assertEquals("", request.initialBody)
        assertNull("a fresh quote has nothing to complete", request.quoteAttribution)
        assertTrue(request.showsStylePicker)
    }

    @Test
    fun `a reply from a row whose body was never opened still quotes`() {
        // The quote is the core's answer, not the reading pane's body.
        val request = contextFor(testOpening(kind = ComposeKind.REPLY_ALL, quote = testQuote()))

        assertEquals("<p>Original</p>", JSONObject(checkNotNull(request.quote)).getString("body_html"))
    }

    @Test
    fun `a stored message opens through its own HTML and completes its quote's attribution`() {
        val request = contextFor(
            testOpening(
                kind = ComposeKind.FORWARD,
                bodyHtml = "<p>See below</p>",
                bodyText = "See below",
                quote = testQuote(),
            ),
        )

        assertNull("the stored HTML already holds the quote", request.quote)
        assertEquals("<p>See below</p>", request.storedHtml)
        assertEquals("See below", request.initialBody)
        val completion = JSONObject(checkNotNull(request.quoteAttribution))
        assertEquals("Forwarded message", completion.getString("line"))
        assertEquals(
            listOf("From", "Sent", "To", "Subject"),
            (0 until completion.getJSONArray("headers").length()).map {
                completion.getJSONArray("headers").getJSONObject(it).getString("label")
            },
        )
        assertTrue(request.showsStylePicker)
    }

    @Test
    fun `a stored message whose original is gone completes nothing and offers no picker`() {
        val request = contextFor(testOpening(bodyHtml = "<p>Draft</p>", bodyText = "Draft"))

        assertNull(request.quoteAttribution)
        assertFalse(request.showsStylePicker)
    }

    @Test
    fun `a mail link's text opens as plain text when there is no quote`() {
        val request = contextFor(testOpening(to = "bob@test.local", bodyText = "Order #123"))

        assertNull(request.quote)
        assertEquals("Order #123", request.initialBody)
        assertTrue("the link supplied the recipient, so the body takes the caret", request.opensInBody)
    }

    @Test
    fun `a composer that seeds no signature has none to seed, swap or offer`() {
        val signatures = ComposerSignatures(library = emptyList(), forAccount = { _, _ -> null }, byId = { null })

        val stored = testOpening(bodyHtml = "<p>x</p>", seedsSignature = false)
        assertNull(ComposeOpeningSeed.signing(signatures, stored))
        assertSame(signatures, ComposeOpeningSeed.signing(signatures, testOpening(seedsSignature = true)))
    }

    @Test
    fun `a reopened quote's picker opens on the style it was saved in`() {
        val lineAndHeader = """{"blocks":[{"Paragraph":{}},{"Quote":{"style":"LineAndHeader"}}]}"""
        val indented = """{"blocks":[{"Quote":{"style":"Indented"}}]}"""

        assertEquals(QuoteStyleKind.LINE_AND_HEADER, ComposeOpeningSeed.quoteStyleOf(lineAndHeader))
        assertEquals(QuoteStyleKind.INDENTED, ComposeOpeningSeed.quoteStyleOf(indented))
        assertNull(ComposeOpeningSeed.quoteStyleOf("""{"blocks":[{"Paragraph":{}}]}"""))
        assertNull(ComposeOpeningSeed.quoteStyleOf("not json"))
        assertNull(ComposeOpeningSeed.quoteStyleOf(null))
    }

    @Test
    fun `from opens on the opening's account, then the default, then the first`() {
        val accounts = listOf(account("work"), account("home"))

        assertEquals("home", sendAccountFor("home", defaultSendAccount = "work", accounts = accounts))
        assertEquals("work", sendAccountFor("gone", defaultSendAccount = "work", accounts = accounts))
        assertEquals("work", sendAccountFor(null, defaultSendAccount = "removed", accounts = accounts))
        assertNull(sendAccountFor("home", defaultSendAccount = null, accounts = emptyList()))
    }

    @Test
    fun `exactly one body seed is sent, a stored body ahead of a quote`() {
        val scripts = composerPageFinishedScripts(
            LABELS,
            quote = """{"blocks":[]}""",
            topInsetDp = 208f,
            body = "Draft",
            storedHtml = "<p>Draft</p>",
        )

        assertEquals(1, scripts.count { it.startsWith("window.setComposerBody(") })
        assertTrue(scripts.none { it.contains("setComposerQuote") || it.contains("setPlainText") })
    }

    @Test
    fun `a quote carries the prefill, so no separate plain text is seeded over it`() {
        val scripts = composerPageFinishedScripts(
            LABELS,
            quote = """{"blocks":[]}""",
            topInsetDp = 208f,
            body = "Sounds good",
        )

        assertNotNull(scripts.singleOrNull { it.startsWith("window.setComposerQuote(") })
        assertTrue(scripts.none { it.contains("setPlainText") })
    }

    @Test
    fun `a reopened quote is completed after the signature, last of all`() {
        val scripts = composerPageFinishedScripts(
            LABELS,
            quote = null,
            topInsetDp = 208f,
            signature = """{"body_html":"<p>Alice</p>","body_plain":"Alice"}""",
            body = "Draft",
            storedHtml = "<p>Draft</p>",
            quoteAttribution = """{"line":"On …","headers":[]}""",
        )

        val signature = scripts.indexOfFirst { it.startsWith("window.setComposerSignature(") }
        val completion = scripts.indexOfFirst { it.startsWith("window.completeComposerQuoteAttribution(") }
        assertTrue(signature >= 0 && completion > signature)
        assertEquals(scripts.size - 1, completion)
    }
}
