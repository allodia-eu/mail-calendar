// Opening a mail link (`mailto:`) in the composer. Two client-side decisions are covered here;
// the URI *parsing* is the shared core's and is held by its own Rust tests (nothing in this
// suite loads the cdylib).
//
// 1. Which incoming Intents count as a mail link. The trap is that every OAuth redirect also
//    arrives as ACTION_VIEW, so an action-only check would swallow a sign-in mid-account-setup.
// 2. Whether the composer reveals its Cc/Bcc row. A `mailto:` link may legally set a Bcc, so
//    left collapsed it would add a recipient the user never sees and cannot remove.
package eu.allodia.mailcal

import android.content.Intent
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class MailtoLaunchTest {

    @Test
    fun `a tapped mail link and a send-to request both count`() {
        // ACTION_VIEW is a link in a browser or document; ACTION_SENDTO is another app (or the
        // system chooser) asking for a mail client. The manifest declares both, so both arrive.
        assertTrue(MailtoLaunch.carriesMailLink(Intent.ACTION_VIEW, "mailto"))
        assertTrue(MailtoLaunch.carriesMailLink(Intent.ACTION_SENDTO, "mailto"))
        // Schemes are case-insensitive per RFC 3986, and senders do capitalize them.
        assertTrue(MailtoLaunch.carriesMailLink(Intent.ACTION_VIEW, "MAILTO"))
    }

    @Test
    fun `an oauth redirect is not a mail link`() {
        // The regression this guards. All three sign-in redirects come back as ACTION_VIEW on the
        // same singleTask activity; matching on the action alone would route them into the
        // composer and strand an account halfway through being added.
        assertFalse(MailtoLaunch.carriesMailLink(Intent.ACTION_VIEW, "msauth"))
        // Google's is a reversed client id, and the real one is whatever this build was given
        // (BUILDING.md), the shape is what matters here, not the project it belongs to.
        assertFalse(
            MailtoLaunch.carriesMailLink(
                Intent.ACTION_VIEW,
                "com.googleusercontent.apps.1234567890-abcdef",
            ),
        )
        assertFalse(MailtoLaunch.carriesMailLink(Intent.ACTION_VIEW, "eu.allodia.mailcal"))
    }

    @Test
    fun `an unrelated launch is not a mail link`() {
        assertFalse(MailtoLaunch.carriesMailLink(Intent.ACTION_MAIN, null))
        assertFalse(MailtoLaunch.carriesMailLink(Intent.ACTION_VIEW, "https"))
        assertFalse(MailtoLaunch.carriesMailLink(null, "mailto"))
        // ACTION_SEND is share-a-file/text, a different feature we do not claim.
        assertFalse(MailtoLaunch.carriesMailLink(Intent.ACTION_SEND, "mailto"))
    }

    @Test
    fun `a pre-filled bcc opens the composer's cc-bcc row`() {
        // The security-relevant half. RFC 6068 lets a link set a Bcc, and the composer collapses
        // that row by default, so a link could add a silent recipient that the user neither sees
        // before sending nor knows to look for.
        assertTrue(revealsCcBcc(cc = "", bcc = "snoop@evil.test"))
        assertTrue(revealsCcBcc(cc = "carol@example.test", bcc = ""))
        assertTrue(revealsCcBcc(cc = "carol@example.test", bcc = "snoop@evil.test"))
    }

    @Test
    fun `a plain compose leaves the cc-bcc row tucked away`() {
        assertFalse(revealsCcBcc(cc = "", bcc = ""))
        assertFalse("whitespace is not an address", revealsCcBcc(cc = "  ", bcc = " "))
    }

    @Test
    fun `a mail link's body is seeded as text once the editor has parsed`() {
        // Same silent-failure class as the top inset: a window.* hook called before the bundle
        // parses lands on an undefined function and evaluateJavascript reports nothing, so the
        // body would simply arrive empty with no error anywhere.
        val scripts = composerPageFinishedScripts(
            labelsJson = """{"placeholder":"Write a message"}""",
            quote = null,
            topInsetDp = 208f,
            body = "Please quote order #123",
        )

        assertTrue(scripts.any { it.startsWith("window.setPlainText(") })
        // The body is JSON-quoted into the call, so a quote or backslash in it cannot close the
        // string and run as script.
        assertTrue(scripts.any { it.contains("\"Please quote order #123\"") })
    }

    /**
     * And it is seeded BEFORE the signature. `setPlainText` assigns the whole body, so a signature
     * seeded first is simply erased, the user's sign-off would vanish from a message a link
     * opened, with nothing to show it had ever been there.
     */
    @Test
    fun `a mail link's body is seeded before the signature it must not erase`() {
        val scripts = composerPageFinishedScripts(
            labelsJson = """{"placeholder":"Write a message"}""",
            quote = null,
            topInsetDp = 208f,
            signature = """{"blocks":[]}""",
            body = "Please quote order #123",
        )

        val body = scripts.indexOfFirst { it.startsWith("window.setPlainText(") }
        val signature = scripts.indexOfFirst { it.startsWith("window.setComposerSignature(") }
        assertTrue("both must be sent", body >= 0 && signature >= 0)
        assertTrue("the body must not overwrite the signature", body < signature)
    }

    @Test
    fun `a composer with no pre-filled body sends no seeding call`() {
        val scripts = composerPageFinishedScripts(
            labelsJson = """{"placeholder":"Write a message"}""",
            quote = null,
            topInsetDp = 208f,
        )

        assertTrue(scripts.none { it.contains("setPlainText") })
    }
}
