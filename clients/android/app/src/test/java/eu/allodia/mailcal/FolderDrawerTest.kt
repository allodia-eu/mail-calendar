// The folder drawer's rules (docs/folder-pane.md), and the two that used to fail silently.
//
// Expansion was a `remember(accounts, selectedAccount)` seeded from the selection, so it reset on
// every recomposition that touched either one: opening a folder in one account shut the account
// beside it, and a relaunch shut them all. It is the core's now, and the core persists it, so
// what this file really pins is that the drawer never invents the state itself.
//
// The badge half is the same shape of bug from the other side: a count that renders at zero shows
// "0 unread" on every folder nobody has mail in, and a provider that reports no count at all
// (Gmail today) would show a confident zero for a folder it never looked at.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.rememberDrawerState
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.FolderRole
import uniffi.mailcal_bindings.FolderRow

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private fun folder(key: String, name: String, role: FolderRole?, unread: UInt = 0u) =
    FolderRow(key = key, name = name, role = role, unread = unread)

@RunWith(RobolectricTestRunner::class)
class FolderDrawerTest {
    @get:Rule val compose = createComposeRule()

    private val expandToggles = mutableListOf<Pair<String, Boolean>>()
    private val selectedAccounts = mutableListOf<String?>()
    private val selectedFolders = mutableListOf<Pair<String, String>>()

    private fun drawer(
        accounts: List<AccountRow>,
        accountFolders: List<AccountFolderRow>,
        selectedAccount: String? = null,
        unifiedUnread: UInt = 0u,
    ) {
        compose.setContent {
            FolderDrawerScaffold(
                drawerState = rememberDrawerState(DrawerValue.Open),
                accounts = accounts,
                accountFolders = accountFolders,
                selectedAccount = selectedAccount,
                selectedFolder = null,
                unifiedUnread = unifiedUnread,
                onSelectAccount = { selectedAccounts.add(it) },
                onSelectFolder = { account, key -> selectedFolders.add(account to key) },
                onSetExpanded = { id, expanded -> expandToggles.add(id to expanded) },
                content = {},
            )
        }
    }

    private fun account(id: String, email: String, expanded: Boolean) =
        AccountRow(id = id, email = email, expanded = expanded)

    @Test
    fun `every expanded account shows its folders regardless of which is selected`() {
        drawer(
            accounts = listOf(
                account("work", "me@work.example", expanded = true),
                account("home", "me@home.example", expanded = true),
            ),
            // Folders the user made, so each keeps a name of its own to assert on: a folder with a
            // role is named by the app (rule 12), and both accounts' inboxes would read "Inbox".
            accountFolders = listOf(
                AccountFolderRow("work", listOf(folder("w1", "Tenders", role = null))),
                AccountFolderRow("home", listOf(folder("h1", "Receipts", role = null))),
            ),
            // "work" is selected; "home" must still be showing its folders.
            selectedAccount = "work",
        )

        compose.onNodeWithText("Tenders").assertIsDisplayed()
        compose.onNodeWithText("Receipts").assertIsDisplayed()
    }

