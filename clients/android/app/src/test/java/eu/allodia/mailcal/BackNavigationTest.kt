// The system back button, as one rule rather than a per-screen accident.
//
// The contract: back unwinds whatever is open, one level at a time, until the app is on the
// destination it was LAUNCHED on, and only a press made there closes the app. So the last press
// always leaves from the screen the user opened, never from a tab they wandered into.
//
// Two of these are worth stating because they are what was broken. First, a screen composed as an
// opaque overlay (the calendar's manager/detail/editor) is NOT a window: nothing hands it the back
// press, so without a handler the press fell straight through to the activity and closed the app
// from three levels deep. Second, "no handler" is the CORRECT answer on the home destination:
// the press has to reach the platform for it to run its own close animation, which is why these
// tests assert `hasEnabledCallbacks() == false` there rather than asserting some finish() call.
package eu.allodia.mailcal

import androidx.activity.compose.LocalOnBackPressedDispatcherOwner
import androidx.compose.material3.DrawerState
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.SetupRecommendation

@RunWith(RobolectricTestRunner::class)
class BackNavigationTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    /** Presses system back the way the OS does, through the activity's dispatcher. */
    private lateinit var back: () -> Unit

    /**
     * Whether ANY screen currently claims back, false means the press reaches the platform and
     * closes the app. Syncs first: `enabled` is applied in a SideEffect, and the v2 test rule
     * queues work rather than running it eagerly, so reading it raw can catch the frame before.
     */
    private fun backIsClaimed(): Boolean {
        compose.waitForIdle()
        return hasEnabledCallbacks()
    }

    private lateinit var hasEnabledCallbacks: () -> Boolean

    private fun pressBack() {
        compose.runOnUiThread { back() }
        compose.waitForIdle()
    }

    /** Captures the dispatcher hooks; call from inside `setContent`. */
    @androidx.compose.runtime.Composable
    private fun captureBack() {
        val dispatcher = LocalOnBackPressedDispatcherOwner.current!!.onBackPressedDispatcher
        back = { dispatcher.onBackPressed() }
        hasEnabledCallbacks = { dispatcher.hasEnabledCallbacks() }
    }

    // ---- The tab floor: back leads to the destination the app opened on -----------------------

    /** Renders the bottom-nav host on [destination], with [home] as the launch destination. */
    private fun navScaffold(
        destination: AppDestination,
        home: AppDestination,
        selected: MutableList<AppDestination>,
    ) {
        compose.setContent {
            AppTheme {
                captureBack()
                AppNavScaffold(
                    destination = destination,
                    home = home,
                    onSelect = { selected += it },
                ) { Text("content") }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun back_from_the_calendar_tab_returns_to_the_mailbox_rather_than_closing_the_app() {
        val selected = mutableListOf<AppDestination>()
        navScaffold(AppDestination.CALENDAR, home = AppDestination.MAIL, selected = selected)

        pressBack()

        assertEquals(listOf(AppDestination.MAIL), selected)
    }

    @Test
    fun back_from_the_contacts_tab_returns_to_the_mailbox_too() {
        val selected = mutableListOf<AppDestination>()
        navScaffold(AppDestination.CONTACTS, home = AppDestination.MAIL, selected = selected)

        pressBack()

        assertEquals(listOf(AppDestination.MAIL), selected)
    }

    /**
     * The end of the walk. On the launch destination the press must reach the PLATFORM, that is
     * what closes the app and runs its predictive-back animation, so nothing may claim it.
     */
    @Test
    fun back_on_the_home_destination_is_left_to_the_system() {
        val selected = mutableListOf<AppDestination>()
        navScaffold(AppDestination.MAIL, home = AppDestination.MAIL, selected = selected)

        assertFalse("the mailbox must not swallow the press that closes the app", backIsClaimed())
    }

    /**
     * "Home" is where the app OPENED, not the mail tab by definition: launched on the calendar,
     * back closes from the calendar rather than detouring through a mailbox the user never asked
     * for.
     */
    @Test
    fun a_launch_on_the_calendar_makes_the_calendar_the_floor() {
        navScaffold(AppDestination.CALENDAR, home = AppDestination.CALENDAR, selected = mutableListOf())

        assertFalse("the launch destination must not swallow the press", backIsClaimed())
    }

    /** ...and then it is the MAILBOX that leads back to it, the mirror of the usual case. */
    @Test
    fun with_the_calendar_as_home_the_mailbox_is_the_tab_that_backs_out() {
        val selected = mutableListOf<AppDestination>()
        navScaffold(AppDestination.MAIL, home = AppDestination.CALENDAR, selected = selected)

        pressBack()

        assertEquals(listOf(AppDestination.CALENDAR), selected)
    }

    // ---- The folder drawer -------------------------------------------------------------------

    /**
     * An open drawer is the first thing back should shut, and the case that failed hardest: on a
     * Galaxy Note 20 (Android 13) `ModalNavigationDrawer`'s own predictive-back callback never
     * fired, so the press went straight to the platform and closed the app with the drawer still
     * on screen. We register our own rather than trusting the library's.
     */
    @Test
    fun back_closes_the_folder_drawer_before_it_leaves_the_mailbox() {
        lateinit var drawerState: DrawerState
        compose.setContent {
            AppTheme {
                captureBack()
                drawerState = rememberDrawerState(DrawerValue.Open)
                FolderDrawerScaffold(
                    drawerState = drawerState,
                    accounts = emptyList(),
                    accountFolders = emptyList(),
                    selectedAccount = null,
                    selectedFolder = null,
                    unifiedUnread = 0u,
                    onSelectAccount = {},
                    onSelectFolder = { _, _ -> },
                    onSetExpanded = { _, _ -> },
                ) { Text("mailbox") }
            }
        }
        compose.waitForIdle()
        assertTrue("precondition: the drawer is open", drawerState.isOpen)

        pressBack()

        assertTrue("back must shut the drawer, not the app", drawerState.isClosed)
    }

    /** Shut, on the unified inbox, the mailbox claims nothing, so back closes the app. */
    @Test
    fun a_closed_drawer_on_the_unified_inbox_leaves_back_to_the_system() {
        mailbox(selectedAccount = null, selectedFolder = null)

        assertFalse(backIsClaimed())
    }

    /**
     * A folder is a step. The core opens every launch on the unified inbox (`selected_account`
     * starts null and is never persisted), so that view is the mailbox's home, and back returns
     * to it rather than closing the app from inside someone's Archive.
     */
    @Test
    fun back_in_a_folder_returns_to_the_unified_inbox() {
        val accounts = mutableListOf<String?>()
        val folders = mutableListOf<Pair<String, String>>()
        mailbox(
            selectedAccount = "work",
            selectedFolder = "archive",
            onSelectAccount = { accounts += it },
            onSelectFolder = { account, key -> folders += account to key },
        )

        pressBack()

        // One press, one intent: the unified list drops the folder with the account, because a
        // folder belongs to one (docs/folder-pane.md, rule 14). Selecting a folder is the only
        // thing that sets one, so back must not be dispatching one at all.
        assertEquals(listOf<String?>(null), accounts)
        assertEquals(emptyList<Pair<String, String>>(), folders)
    }

    /** An account with no folder chosen is the same step, its whole mailbox is still a narrowing. */
    @Test
    fun back_on_an_account_with_no_folder_also_returns_to_the_unified_inbox() {
        val accounts = mutableListOf<String?>()
        mailbox(selectedAccount = "work", selectedFolder = null, onSelectAccount = { accounts += it })

        pressBack()

        assertEquals(listOf<String?>(null), accounts)
    }

    /** The drawer wins over the folder step: shut it first, then leave the folder. */
    @Test
    fun an_open_drawer_shuts_before_the_folder_is_left() {
        lateinit var drawerState: DrawerState
        val accounts = mutableListOf<String?>()
        compose.setContent {
            AppTheme {
                captureBack()
                drawerState = rememberDrawerState(DrawerValue.Open)
                FolderDrawerScaffold(
                    drawerState = drawerState,
                    accounts = emptyList(),
                    accountFolders = emptyList(),
                    selectedAccount = "work",
                    selectedFolder = "archive",
                    unifiedUnread = 0u,
                    onSelectAccount = { accounts += it },
                    onSelectFolder = { _, _ -> },
                    onSetExpanded = { _, _ -> },
                ) { Text("mailbox") }
            }
        }
        compose.waitForIdle()

        pressBack()
        assertTrue("the drawer goes first", drawerState.isClosed)
        assertTrue("...and the folder is still selected", accounts.isEmpty())

        pressBack()
        assertEquals(listOf<String?>(null), accounts)
    }

    /** Renders the drawer scaffold over a stand-in mailbox. */
    private fun mailbox(
        selectedAccount: String?,
        selectedFolder: String?,
        onSelectAccount: (String?) -> Unit = {},
        onSelectFolder: (String, String) -> Unit = { _, _ -> },
    ) {
        compose.setContent {
            AppTheme {
                captureBack()
                FolderDrawerScaffold(
                    drawerState = rememberDrawerState(DrawerValue.Closed),
                    accounts = emptyList(),
                    accountFolders = emptyList(),
                    selectedAccount = selectedAccount,
                    selectedFolder = selectedFolder,
                    unifiedUnread = 0u,
                    onSelectAccount = onSelectAccount,
                    onSelectFolder = onSelectFolder,
                    onSetExpanded = { _, _ -> },
                ) { Text("mailbox") }
            }
        }
        compose.waitForIdle()
    }

    // ---- Diagnostics: two screens deep inside Settings ----------------------------------------

    @Test
    fun back_walks_the_log_viewer_out_to_settings_one_screen_at_a_time() {
        var left = 0
        compose.setContent {
            AppTheme {
                captureBack()
                DiagnosticsScreen(onSetLogLevel = {}, onBack = { left++ })
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText(L10n.diagnostics_view_log(ctx())).performClick()
        // The viewer is showing (its own title replaces the diagnostics one).
        compose.onNodeWithText(L10n.settings_category_diagnostics(ctx())).assertDoesNotExist()

        pressBack()
        assertEquals("leaving the viewer must not leave diagnostics", 0, left)
        compose.onNodeWithText(L10n.settings_category_diagnostics(ctx())).assertExists()

        pressBack()
        assertEquals(1, left)
    }

    // ---- Account setup: a flow with phases, reached both on first run and over a running app ---

    /** Renders the email-first setup flow; [onCancel] null means the first run (nothing to cancel to). */
    private fun setup(onCancel: (() -> Unit)?) {
        compose.setContent {
            AppTheme {
                captureBack()
                AccountSetupFlow(
                    externalError = null,
                    onCancel = onCancel,
                    signingIn = false,
                    connecting = false,
                    detect = { SetupRecommendation.Manual(MissReason.NOTHING_FOUND) },
                    onSignInMicrosoft = {},
                    onConnect = { null },
                    onConnectJmap = { null },
                )
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun back_out_of_the_manual_form_returns_to_the_email_question() {
        setup(onCancel = {})
        compose.onNodeWithText(L10n.setup_detect_manual(ctx())).performClick()
        compose.onNodeWithText(L10n.setup_title(ctx())).assertExists()

        pressBack()

        compose.onNodeWithText(L10n.setup_detect_title(ctx())).assertExists()
    }

    @Test
    fun back_at_the_email_question_cancels_out_when_adding_another_account() {
        var cancelled = 0
        setup(onCancel = { cancelled++ })

        pressBack()

        assertEquals(1, cancelled)
    }

    /**
     * The first run is the one place setup IS the launch screen: there is no running app behind
     * it, so back must close the app rather than dead-end on a form that cannot be cancelled.
     */
    @Test
    fun back_on_the_first_run_setup_is_left_to_the_system() {
        setup(onCancel = null)

        assertFalse("first-run setup is the root screen", backIsClaimed())
    }

    /**
     * Even on a first run the flow's own phases are still steps: the manual form goes back to the
     * email question, and only THERE does the press reach the platform.
     */
    @Test
    fun the_first_run_still_steps_back_through_its_own_phases() {
        setup(onCancel = null)
        compose.onNodeWithText(L10n.setup_detect_manual(ctx())).performClick()
        assertTrue("the manual form is a step, not the root", backIsClaimed())

        pressBack()

        compose.onNodeWithText(L10n.setup_detect_title(ctx())).assertExists()
        assertFalse(backIsClaimed())
    }
}
