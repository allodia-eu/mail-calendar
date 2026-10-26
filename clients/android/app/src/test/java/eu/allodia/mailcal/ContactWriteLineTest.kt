// What the contacts list says about the last write, for every status: a save that did not happen
// and one that could not be confirmed are two different sentences, and neither reads as saved.
package eu.allodia.mailcal

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ContactWriteStatus

@RunWith(RobolectricTestRunner::class)
class ContactWriteLineTest {
    private fun ctx(): Context = ApplicationProvider.getApplicationContext()

    @Test
    fun every_status_maps_to_its_line() {
        val expected = mapOf(
            ContactWriteStatus.IDLE to null,
            ContactWriteStatus.SAVING to L10n.contacts_saving(ctx()),
            ContactWriteStatus.SAVED to L10n.contacts_saved(ctx()),
            ContactWriteStatus.NOT_SAVED to L10n.contacts_save_failed(ctx()),
            ContactWriteStatus.UNCONFIRMED to L10n.contacts_save_unconfirmed(ctx()),
            // Stated under the form, never beside the list.
            ContactWriteStatus.INVALID to null,
        )
        assertEquals(ContactWriteStatus.entries.toSet(), expected.keys)
        for ((status, line) in expected) {
            assertEquals(status.name, line, contactWriteLine(ctx(), status))
        }
    }

    @Test
    fun the_english_failures_say_what_happened() {
        assertEquals(
            "Couldn't save this contact",
            contactWriteLine(ctx(), ContactWriteStatus.NOT_SAVED),
        )
        assertEquals(
            "Couldn't confirm this was saved",
            contactWriteLine(ctx(), ContactWriteStatus.UNCONFIRMED),
        )
    }
}
