// The JS the composer host injects once the editor bundle has parsed (`onPageFinished`). This is an
// *ordering* contract, and it is the kind that fails silently: a `window.*` hook called before the
// bundle has parsed lands on an undefined function, and `evaluateJavascript` reports nothing. So the
// only thing that keeps the editor usable is that every open-time hook is in THIS batch.
//
// The bug this guards: `setComposerTopInset` was sent only from a layout-time `LaunchedEffect`, which
// runs on the pass that measures the header, frames before the 31 KB editor page finishes parsing.
// It lost that race, the call was silently dropped, and the editor kept its 14px CSS default padding
// (`editor.html`: `padding: var(--composer-top-inset, 14px)`). That puts the entire typing area
// underneath the opaque address-field header: the body could not be tapped and nothing typed was
// visible, reported as "I cannot put any text in the message body".
//
// Robolectric, for org.json (JSONObject.quote), the rest is a plain function.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

private const val LABELS = """{"placeholder":"Write a message"}"""

@RunWith(RobolectricTestRunner::class)
class ComposerPageFinishedTest {

    @Test
    fun `the top inset is sent on page-finished, not left to the layout effect`() {
        val scripts = composerPageFinishedScripts(LABELS, quote = null, topInsetDp = 208f)

        assertTrue(
            "the editor's top inset must be applied once the bundle's hooks exist; sending it any " +
                "earlier is silently dropped and the body ends up under the header",
            scripts.any { it.startsWith("window.setComposerTopInset(") },
        )
        assertTrue(scripts.any { it.contains("208") })
    }

    @Test
    fun `the native chrome is switched on before the inset it sizes`() {
        val scripts = composerPageFinishedScripts(LABELS, quote = null, topInsetDp = 208f)

        val chrome = scripts.indexOfFirst { it.startsWith("window.useNativeComposerChrome(") }
        val inset = scripts.indexOfFirst { it.startsWith("window.setComposerTopInset(") }
        assertTrue("both hooks are sent", chrome >= 0 && inset >= 0)
        assertTrue(
            "the inset drives `body.native-chrome .editor` padding, so the class has to be on first",
            chrome < inset,
        )
    }

    @Test
    fun `an unmeasured header sends no inset rather than a zero one`() {
        // Belt and braces: the header is always measured before the page parses, but a 0 inset would
        // paint the body under the header again. Omit it, the LaunchedEffect still catches up when
        // the height lands.
        val scripts = composerPageFinishedScripts(LABELS, quote = null, topInsetDp = 0f)

        assertTrue(scripts.none { it.startsWith("window.setComposerTopInset(") })
    }

    @Test
    fun `labels and an optional quote ride the same batch`() {
        val plain = composerPageFinishedScripts(LABELS, quote = null, topInsetDp = 208f)
        assertTrue(plain.any { it.startsWith("window.setComposerLabels(") })
        assertTrue("no quote on a new message", plain.none { it.contains("setComposerQuote") })

        val replied = composerPageFinishedScripts(LABELS, quote = """{"blocks":[]}""", topInsetDp = 208f)
        assertEquals(plain.size + 1, replied.size)
        assertTrue(replied.any { it.startsWith("window.setComposerQuote(") })
    }

    /**
     * The signature seed is in the same batch for the same reason as everything else here, sent any
     * earlier it lands on an undefined `window.setComposerSignature` and the composer opens with no
     * signature at all, with nothing logged.
     */
    @Test
    fun `the signature seed rides the same batch, and only when there is one`() {
        val none = composerPageFinishedScripts(LABELS, quote = null, topInsetDp = 208f)
        assertTrue(
            "an account with both slots unassigned seeds nothing",
            none.none { it.contains("setComposerSignature") },
        )

        val seeded = composerPageFinishedScripts(
            LABELS,
            quote = null,
            topInsetDp = 208f,
            signature = """{"body_html":"<p>Alice</p>","body_plain":"Alice"}""",
        )
        assertEquals(none.size + 1, seeded.size)
        assertTrue(seeded.any { it.startsWith("window.setComposerSignature(") })
    }

    /**
     * Placement is decided on the FIRST insert, above the quoted original when there is one, and
     * reused on every later swap. So seeding the signature before the quote exists would put a
     * reply's signature underneath the message it is replying to, and no later swap would move it.
     */
    @Test
    fun `the signature is seeded after the quote it has to sit above`() {
        val scripts = composerPageFinishedScripts(
            LABELS,
            quote = """{"blocks":[]}""",
            topInsetDp = 208f,
            signature = """{"body_html":"<p>Alice</p>","body_plain":"Alice"}""",
        )

        val quote = scripts.indexOfFirst { it.startsWith("window.setComposerQuote(") }
        val signature = scripts.indexOfFirst { it.startsWith("window.setComposerSignature(") }
        assertTrue("both hooks are sent", quote >= 0 && signature >= 0)
        assertTrue("the signature goes above the quote, so the quote must exist first", quote < signature)
    }
}
