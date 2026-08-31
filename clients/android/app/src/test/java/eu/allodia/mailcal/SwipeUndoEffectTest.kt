// The Compose half of swipe-undo: SwipeUndoEffect must turn a real Snackbar's outcome into exactly
// one commit or one revert, and must not drop a swipe when its coroutine is cancelled.
//
// Driven through a real SnackbarHost rather than a fake, so the Material3 contract we depend on
// (tapping the action resumes with ActionPerformed; a dismissal resumes with Dismissed) is the thing
// under test. `SnackbarData.dismiss()` is exactly what the 4s timeout calls, so the timeout path is
// covered without any clock manipulation.
package eu.allodia.mailcal

import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.SwipeActionKind

@RunWith(RobolectricTestRunner::class)
class SwipeUndoEffectTest {
    @get:Rule val compose = createComposeRule()

    private val committed = mutableListOf<PendingSwipe>()
    private val reverted = mutableListOf<PendingSwipe>()

    private fun swipe(id: Long, key: String = "m1") =
        PendingSwipe(id, "acct-1", key, SwipeActionKind.DELETE)

    /**
     * Hosts the effect over a real SnackbarHost. The pending swipe is snapshot state created OUTSIDE
     * composition, so the test can drive it and the effect re-keys, a `mutableStateOf` inside
     * `setContent` would be rebuilt on every recomposition and never change.
     */
    private fun host(initial: PendingSwipe?): MutableState<PendingSwipe?> {
        val pending = mutableStateOf(initial)
        compose.setContent {
            val hostState = remember { SnackbarHostState() }
            SwipeUndoEffect(
                pending = pending.value,
                snackbarHostState = hostState,
                onCommit = { committed += it },
                onRevert = { reverted += it },
            )
            SnackbarHost(hostState)
        }
        return pending
    }

    @Test
    fun tapping_undo_reverts_and_never_commits() {
        host(swipe(1))
        compose.waitForIdle()

        compose.onNodeWithText("Undo").performClick()
        compose.waitForIdle()

        assertEquals(listOf(swipe(1)), reverted)
        assertEquals(emptyList<PendingSwipe>(), committed)
    }

    @Test
    fun the_snackbar_shows_the_action_that_ran() {
        host(PendingSwipe(1, "acct-1", "m1", SwipeActionKind.ARCHIVE))
        compose.waitForIdle()

        // Past tense: by the time it shows, the row has already left the list.
        compose.onNodeWithText("Archived").assertIsDisplayed()
    }

    @Test
    fun letting_the_snackbar_dismiss_commits_the_action() {
        lateinit var hostState: SnackbarHostState
        compose.setContent {
            hostState = remember { SnackbarHostState() }
            SwipeUndoEffect(
                pending = swipe(1),
                snackbarHostState = hostState,
                onCommit = { committed += it },
                onRevert = { reverted += it },
            )
            SnackbarHost(hostState)
        }
        compose.waitForIdle()

        // What the 4-second timeout does internally, without waiting four seconds.
        compose.runOnUiThread { hostState.currentSnackbarData!!.dismiss() }
        compose.waitForIdle()

        assertEquals(listOf(swipe(1)), committed)
        assertEquals(emptyList<PendingSwipe>(), reverted)
    }

    @Test
    fun a_newer_swipe_cancels_the_effect_and_the_older_one_still_commits() {
        val pending = host(swipe(1))
        compose.waitForIdle()

        // The user swipes a second message before the first Snackbar settles.
        compose.runOnUiThread { pending.value = swipe(2, key = "m2") }
        compose.waitForIdle()

        // The cancelled coroutine's `finally` committed the first, a swipe is never silently lost.
        assertEquals(listOf(swipe(1)), committed)
        assertEquals(emptyList<PendingSwipe>(), reverted)
    }

    @Test
    fun clearing_the_pending_swipe_commits_it() {
        val pending = host(swipe(1))
        compose.waitForIdle()

        compose.runOnUiThread { pending.value = null }
        compose.waitForIdle()

        assertEquals(listOf(swipe(1)), committed)
    }

    @Test
    fun a_null_pending_swipe_does_nothing() {
        host(null)
        compose.waitForIdle()

        assertEquals(emptyList<PendingSwipe>(), committed)
        assertEquals(emptyList<PendingSwipe>(), reverted)
    }
}
