// The composer's From dropdown. The account it opens on decides which mailbox a message actually
// leaves from (the core sends as, and through, that account), so "which one is selected" is a
// correctness question, not a cosmetic one. It also decides what a sender is shown about that
// choice: the label is `Name <address>`, the whole of what the recipient will see.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertDoesNotExist
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

private val ALICE = AccountRow("acct-1", "alice@test.local", name = "Alice Tester", expanded = true)
private val BOB = AccountRow("acct-2", "bob@test.local", name = "", expanded = true)

/**
 * What the composer hands the field, standing in for the core's `senderLabel`: this suite loads no
 * cdylib, so the real one cannot be called here. The formatting rule itself is tested in Rust
 * (`mailcal-viewmodel`); what is tested below is that the field draws what it is given, on the
 * selected account and on every item in the menu.
 */
private fun label(account: AccountRow): String =
    if (account.name.isEmpty()) account.email else "${account.name} <${account.email}>"

@RunWith(RobolectricTestRunner::class)
class FromAccountFieldTest {
    @get:Rule val compose = createComposeRule()

    /** Renders the field over `accounts`, tracking the selection the way the composer does. */
    private fun field(accounts: List<AccountRow>, initial: AccountRow?): () -> AccountRow? {
        var selected by mutableStateOf(initial)
        compose.setContent {
            FromAccountField(
                accounts = accounts,
                selected = selected,
                onSelect = { selected = it },
                accountLabel = ::label,
            )
        }
        return { selected }
    }

    @Test
    fun it_shows_the_selected_account() {
        field(listOf(ALICE, BOB), ALICE)

        compose.onNodeWithText(label(ALICE)).assertIsDisplayed()
    }

    @Test
    fun opening_it_lists_every_configured_account() {
        field(listOf(ALICE, BOB), ALICE)

        compose.onNodeWithText(label(ALICE)).performClick()
        compose.waitForIdle()

        // Both are offered; the user can send from either mailbox.
        compose.onNodeWithText(label(BOB)).assertIsDisplayed()
    }

    @Test
    fun picking_another_account_selects_it() {
        val selected = field(listOf(ALICE, BOB), ALICE)

        compose.onNodeWithText(label(ALICE)).performClick()
        compose.waitForIdle()
        compose.onNodeWithText(label(BOB)).performClick()
        compose.waitForIdle()

        assertEquals(BOB, selected())
    }

    @Test
    fun the_field_reads_as_the_recipient_will_read_it() {
        field(listOf(ALICE, BOB), ALICE)

        // Name and address together: the From is where a sender picks who a message comes from,
        // so it shows the whole of what arrives rather than half of it (docs/sending.md rule 7).
        compose.onNodeWithText("Alice Tester <alice@test.local>").assertIsDisplayed()
        // And the bare address is not what is drawn. onNodeWithText matches exactly, so this
        // fails only if the field fell back to the address it used to show.
        compose.onNodeWithText("alice@test.local").assertDoesNotExist()
    }

    @Test
    fun an_account_with_no_name_reads_as_its_address_alone() {
        field(listOf(BOB), BOB)

        // The first-run state, and a real answer: an account nobody has named sends as a bare
        // address, and the field says so rather than inventing one (docs/sending.md rule 3).
        compose.onNodeWithText("bob@test.local").assertIsDisplayed()
    }

    @Test
    fun a_single_account_does_not_open_a_menu_onto_one_item() {
        field(listOf(ALICE), ALICE)

        compose.onNodeWithText(label(ALICE)).performClick()
        compose.waitForIdle()

        // Exactly one node carries the address: the field itself. An opened menu would add a
        // second, identical item, a menu onto a single choice. (The field stays visible either
        // way; the From address is never hidden.)
        val nodes = compose.onAllNodesWithText(label(ALICE)).fetchSemanticsNodes().size
        assertEquals("nodes carrying the address", 1, nodes)
    }
}
