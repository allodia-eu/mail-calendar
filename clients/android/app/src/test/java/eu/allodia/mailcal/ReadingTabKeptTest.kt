// An open message is the Mail tab's (MainActivityMailboxTab.kt, docs/reading-actions.md): the
// navigation bar stays under it, a visit to another tab leaves it open to come back to, and system
// back closes it before it leaves the tab.
//
// The activity cannot be composed here (it loads the cdylib), so the screens are stand-ins, picked
// by the activity's own decision (`tabBody`) inside the real scaffold, with the open message held
// outside it as the activity holds it.
package eu.allodia.mailcal

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalOnBackPressedDispatcherOwner
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingTabKeptTest {
    @get:Rule val rule = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private lateinit var back: () -> Unit

    private fun show(home: AppDestination = AppDestination.MAIL) {
        var destination by mutableStateOf(AppDestination.MAIL)
        var opened by mutableStateOf<String?>("Quarterly report")
        rule.setContent {
            AppTheme {
                val dispatcher = LocalOnBackPressedDispatcherOwner.current!!.onBackPressedDispatcher
                back = { dispatcher.onBackPressed() }
                AppNavScaffold(destination = destination, home = home, onSelect = { destination = it }) {
                    when (tabBody(destination, opened != null)) {
                        TabBody.CALENDAR -> Text("calendar grid")
                        TabBody.CONTACTS -> Text("contacts list")
                        TabBody.READING -> {
                            BackHandler { opened = null }
                            Text("reading $opened")
                        }
                        TabBody.MAILBOX -> Text("mailbox list")
                    }
                }
            }
        }
    }

    @Test
    fun anOpenMessageIsTheMailTabsAlone() {
        assertEquals(TabBody.READING, tabBody(AppDestination.MAIL, messageOpen = true))
        assertEquals(TabBody.MAILBOX, tabBody(AppDestination.MAIL, messageOpen = false))
        assertEquals(TabBody.CALENDAR, tabBody(AppDestination.CALENDAR, messageOpen = true))
        assertEquals(TabBody.CONTACTS, tabBody(AppDestination.CONTACTS, messageOpen = true))
    }

    @Test
    fun anAccountRemovedWhileItsMessageWaitsTakesTheMessageWithIt() {
        val opened = OpenedMessage("acct-1", "m1", "Quarterly report", "Ada", stubAvatar(), "")
        assertTrue(outlivedItsAccount(opened, listOf("acct-2")))
        assertFalse(outlivedItsAccount(opened, listOf("acct-1", "acct-2")))
        // A mailbox not read yet is not a mailbox without accounts.
        assertFalse(outlivedItsAccount(opened, emptyList()))
        assertFalse(outlivedItsAccount(null, listOf("acct-2")))
    }

    @Test
    fun theNavigationBarStaysWhileAMessageIsOpen() {
        show()
        rule.onNodeWithText("reading Quarterly report").assertIsDisplayed()
        rule.onNodeWithText(L10n.nav_calendar(ctx)).assertIsDisplayed()
        rule.onNodeWithText(L10n.nav_contacts(ctx)).assertIsDisplayed()
    }

    @Test
    fun mailReturnsToTheOpenMessageAfterAVisitToTheCalendar() {
        show()
        rule.onNodeWithText(L10n.nav_calendar(ctx)).performClick()
        rule.onNodeWithText("calendar grid").assertIsDisplayed()
        rule.onNodeWithText(L10n.nav_mail(ctx)).performClick()
        rule.onNodeWithText("reading Quarterly report").assertIsDisplayed()
    }

    @Test
    fun backClosesTheMessageBeforeItLeavesTheTab() {
        // Calendar as home, so the scaffold's own handler is armed on Mail: the message's must win.
        show(home = AppDestination.CALENDAR)
        rule.runOnUiThread { back() }
        rule.onNodeWithText("mailbox list").assertIsDisplayed()
        rule.runOnUiThread { back() }
        rule.onNodeWithText("calendar grid").assertIsDisplayed()
    }
}
