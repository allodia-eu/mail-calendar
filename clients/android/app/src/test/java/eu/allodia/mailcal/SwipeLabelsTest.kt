// The swipe action labels. A swapped mapping here is a lie told to the user at the worst moment:
// the Snackbar saying "Archived" while the message went to Trash, so each arm is pinned.
package eu.allodia.mailcal

import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.SwipeActionKind

private fun ctx(): Context = RuntimeEnvironment.getApplication()

@RunWith(RobolectricTestRunner::class)
class SwipeLabelsTest {

    @Test
    fun the_settings_picker_names_each_action() {
        assertEquals("Move to Trash", swipeActionLabel(ctx(), SwipeActionKind.DELETE))
        assertEquals("Archive", swipeActionLabel(ctx(), SwipeActionKind.ARCHIVE))
        assertEquals("Star", swipeActionLabel(ctx(), SwipeActionKind.STAR))
    }

    @Test
    fun the_snackbar_reports_what_actually_happened_in_the_past_tense() {
        assertEquals("Moved to Trash", swipeDoneLabel(ctx(), SwipeActionKind.DELETE))
        assertEquals("Archived", swipeDoneLabel(ctx(), SwipeActionKind.ARCHIVE))
        assertEquals("Starred", swipeDoneLabel(ctx(), SwipeActionKind.STAR))
    }

    @Test
    @Config(qualifiers = "nl")
    fun both_label_sets_are_translated() {
        assertEquals("Naar prullenbak", swipeActionLabel(ctx(), SwipeActionKind.DELETE))
        assertEquals("Gearchiveerd", swipeDoneLabel(ctx(), SwipeActionKind.ARCHIVE))
    }
}
