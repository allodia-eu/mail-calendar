// One swipe on a mail row parks exactly ONE action.
//
// This is the regression test for the Compose 1.9+ behaviour change that the 2026.06.01 BOM bump
// brought in. The row used to dispatch its action from `rememberSwipeToDismissBoxState`'s
// `confirmValueChange` and return false to veto the state change; Compose now consults that
// callback repeatedly *during* the drag rather than once at settle, so a single swipe dispatched
// the configured action eight times, eight Trash intents, eight undo entries. Only the calendar
// agenda's equivalent test caught it, by luck; the mail row, where the blast radius is real mail,
// had no swipe test at all.
//
// The row now dispatches from `SwipeToDismissBox`'s `onDismiss`, which fires once per settle:
// but only as long as the lambda handed to it is a single remembered instance, because the box
// keys a `LaunchedEffect` on it. Passing a fresh lambda per recomposition re-arms that effect and
// re-parks the same swipe. Both halves of that contract are what this test pins.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.MutableState
import androidx.compose.runtime.State
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeRight
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings

private const val ACCOUNT = "acct-1"
private const val KEY = "imap:v1:u1@INBOX"
private const val SUBJECT = "Quarterly report"

// Two zones on the same offset: swapping between them recomposes the row without changing a
// single character it renders, which is exactly what the recomposition test needs.
private const val ZONE = "Europe/Amsterdam"
private const val SAME_OFFSET_ZONE = "Europe/Brussels"

private fun flatRow() = FlatRow(
    account = ACCOUNT,
    key = KEY,
    subject = SUBJECT,
    from = "someone@remote.test",
    avatar = stubAvatar(),
    date = "2026-07-10T11:34:41Z",
    unread = false,
    flagged = false,
    hasAttachment = false,
    preview = "",
)

// The row under test, with everything it needs that this file does not care about stubbed out.
@androidx.compose.runtime.Composable
private fun TestSwipeRow(
    swipe: SwipeSettings,
    zone: String,
    onSwipe: (key: String, action: SwipeActionKind) -> Unit,
) {
    SwipeableFlatMessageRow(
        message = flatRow(),
        activeZoneId = zone,
        inJunkFolder = false,
        swipe = swipe,
        onSwipe = { _, key, action -> onSwipe(key, action) },
        accounts = emptyList(),
        selected = false,
        selecting = false,
        onToggleSelect = {},
        onOpen = {},
        onSetRead = { _, _, _ -> },
        onSetFlagged = { _, _, _ -> },
        onDelete = { _, _ -> },
        onPermanentlyDelete = { _, _ -> },
        onMarkAsSpam = { _, _ -> },
        onMarkAsNotSpam = { _, _ -> },
        onReply = { _, _, _, _, _, _ -> true },
        onForward = { _, _, _, _, _, _ -> true },
        replyRecipients = { _, _, _ -> null },
    )
}

@RunWith(RobolectricTestRunner::class)
class MailRowSwipeTest {
    @get:Rule val compose = createComposeRule()

    /** Every action the row parked, in order. */
    private val parked = mutableListOf<Pair<String, SwipeActionKind>>()

    /**
     * Renders one swipeable row with [swipe] bound to its two directions.
     *
     * [zone] is a live state read purely so a test can force the row to recompose: flipping it
     * changes a parameter, so the composable cannot be skipped. Nothing else about the row
     * depends on which of the two equivalent zones is current.
     */
    private fun row(swipe: SwipeSettings, zone: State<String> = mutableStateOf(ZONE)) {
        compose.setContent {
            AppTheme {
                Box {
                    TestSwipeRow(swipe, zone.value) { key, action -> parked += key to action }
                }
            }
        }
    }

    /**
     * Renders the row the way the mailbox does: one keyed [LazyColumn] item that the parked swipe
     * removes, and an Undo ([hidden] back to false) that puts it back.
     */
    private fun hidingRow(swipe: SwipeSettings, hidden: MutableState<Boolean>) {
        compose.setContent {
            AppTheme {
                LazyColumn {
                    if (!hidden.value) {
                        item(key = KEY) {
                            TestSwipeRow(swipe, ZONE) { key, action ->
                                parked += key to action
                                hidden.value = true
                            }
                        }
                    }
                }
            }
        }
    }

