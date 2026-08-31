// The share intent the Diagnostics screen hands to the system share sheet.
//
// Robolectric, because FileProvider resolves the authority and its path roots from the merged
// manifest, so this also pins that res/xml/file_paths.xml actually exposes files/logs/:
// getUriForFile throws IllegalArgumentException for a file outside a declared root, which is
// exactly the misconfiguration that would otherwise only surface as a crash on a user's phone
// the moment they tap Share.
package eu.allodia.mailcal

import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.core.content.IntentCompat
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class LogShareIntentTest {

    private val ctx: Context get() = RuntimeEnvironment.getApplication()

    // The log file exactly where FileLog.init puts it: <filesDir>/logs/app.log, the path the
    // files-path root in file_paths.xml must cover.
    private fun logFile(): File =
        File(File(ctx.filesDir, "logs").apply { mkdirs() }, "app.log")
            .apply { writeText("2026-07-16 10:00:00.000 INFO [test] one line\n") }

    // One test method, deliberately: FileProvider caches its parsed path roots in a static map
    // keyed by authority, and that static survives across Robolectric tests while filesDir does
    // not, a second getUriForFile in this class would be checked against the FIRST test's
    // (deleted) roots and fail for a reason that has nothing to do with the intent under test.
    @Test
    fun the_share_is_a_plain_text_send_of_a_readable_fileprovider_stream() {
        val intent = buildLogShareIntent(ctx, logFile())

        assertEquals(Intent.ACTION_SEND, intent.action)
        assertEquals("text/plain", intent.type)

        val uri = IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
        assertNotNull(uri)
        assertEquals("${ctx.packageName}.fileprovider", uri!!.authority)
        // Read-only is all the share target gets; without the flag it would see a
        // SecurityException instead of the file.
        assertTrue(intent.flags and Intent.FLAG_GRANT_READ_URI_PERMISSION != 0)
    }
}
