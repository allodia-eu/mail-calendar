// The swipe undo state machine, tested without a UI. These are the rules a user feels as "my mail
// didn't get deleted when I hit Undo" and "swiping two messages in a row deletes both", the parts
// that were only reachable by hand on a device before this suite existed.
//
// Plain JUnit: SwipeUndoController holds Compose snapshot state, but snapshot state is pure JVM, so
// no Robolectric and no composition is needed here.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.SwipeActionKind

private const val ACCOUNT = "acct-1"
private const val KEY = "imap:v1:u1@INBOX"

/** Every dispatch the controller made, in order, as `"verb account key[ arg]"`. */
private class Dispatches {
    val calls = mutableListOf<String>()

    fun controller() = SwipeUndoController(
        onDelete = { account, key -> calls += "delete $account $key" },
        onArchive = { account, key -> calls += "archive $account $key" },
        onSetFlagged = { account, key, flagged -> calls += "flag $account $key $flagged" },
    )
}

private fun flatRow(account: String = ACCOUNT, key: String = KEY): SnapshotRow =
    SnapshotRow.Flat(
        FlatRow(
            account = account,
            key = key,
            subject = "Subject",
            from = "someone@remote.test",
            avatar = stubAvatar(),
            date = "2026-07-10T11:34:41Z",
            unread = false,
            flagged = false,
            hasAttachment = false,
            preview = "",
        ),
    )

class SwipeUndoControllerTest {

    @Test
    fun a_delete_swipe_hides_the_row_but_dispatches_nothing_yet() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()

        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)

        // The whole point of deferring: the row leaves the list, but the server hasn't been told.
        assertTrue(controller.isHidden(flatRow()))
        assertEquals(emptyList<String>(), dispatches.calls)
        assertEquals(SwipeActionKind.DELETE, controller.pending?.action)
    }

    @Test
    fun committing_a_delete_dispatches_it_once_and_keeps_the_row_hidden() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)

        controller.commit(controller.pending!!)

        assertEquals(listOf("delete $ACCOUNT $KEY"), dispatches.calls)
        // Still hidden: the core's own optimistic removal has not reached us yet, and un-hiding now
        // would flash the row back before the next snapshot arrives.
        assertTrue(controller.isHidden(flatRow()))
        assertNull(controller.pending)
    }

    @Test
    fun committing_an_archive_dispatches_archive_not_delete() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.ARCHIVE)

        controller.commit(controller.pending!!)

        assertEquals(listOf("archive $ACCOUNT $KEY"), dispatches.calls)
    }

    @Test
    fun reverting_a_delete_restores_the_row_and_never_dispatches() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)

        controller.revert(controller.pending!!)

        // Undo is exact because nothing was ever sent, there is no move to reverse.
        assertEquals(emptyList<String>(), dispatches.calls)
        assertFalse(controller.isHidden(flatRow()))
        assertNull(controller.pending)
    }

    @Test
    fun releasing_the_hide_after_a_commit_lets_a_core_rejected_edit_reappear() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)
        val swipe = controller.pending!!
        controller.commit(swipe)

        controller.releaseHide(swipe)

        // If the core applied the edit the row is gone from the snapshot anyway; if it REJECTED the
        // edit it restored the row, and we must stop hiding it or it stays invisible until restart.
        assertFalse(controller.isHidden(flatRow()))
    }

    @Test
    fun a_star_swipe_applies_immediately_and_leaves_the_row_in_place() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()

        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.STAR)

        // Deferring a star would look broken: the row stays on screen with no star for 4 seconds.
        assertEquals(listOf("flag $ACCOUNT $KEY true"), dispatches.calls)
        assertFalse(controller.isHidden(flatRow()))
    }

    @Test
    fun committing_a_star_is_a_no_op_because_it_already_applied() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.STAR)

        controller.commit(controller.pending!!)

        assertEquals(listOf("flag $ACCOUNT $KEY true"), dispatches.calls)
    }

    @Test
    fun reverting_a_star_un_stars_it() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.STAR)

        controller.revert(controller.pending!!)

        assertEquals(
            listOf("flag $ACCOUNT $KEY true", "flag $ACCOUNT $KEY false"),
            dispatches.calls,
        )
    }

    @Test
    fun a_second_swipe_supersedes_the_first_and_the_first_still_commits() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, "first", SwipeActionKind.DELETE)
        val first = controller.pending!!

        // The user swipes another message before the first Snackbar times out. Compose cancels the
        // first undo coroutine, whose `finally` commits it, while `pending` already names the
        // second swipe.
        controller.onSwipe(ACCOUNT, "second", SwipeActionKind.DELETE)
        val second = controller.pending!!
        controller.commit(first)

        assertEquals(listOf("delete $ACCOUNT first"), dispatches.calls)
        // The late commit must NOT clear the newer swipe, or the second Snackbar never runs and the
        // second message is silently never deleted.
        assertEquals(second, controller.pending)
        assertTrue(controller.isHidden(flatRow(key = "second")))
    }

    @Test
    fun a_late_revert_does_not_clear_a_newer_pending_swipe_either() {
        val dispatches = Dispatches()
        val controller = dispatches.controller()
        controller.onSwipe(ACCOUNT, "first", SwipeActionKind.DELETE)
        val first = controller.pending!!
        controller.onSwipe(ACCOUNT, "second", SwipeActionKind.DELETE)
        val second = controller.pending!!

        controller.revert(first)

        assertEquals(second, controller.pending)
        assertFalse(controller.isHidden(flatRow(key = "first")))
        assertTrue(controller.isHidden(flatRow(key = "second")))
    }

    @Test
    fun the_same_message_swiped_twice_produces_distinct_swipes() {
        val controller = Dispatches().controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)
        val first = controller.pending!!
        controller.revert(first)
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)
        val second = controller.pending!!

        // Equal ids would leave `LaunchedEffect(pending)` un-re-keyed, so the second swipe's
        // Snackbar would never show and its delete would never commit.
        assertTrue(first.id != second.id)
        assertTrue(first != second)
    }

    @Test
    fun hiding_is_scoped_to_the_owning_account() {
        val controller = Dispatches().controller()
        controller.onSwipe(ACCOUNT, KEY, SwipeActionKind.DELETE)

        // A provider key is unique only WITHIN an account, so the unified inbox can show the same
        // key under two accounts. Hiding one must not blank the other.
        assertTrue(controller.isHidden(flatRow(account = ACCOUNT, key = KEY)))
        assertFalse(controller.isHidden(flatRow(account = "acct-2", key = KEY)))
    }

    @Test
    fun visible_rows_drop_only_the_hidden_flat_rows() {
        val controller = Dispatches().controller()
        val rows = listOf(flatRow(key = "a"), flatRow(key = "b"), flatRow(key = "c"))
        controller.onSwipe(ACCOUNT, "b", SwipeActionKind.DELETE)

        assertEquals(listOf(flatRow(key = "a"), flatRow(key = "c")), controller.visibleRows(rows))
    }

    @Test
    fun visible_rows_returns_the_input_untouched_when_nothing_is_hidden() {
        val controller = Dispatches().controller()
        val rows = listOf(flatRow(key = "a"), flatRow(key = "b"))

        assertEquals(rows, controller.visibleRows(rows))
    }
}