    @Test
    fun `a known folder takes the app's name and a folder you made keeps yours`() {
        drawer(
            accounts = listOf(account("work", "me@work.example", expanded = true)),
            accountFolders = listOf(
                AccountFolderRow(
                    "work",
                    listOf(
                        // What a server really calls them: the one name IMAP mandates, and
                        // Exchange's word for the Trash.
                        folder("w1", "INBOX", FolderRole.INBOX),
                        folder("w2", "Deleted Items", FolderRole.TRASH),
                        folder("w3", "Tenders", role = null),
                    ),
                ),
            ),
        )

        compose.onNodeWithText(L10n.folder_inbox(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.folder_trash(ctx())).assertIsDisplayed()
        compose.onNodeWithText("INBOX").assertDoesNotExist()
        compose.onNodeWithText("Deleted Items").assertDoesNotExist()
        // Only a role is renamed, a folder the user made keeps the name they gave it.
        compose.onNodeWithText("Tenders").assertIsDisplayed()
    }

    @Test
    fun `a collapsed account hides its folders even while it is the selected one`() {
        drawer(
            accounts = listOf(account("work", "me@work.example", expanded = false)),
            accountFolders = listOf(
                AccountFolderRow("work", listOf(folder("w1", "Tenders", role = null))),
            ),
            selectedAccount = "work",
        )

        // Under the old rule this was impossible: being selected WAS being expanded. The folder
        // carries no role, so the name asserted on is one the drawer would really have drawn:
        // against a renamed folder this assertion could not fail.
        compose.onNodeWithText("Tenders").assertDoesNotExist()
    }

    @Test
    fun `the chevron reports the toggle to the core and navigates nowhere`() {
        drawer(
            accounts = listOf(account("work", "me@work.example", expanded = true)),
            accountFolders = listOf(
                AccountFolderRow("work", listOf(folder("w1", "Work Inbox", FolderRole.INBOX))),
            ),
        )

        compose.onNodeWithContentDescription(L10n.a11y_collapse_account(ctx())).performClick()

        assertEquals(listOf("work" to false), expandToggles)
        // Expanding is not navigating: the selection must not have moved.
        assertTrue(selectedAccounts.isEmpty())
        assertTrue(selectedFolders.isEmpty())
    }

    @Test
    fun `the account row itself still selects the account`() {
        drawer(
            accounts = listOf(account("work", "me@work.example", expanded = true)),
            accountFolders = listOf(AccountFolderRow("work", emptyList())),
        )

        compose.onNodeWithText("me@work.example").performClick()

        assertEquals(listOf<String?>("work"), selectedAccounts)
        // …and did not toggle the tree on the way past.
        assertTrue(expandToggles.isEmpty())
    }

    @Test
    fun `a folder tap names the account whose tree it is in, not the selected one`() {
        // Both accounts key their folder `archive`, as every provider does, so the key alone
        // says nothing about which mailbox to open (docs/folder-pane.md, rule 14). The names
        // differ only so the test can click one of them.
        drawer(
            accounts = listOf(
                account("work", "me@work.example", expanded = true),
                account("home", "me@home.example", expanded = true),
            ),
            accountFolders = listOf(
                AccountFolderRow("work", listOf(folder("archive", "Work Filing", role = null))),
                AccountFolderRow("home", listOf(folder("archive", "Home Filing", role = null))),
            ),
            // "work" is the selected account, and the tap is on the OTHER one's folder, which
            // is exactly the case a bare key gets wrong.
            selectedAccount = "work",
        )

        compose.onNodeWithText("Home Filing").performClick()

        assertEquals(listOf("home" to "archive"), selectedFolders)
        // One intent, not an account followed by a folder: two would race in the core, and the
        // account handler clears the folder the other one just set.
        assertTrue(selectedAccounts.isEmpty())
    }

    @Test
    fun `a count shows on the folder and on all inboxes but never at zero`() {
        // Two accounts, so each number on screen belongs to exactly one row, and so the All
        // Inboxes badge is the sum it claims to be (rule 7) rather than a repeat of the only
        // count in the fixture.
        drawer(
            accounts = listOf(
                account("work", "me@work.example", expanded = true),
                account("home", "me@home.example", expanded = true),
            ),
            accountFolders = listOf(
                AccountFolderRow(
                    "work",
                    listOf(
                        folder("w1", "INBOX", FolderRole.INBOX, unread = 4u),
                        // Counted and empty, and never counted at all: both show nothing.
                        folder("w2", "Sent Items", FolderRole.SENT, unread = 0u),
                    ),
                ),
                AccountFolderRow(
                    "home",
                    listOf(folder("h1", "INBOX", FolderRole.INBOX, unread = 1u)),
                ),
            ),
            unifiedUnread = 5u,
        )

        compose.onNodeWithText("4").assertIsDisplayed()
        compose.onNodeWithText("1").assertIsDisplayed()
        compose.onNodeWithText("5").assertIsDisplayed()
        // The count is announced as a sentence, not as a bare number beside a folder name.
        compose.onNodeWithContentDescription(L10n.a11y_unread_count(ctx(), 4)).assertExists()
        compose.onNodeWithText("0").assertDoesNotExist()
    }
}
