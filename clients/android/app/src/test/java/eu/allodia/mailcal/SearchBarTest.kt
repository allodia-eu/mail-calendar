// The search chrome: what it tells the core, what it tells the user, and how you get out of it.
//
// Three things here were real reports, not hypotheticals. The system back gesture used to leave
// the app with the search still running in the core, so returning showed stale results with no
// search field in sight ("leaving search doesn't restore my inbox"). Clearing the query resets the
// scope in the core, so a filter that kept claiming "this folder" would have been lying. And the
// "this folder" label means three different things depending on what the list was showing.
package eu.allodia.mailcal

import android.content.Context
import androidx.activity.compose.LocalOnBackPressedDispatcherOwner
import androidx.compose.ui.test.junit4.v2.createComposeRule
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.FolderRole
import uniffi.mailcal_bindings.FolderRow
import uniffi.mailcal_bindings.SearchHorizon
import uniffi.mailcal_bindings.SearchScope

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private fun folders() = listOf(
    AccountFolderRow(
        accountId = "work",
        folders = listOf(
            FolderRow(key = "inbox-key", name = "Inbox", role = FolderRole.INBOX, unread = 0u),
            FolderRow(key = "arch-key", name = "Archief", role = FolderRole.ARCHIVE, unread = 0u),
        ),
    ),
)

@RunWith(RobolectricTestRunner::class)
class SearchBarTest {
    @get:Rule val compose = createComposeRule()

    /** Everything the state dispatched at the core, in order. */
    private val queries = mutableListOf<String?>()
    private val scopes = mutableListOf<SearchScope>()

    private fun state() = SearchBarState(
        onSearch = { queries += it },
        onSetScope = { scopes += it },
    )

    @Test
    fun a_keystroke_does_not_search_on_its_own_but_emptying_the_field_clears_at_once() {
        val state = state()
        state.type("report")
        // Typing alone dispatches nothing, SearchField debounces it. A search costs the core a
        // full-text query per account plus a store read per hit, so one per letter stacked seven
        // of them to type "monitor".
        assertEquals(emptyList<String?>(), queries)
        assertEquals("report", state.query)

        // A field with nothing in it is not a search for "", it is no search, and it takes
        // effect immediately rather than after a pause.
        state.type("")
        assertEquals(listOf<String?>(null), queries)
    }

    @Test
    fun a_burst_of_typing_costs_one_search_once_it_settles() {
        val state = state()
        state.openSearch()
        compose.mainClock.autoAdvance = false
        compose.setContent { SearchField(state = state) }

        // Three letters in quick succession: each cancels the pending dispatch.
        listOf("r", "re", "rep").forEach { text ->
            compose.runOnUiThread { state.type(text) }
            compose.mainClock.advanceTimeBy(50)
        }
        assertEquals(emptyList<String?>(), queries)

        // Typing settles: exactly one search, for what is actually in the field.
        compose.mainClock.advanceTimeBy(400)
        assertEquals(listOf<String?>("rep"), queries)
    }

    @Test
    fun coming_back_to_the_field_does_not_re_run_a_search_the_core_already_applies() {
        val state = state()
        state.openSearch()
        state.type("report")
        state.commitQuery()
        assertEquals(listOf<String?>("report"), queries)

        // The debounce effect is keyed on the query, so it re-arms every time SearchField enters
        // composition, and the state outlives that screen (MainActivity holds it). Returning from
        // a message must not cost a second full-text query per account.
        state.commitQuery()
        assertEquals(listOf<String?>("report"), queries)

        // A query that actually moved is still a search.
        state.type("reports")
        state.commitQuery()
        assertEquals(listOf<String?>("report", "reports"), queries)
    }

    @Test
    fun clearing_and_retyping_the_same_query_searches_again() {
        val state = state()
        state.openSearch()
        state.type("report")
        state.commitQuery()
        state.clearQuery()
        state.type("report")
        state.commitQuery()

        // The core dropped the search when the field emptied, so asking for it again is not a
        // repeat, it is the only way the results come back.
        assertEquals(listOf<String?>("report", null, "report"), queries)
    }

