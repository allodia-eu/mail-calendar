// The mailbox search chrome: the expanding search field, the scope filter under it, and the state
// tying them to the core. Split out of MailboxScreen.kt so the behaviour can be tested without
// composing the whole screen (AGENTS.md: put the logic in a plain class, not a knot of remembers).
//
// The core owns the search, the query, the scope, and the results. This owns only how they are
// offered, and one invariant: **what the filter shows is what the core is applying**. The core
// resets the scope whenever the query clears, so every path that clears the query here resets the
// chip in the same action. Otherwise the field empties, the core silently widens back to all mail,
// and the filter goes on claiming the search is narrowed to one folder.
package eu.allodia.mailcal

import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.SearchHorizon
import uniffi.mailcal_bindings.SearchScope

// How long typing has to pause before the query is dispatched. A search is not free, the core
// runs a full-text query per account, resolves every hit from the store, and re-projects the list,
// which on a real multi-account device is roughly a second. Dispatching per keystroke stacked
// seven of those to type "monitor", each fighting the others for the same store. One per pause
// costs the user 250 ms and the device one search.
private const val SEARCH_DEBOUNCE_MS = 250L

// The search chrome's state: whether the field is showing, what is typed, and which scope the
// filter offers. `onSearch` and `onSetScope` dispatch to the core; every transition that empties
// the query resets the scope too, because the core does the same on its side.
internal class SearchBarState(
    private val onSearch: (query: String?) -> Unit,
    private val onSetScope: (SearchScope) -> Unit,
) {
    var open by mutableStateOf(false)
        private set
    var query by mutableStateOf("")
        private set
    var scope by mutableStateOf(SearchScope.ALL_FOLDERS)
        private set

    // What the core was last asked for. The debounce effect in [SearchField] is keyed on the
    // query, so it re-arms every time the field enters composition, and this state outlives the
    // mailbox screen (MainActivity holds it): without this, coming back from a message would
    // re-run a search the core is already applying.
    private var dispatched: String? = null

    /** The magnifier was tapped: reveal the field (the query and scope start clean). */
    fun openSearch() {
        open = true
    }

    /** A keystroke. A blank field is not a search, so it clears rather than querying for "".
     *  A non-blank one is *not* dispatched here, [SearchField] debounces it (see
     *  [commitQuery]), so a burst of typing costs one search rather than one per letter. */
    fun type(text: String) {
        query = text
        if (text.isBlank()) clearQuery()
    }

    /** Dispatch what is currently typed, called once typing has settled. */
    fun commitQuery() {
        if (query.isBlank() || query == dispatched) return
        dispatched = query
        onSearch(query)
    }

    /** The field's clear (×) button: empty the query but stay in search, ready for the next one. */
    fun clearQuery() {
        query = ""
        scope = SearchScope.ALL_FOLDERS
        dispatched = null
        onSearch(null)
    }

    /** Leave search entirely, the back arrow and the system back gesture alike. */
    fun close() {
        open = false
        clearQuery()
    }

    /** The scope filter was moved. */
    fun select(chosen: SearchScope) {
        scope = chosen
        onSetScope(chosen)
    }
}

// Names the scope the user was standing in when they opened search, the narrowing half of the
// filter. The unified view shows every account's inbox, an account with no folder selected shows
// its whole mailbox, and otherwise it is one named folder (the server's own name, as the drawer
// shows it; a folder the snapshot no longer lists falls back to the generic "This folder").
internal fun currentScopeLabel(
    ctx: Context,
    accountFolders: List<AccountFolderRow>,
    selectedAccount: String?,
    selectedFolder: String?,
): String = when {
    selectedAccount == null -> L10n.search_scope_inboxes(ctx)
    selectedFolder == null -> L10n.search_scope_account(ctx)
    else -> accountFolders
        .firstOrNull { it.accountId == selectedAccount }
        ?.folders
        ?.firstOrNull { it.key == selectedFolder }
        ?.name
        ?: L10n.search_scope_folder(ctx)
}

