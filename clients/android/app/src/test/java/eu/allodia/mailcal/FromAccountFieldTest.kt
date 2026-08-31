// The composer's From dropdown. The account it opens on decides which mailbox a message actually
// leaves from (the core sends as, and through, that account), so "which one is selected" is a
// correctness question, not a cosmetic one.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountRow

private val ALICE = AccountRow("acct-1", "alice@test.local", expanded = true)
private val BOB = AccountRow("acct-2", "bob@test.local", expanded = true)

@RunWith(RobolectricTestRunner::class)
class FromAccountFieldTest {
    @get:Rule val compose = createComposeRule()

    /** Renders the field over `accounts`, tracking the selection the way the composer does. */
    private fun field(accounts: List<AccountRow>, initial: AccountRow?): () -> AccountRow? {
        var selected by mutableStateOf(initial)
        compose.setContent {
            FromAccountField(accounts = accounts, selected = selected, onSelect = { selected = it })
        }
        return { selected }
    }

    @Test
    fun it_shows_the_selected_account() {
        field(listOf(ALICE, BOB), ALICE)

        compose.onNodeWithText("alice@test.local").assertIsDisplayed()
    }

    @Test
    fun opening_it_lists_every_configured_account() {
        field(listOf(ALICE, BOB), ALICE)

        compose.onNodeWithText("alice@test.local").performClick()
        compose.waitForIdle()

        // Both are offered; the user can send from either mailbox.
        compose.onNodeWithText("bob@test.local").assertIsDisplayed()
    }

    @Test
    fun picking_another_account_selects_it() {
        val selected = field(listOf(ALICE, BOB), ALICE)

        compose.onNodeWithText("alice@test.local").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("bob@test.local").performClick()
        compose.waitForIdle()

        assertEquals(BOB, selected())
    }

    @Test
    fun a_single_account_does_not_open_a_menu_onto_one_item() {
        field(listOf(ALICE), ALICE)

        compose.onNodeWithText("alice@test.local").performClick()
        compose.waitForIdle()

        // Exactly one node carries the address: the field itself. An opened menu would add a
        // second, identical item, a menu onto a single choice. (The field stays visible either
        // way; the From address is never hidden.)
        val nodes = compose.onAllNodesWithText("alice@test.local").fetchSemanticsNodes().size
        assertEquals("nodes carrying the address", 1, nodes)
    }
}