    @Test
    fun clearing_the_query_widens_the_scope_back_because_the_core_does_too() {
        val state = state()
        state.openSearch()
        state.type("report")
        state.select(SearchScope.CURRENT_FOLDER)
        assertEquals(listOf(SearchScope.CURRENT_FOLDER), scopes)

        // The core resets the scope on any cleared query. If the filter kept showing
        // CURRENT_FOLDER, the next thing typed would search everything while the control said
        // otherwise, the filter has to follow in the same action.
        state.clearQuery()
        assertEquals(SearchScope.ALL_FOLDERS, state.scope)
        assertTrue(state.open)
    }

    @Test
    fun closing_leaves_search_entirely_and_tells_the_core_to_restore_the_folder_view() {
        val state = state()
        state.openSearch()
        state.type("report")
        state.select(SearchScope.CURRENT_FOLDER)

        state.close()
        assertFalse(state.open)
        assertEquals("", state.query)
        assertEquals(SearchScope.ALL_FOLDERS, state.scope)
        // The trailing `null` is what makes the core drop the search and re-project the list the
        // user came from.
        assertEquals(null, queries.last())
    }

    @Test
    fun the_system_back_gesture_leaves_search_rather_than_the_app() {
        val state = state()
        state.openSearch()
        state.type("report")
        lateinit var back: () -> Unit
        compose.setContent {
            val dispatcher = LocalOnBackPressedDispatcherOwner.current!!.onBackPressedDispatcher
            back = { dispatcher.onBackPressed() }
            SearchField(state = state)
        }
        compose.waitForIdle()

        compose.runOnUiThread { back() }
        compose.waitForIdle()

        // Without the handler this back press fell through to the activity: the app went to the
        // background with the core still searching, and coming back showed the stale results.
        assertFalse(state.open)
        assertEquals(null, queries.last())
    }

    @Test
    fun back_is_left_alone_when_the_search_field_is_closed() {
        val state = state()
        lateinit var dispatcherHasCallback: () -> Boolean
        compose.setContent {
            val dispatcher = LocalOnBackPressedDispatcherOwner.current!!.onBackPressedDispatcher
            dispatcherHasCallback = { dispatcher.hasEnabledCallbacks() }
            SearchField(state = state)
        }
        compose.waitForIdle()

        // Nothing to leave, so back stays the activity's, the mailbox must not swallow it.
        assertFalse(dispatcherHasCallback())
    }

    @Test
    fun the_current_scope_label_names_what_the_list_was_showing() {
        // The unified view: the filter narrows to the inboxes it shows, not to "a folder".
        assertEquals("Inboxes", currentScopeLabel(ctx(), folders(), null, null))
        // One account, no folder, its whole mailbox.
        assertEquals("This account", currentScopeLabel(ctx(), folders(), "work", null))
        // One folder: its own name, as the drawer shows it (server-named, not translated).
        assertEquals("Archief", currentScopeLabel(ctx(), folders(), "work", "arch-key"))
        // A folder the snapshot no longer lists still gets a truthful, generic label.
        assertEquals("This folder", currentScopeLabel(ctx(), folders(), "work", "gone"))
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_scope_labels_are_translated() {
        assertEquals("Postvakken IN", currentScopeLabel(ctx(), folders(), null, null))
        assertEquals("Dit account", currentScopeLabel(ctx(), folders(), "work", null))
        assertEquals("Deze map", currentScopeLabel(ctx(), folders(), "work", "gone"))
    }

    @Test
    fun the_horizon_says_how_far_back_the_results_reach() {
        // The month count comes from the core, so the line and the sync-depth setting can never
        // disagree about what this device holds.
        assertEquals(
            "Searching the last 3 months",
            searchHorizonLabel(ctx(), SearchHorizon.Months(3u)),
        )
        assertEquals("Searching all mail", searchHorizonLabel(ctx(), SearchHorizon.AllTime))
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_horizon_is_translated() {
        assertEquals(
            "Zoeken in de laatste 6 maanden",
            searchHorizonLabel(ctx(), SearchHorizon.Months(6u)),
        )
        assertEquals("Zoeken in alle berichten", searchHorizonLabel(ctx(), SearchHorizon.AllTime))
    }
}