// The expanded search field: a back arrow out of search, the input, and a clear button. Sits in
// the top bar in place of the account switcher while `state.open`.
//
// The back arrow is not the only way out: `BackHandler` claims the system back gesture too. Without
// it, back backgrounded the app while the core kept searching, so returning showed stale results
// with no search field in sight, the "leaving search doesn't restore my inbox" report.
@Composable
internal fun SearchField(state: SearchBarState, modifier: Modifier = Modifier) {
    val ctx = LocalContext.current
    // Focus the field the moment search opens, so the keyboard comes up without a second tap.
    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(state.open) {
        if (state.open) focusRequester.requestFocus()
    }
    // Debounced dispatch: each keystroke re-keys this effect, cancelling the pending one, so the
    // core is asked once, when the typing stops. Clearing and closing stay immediate (they are
    // not queries, and a user who leaves search should not watch it think first).
    LaunchedEffect(state.query) {
        if (state.query.isBlank()) return@LaunchedEffect
        delay(SEARCH_DEBOUNCE_MS)
        state.commitQuery()
    }
    BackHandler(enabled = state.open) { state.close() }
    Row(modifier = modifier, verticalAlignment = Alignment.CenterVertically) {
        IconButton(onClick = state::close) {
            Icon(
                painter = painterResource(R.drawable.ic_arrow_back),
                contentDescription = L10n.action_close(ctx),
            )
        }
        OutlinedTextField(
            value = state.query,
            onValueChange = state::type,
            modifier = Modifier
                .weight(1f)
                .focusRequester(focusRequester),
            singleLine = true,
            placeholder = { Text(L10n.search_placeholder(ctx)) },
            trailingIcon = {
                if (state.query.isNotBlank()) {
                    IconButton(onClick = state::clearQuery) {
                        Icon(
                            painter = painterResource(R.drawable.ic_close),
                            contentDescription = L10n.search_clear(ctx),
                        )
                    }
                }
            },
        )
    }
}

// The scope toggle, shown under the field while searching. Two choices, never zero, the core
// always has a scope, so one side is always selected and this cannot render an "off" state that
// does not exist.
@Composable
internal fun SearchScopeFilter(
    state: SearchBarState,
    currentLabel: String,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    SingleChoiceSegmentedButtonRow(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 4.dp),
    ) {
        SegmentedButton(
            selected = state.scope == SearchScope.CURRENT_FOLDER,
            onClick = { state.select(SearchScope.CURRENT_FOLDER) },
            shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
        ) { Text(currentLabel, maxLines = 1) }
        SegmentedButton(
            selected = state.scope == SearchScope.ALL_FOLDERS,
            onClick = { state.select(SearchScope.ALL_FOLDERS) },
            shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
        ) { Text(L10n.search_scope_all(ctx), maxLines = 1) }
    }
}

// How far back the results reach, the sync depth of the accounts the scope searched. Search
// reads what is on the device and nothing else, so an empty result means "not in the last three
// months" far more often than "no such message"; saying which is the whole point of the line.
internal fun searchHorizonLabel(ctx: Context, horizon: SearchHorizon): String = when (horizon) {
    is SearchHorizon.AllTime -> L10n.search_horizon_all(ctx)
    is SearchHorizon.Months -> L10n.search_horizon_months(ctx, horizon.months.toInt())
}

// The horizon line under the scope filter, with a route to the setting that changes it. A
// statement the user cannot act on is half the value, so the whole row is tappable and lands in
// Settings, where the depth lives.
@Composable
internal fun SearchHorizonNotice(
    horizon: SearchHorizon?,
    onOpenSettings: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    if (horizon == null) return
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clickable(onClick = onOpenSettings)
            .padding(horizontal = 16.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = searchHorizonLabel(ctx, horizon),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.weight(1f),
        )
        Text(
            text = L10n.search_horizon_change(ctx),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.primary,
        )
    }
}