    @Test
    fun a_rightward_swipe_parks_its_action_exactly_once() {
        row(SwipeSettings(left = SwipeActionKind.ARCHIVE, right = SwipeActionKind.DELETE))

        compose.onNodeWithText(SUBJECT).performTouchInput { swipeRight() }
        compose.waitForIdle()

        // Exactly one, the eight-dispatch regression is a list of eight identical entries.
        assertEquals(listOf(KEY to SwipeActionKind.DELETE), parked)
    }

    @Test
    fun a_leftward_swipe_parks_the_other_direction_exactly_once() {
        row(SwipeSettings(left = SwipeActionKind.ARCHIVE, right = SwipeActionKind.DELETE))

        compose.onNodeWithText(SUBJECT).performTouchInput { swipeLeft() }
        compose.waitForIdle()

        assertEquals(listOf(KEY to SwipeActionKind.ARCHIVE), parked)
    }

    @Test
    fun the_row_settles_back_after_a_swipe_that_leaves_it_in_place() {
        // Star does not remove the row, so the box has to animate home or that row is stuck
        // off-screen at its dismissed anchor and can never be swiped again.
        row(SwipeSettings(left = SwipeActionKind.ARCHIVE, right = SwipeActionKind.DELETE))

        compose.onNodeWithText(SUBJECT).performTouchInput { swipeRight() }
        compose.waitForIdle()

        compose.onNodeWithText(SUBJECT).assertExists()
        // A second swipe is still possible, which it would not be if the box were stuck away:
        // SwipeToDismissBox disables its gestures unless the state is settled.
        compose.onNodeWithText(SUBJECT).performTouchInput { swipeRight() }
        compose.waitForIdle()
        assertEquals(
            listOf(KEY to SwipeActionKind.DELETE, KEY to SwipeActionKind.DELETE),
            parked,
        )
    }

    @Test
    fun a_recomposition_while_the_row_is_settled_away_does_not_re_park_the_swipe() {
        // The subtle half of the contract. `SwipeToDismissBox` runs `onDismiss` from a
        // `LaunchedEffect(state.settledValue, onDismiss)`, so a lambda rebuilt on each
        // recomposition re-keys that effect; while the box is still settled away, the window
        // between the dismiss landing and `reset()` finishing, that re-fires the action.
        //
        // In the real app that window is busy: parking a swipe flips the undo controller's
        // snapshot state, which hides the row and raises the Snackbar. So the clock is taken away
        // from Compose (per AGENTS.md) to land a recomposition inside it deliberately, rather than
        // hoping one arrives.
        val zone = mutableStateOf(ZONE)
        compose.mainClock.autoAdvance = false
        row(SwipeSettings(left = SwipeActionKind.ARCHIVE, right = SwipeActionKind.DELETE), zone)

        compose.onNodeWithText(SUBJECT).performTouchInput { swipeRight() }

        // Advance frame by frame only until the dismiss has landed, so the reset animation is
        // still in flight and `settledValue` is still the dismissed anchor.
        var frames = 0
        while (parked.isEmpty() && frames++ < 300) {
            compose.mainClock.advanceTimeByFrame()
        }
        assertEquals("the swipe should have parked once by now", 1, parked.size)

        // Recompose repeatedly inside that window.
        repeat(5) { i ->
            zone.value = if (i % 2 == 0) SAME_OFFSET_ZONE else ZONE
            compose.mainClock.advanceTimeByFrame()
        }

        assertEquals("a recomposition must not re-park the swipe", 1, parked.size)
    }

    @Test
    fun an_undo_that_puts_the_row_back_does_not_park_the_swipe_again() {
        // What the user sees when this breaks: Undo appears to do nothing. The row never returns,
        // and the Snackbar flickers and starts over, because putting the row back re-parks the
        // same swipe.
        //
        // The screen hides the row the instant a swipe parks, so the row leaves the composition
        // while `reset()` is still in flight, and LazyColumn saves the SwipeToDismissBox state
        // under the item key, so Undo restores a box still settled at its dismissed anchor.
        val hidden = mutableStateOf(false)
        hidingRow(SwipeSettings(left = SwipeActionKind.ARCHIVE, right = SwipeActionKind.DELETE), hidden)

        compose.onNodeWithText(SUBJECT).performTouchInput { swipeRight() }
        compose.waitForIdle()
        assertEquals(listOf(KEY to SwipeActionKind.DELETE), parked)

        // Undo.
        hidden.value = false
        compose.waitForIdle()

        assertEquals("an undo must not re-park the swipe", listOf(KEY to SwipeActionKind.DELETE), parked)
        // And the row is actually back on screen, not parked off it.
        compose.onNodeWithText(SUBJECT).assertIsDisplayed()
    }
}
