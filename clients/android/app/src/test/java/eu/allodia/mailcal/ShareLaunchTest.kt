// Sharing a file into the app ("share this by email" from another app). What is covered here is
// the client-side half only: which Intents count as a share, and what payload comes off one. What
// a payload *means*, the names, the media types, the cap and the refusals, is the shared core's
// and is held by its own Rust tests, so nothing in this suite loads the cdylib.
//
// docs/os-integration.md; docs/composer-security.md, Gate 13.
package eu.allodia.mailcal

import android.content.Intent
import android.net.Uri
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShareLaunchTest {

    private fun uri(value: String): Uri = Uri.parse(value)

    @Test
    fun `both send actions count as a share`() {
        // ACTION_SEND carries one item, ACTION_SEND_MULTIPLE several. The manifest declares both,
        // so both arrive, and Android dispatches them separately.
        assertTrue(ShareLaunch.carriesShare(Intent.ACTION_SEND))
        assertTrue(ShareLaunch.carriesShare(Intent.ACTION_SEND_MULTIPLE))
    }

    @Test
    fun `nothing else is a share`() {
        // The mail-link actions above all: a `mailto:` is not a file, and a share may never
        // pre-fill a recipient, so a link reaching the share path would be a real divergence.
        assertFalse(ShareLaunch.carriesShare(Intent.ACTION_VIEW))
        assertFalse(ShareLaunch.carriesShare(Intent.ACTION_SENDTO))
        assertFalse(ShareLaunch.carriesShare(Intent.ACTION_MAIN))
        assertFalse(ShareLaunch.carriesShare(null))
    }

    @Test
    fun `a mail link stays a mail link and not a share`() {
        // The two gates are asked in order (MainActivityBoot.routeNewIntent), and this is what
        // makes that order safe: ACTION_SENDTO on a `mailto:` is a mail link to one and not a
        // share to the other, whichever way round they were asked.
        assertTrue(MailtoLaunch.carriesMailLink(Intent.ACTION_SENDTO, "mailto"))
        assertFalse(ShareLaunch.carriesShare(Intent.ACTION_SENDTO))
    }

    @Test
    fun `a single shared item is read off EXTRA_STREAM`() {
        val intent = Intent(Intent.ACTION_SEND).putExtra(
            Intent.EXTRA_STREAM,
            uri("content://media/external/images/1"),
        )
        assertEquals(listOf(uri("content://media/external/images/1")), ShareLaunch.sharedUris(intent))
    }

    @Test
    fun `several shared items keep the order the sender gave them`() {
        val uris = arrayListOf(uri("content://x/1"), uri("content://x/2"), uri("content://x/3"))
        val intent = Intent(Intent.ACTION_SEND_MULTIPLE)
            .putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)
        assertEquals(uris.toList(), ShareLaunch.sharedUris(intent))
    }

    @Test
    fun `an item named the wrong way round is still read`() {
        // A sender may put a single item in the list extra, or a list under ACTION_SEND, and some
        // do. Reading both is what stops a share that used the "wrong" one from silently
        // attaching nothing.
        val single = Intent(Intent.ACTION_SEND_MULTIPLE)
            .putExtra(Intent.EXTRA_STREAM, uri("content://x/1"))
        assertEquals(listOf(uri("content://x/1")), ShareLaunch.sharedUris(single))

        val many = Intent(Intent.ACTION_SEND)
            .putParcelableArrayListExtra(Intent.EXTRA_STREAM, arrayListOf(uri("content://x/2")))
        assertEquals(listOf(uri("content://x/2")), ShareLaunch.sharedUris(many))
    }

    @Test
    fun `one item offered under both extras is attached once`() {
        val intent = Intent(Intent.ACTION_SEND)
            .putExtra(Intent.EXTRA_STREAM, uri("content://x/1"))
            .putParcelableArrayListExtra(Intent.EXTRA_STREAM, arrayListOf(uri("content://x/1")))
        assertEquals(listOf(uri("content://x/1")), ShareLaunch.sharedUris(intent))
    }

    @Test
    fun `a share carrying no files is empty rather than an error`() {
        // A text-only share (a URL from a browser) reaches the same path; the core turns the text
        // into a body.
        val intent = Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_TEXT, "https://allodia.eu")
        assertTrue(ShareLaunch.sharedUris(intent).isEmpty())
        assertEquals("https://allodia.eu", ShareLaunch.sharedText(intent))
    }

    @Test
    fun `text and subject come across when the sender offers them`() {
        val intent = Intent(Intent.ACTION_SEND)
            .putExtra(Intent.EXTRA_TEXT, "Notes from today.")
            .putExtra(Intent.EXTRA_SUBJECT, "Board meeting")
        assertEquals("Notes from today.", ShareLaunch.sharedText(intent))
        assertEquals("Board meeting", ShareLaunch.sharedSubject(intent))
    }

    @Test
    fun `absent text and subject are blank rather than null`() {
        val intent = Intent(Intent.ACTION_SEND)
        assertEquals("", ShareLaunch.sharedText(intent))
        assertEquals("", ShareLaunch.sharedSubject(intent))
    }

    @Test
    fun `a CharSequence extra is read as text`() {
        // `EXTRA_TEXT` is a CharSequence, and senders do put styled spans in it; reading it as a
        // String would come back null and lose the whole body.
        val styled: CharSequence = android.text.SpannableString("Quarterly figures")
        val intent = Intent(Intent.ACTION_SEND).putExtra(Intent.EXTRA_TEXT, styled)
        assertEquals("Quarterly figures", ShareLaunch.sharedText(intent))
    }
}
