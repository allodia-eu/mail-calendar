// How Android hands the editor its page (docs/reading-dark-mode.md, rules 10 and 14). The page and
// its scripts are the core's, and nothing in this suite loads the native library, so these hold
// what is Android's: the page is in the document before the bundle runs, it is named again once the
// bundle has parsed, and an editor with no page host is left on the light page it starts on.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

private const val BUNDLE = """<!doctype html><html><head><meta charset="utf-8"><style>p{}</style>
  <script>window.bundle = true;</script></head><body><script>window.late = true;</script></body></html>"""

@RunWith(RobolectricTestRunner::class)
class ComposerPageTest {

    @Test
    fun `the page runs ahead of the bundle's own script`() {
        val html = composerHtmlWithPreload(BUNDLE, """window.composerPage = {"scheme":"dark"};""")

        val preload = html.indexOf("""window.composerPage = {"scheme":"dark"};""")
        val bundle = html.indexOf("window.bundle = true;")
        assertTrue("the preload is in the document", preload >= 0)
        assertTrue(
            "the bundle reads the page as it starts, so the preload has to have run by then",
            preload < bundle,
        )
        // Once, and as a script of its own: the bundle's own markup is otherwise untouched.
        assertEquals(BUNDLE, html.replaceFirst("""<script>window.composerPage = {"scheme":"dark"};</script>""" + "\n", ""))
    }

    @Test
    fun `nothing the preload carries can close its element early`() {
        val html = composerHtmlWithPreload(BUNDLE, """window.composerPage = {"css":"</script><b>"};""")

        assertTrue(html.contains("""{"css":"<\/script><b>"}"""))
        assertEquals(BUNDLE.split("</script>").size, html.split("</script>").size - 1)
    }

    @Test
    fun `a document without a script is left alone`() {
        assertEquals("<p>x</p>", composerHtmlWithPreload("<p>x</p>", "window.x = 1;"))
    }

    @Test
    fun `without a page host the editor is told only the app's scheme`() {
        // The JVM suite and a composer opened before the core was read: no page is named, so the
        // editor keeps its light page and never asks about colours.
        assertEquals("""window.composerScheme = "dark";""", composerPreloadScript(appDark = true, page = null))
        assertEquals("""window.composerScheme = "light";""", composerPreloadScript(appDark = false, page = null))
        assertEquals(composerSchemeScript(true), composerAppearanceScript(appDark = true, page = null))
    }

    @Test
    fun `the page is named on page-finished, after the scheme and before the body is seeded`() {
        val page = """window.setComposerPage({"scheme":"dark"})"""
        val scripts = composerPageFinishedScripts(
            """{"placeholder":"Write a message"}""",
            quote = "<p>quoted</p>",
            topInsetDp = 208f,
            dark = true,
            page = page,
        )

        val scheme = scripts.indexOf(composerSchemeScript(true))
        val named = scripts.indexOf(page)
        val quote = scripts.indexOfFirst { it.startsWith("window.setComposerQuote(") }
        assertTrue("the page is named", named >= 0)
        assertTrue("after the app's scheme, which the toggle's showing reads", scheme in 0 until named)
        assertTrue("before the quote, which then lands on the page already named", named < quote)
    }

    @Test
    fun `no page is named where there is none`() {
        val scripts = composerPageFinishedScripts("{}", quote = null, topInsetDp = 0f, dark = true)

        assertTrue(scripts.none { it.contains("setComposerPage") })
    }
}
