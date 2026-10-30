// The mailbox screen: the account switcher + search/settings top bar, the message list
// (pull-to-refresh), and the compose FAB + timezone prompt. Split out of MainActivity.kt to
// keep each file under the 500-line limit (gradle auto-globs the package). State lives in the
// Rust core; this dispatches intents through the callbacks.
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.material3.DrawerState
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Snackbar
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.BulkAction
import uniffi.mailcal_bindings.ComposeOrigin
import uniffi.mailcal_bindings.EmptyReason
import uniffi.mailcal_bindings.RowMenu
import uniffi.mailcal_bindings.SelectedRow
import uniffi.mailcal_bindings.SendStatus
import uniffi.mailcal_bindings.SearchHorizon
import uniffi.mailcal_bindings.SnapshotRow
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.SyncProgressSnapshot
import uniffi.mailcal_bindings.ThreadRow
import uniffi.mailcal_bindings.TimeZoneSnapshot

@OptIn(ExperimentalMaterial3Api::class)
@androidx.compose.runtime.Composable
internal fun MailboxScreen(
    rows: List<SnapshotRow>,
    sendStatus: SendStatus,
    accounts: List<AccountRow>,
    selectedAccount: String?,
    onSelectAccount: (String?) -> Unit,
    onRemoveAccount: (String) -> Unit,
    // The folder navigation drawer state, owned by the caller (FolderDrawerScaffold). The
    // hamburger icon here opens it; the scaffold handles folder selection and rendering.
    drawerState: DrawerState,
    // Where the list is scrolled to, owned by the caller so it outlives this composition: opening
    // a message replaces this screen rather than covering it. Defaulted for a test or a screenshot
    // run, which never leaves the list.
    position: MailListPosition = remember { MailListPosition() },
    // The search chrome (field shown, query, scope), owned by the caller for the same reason and
    // holding the dispatch to the core. Behaviour and invariants live in SearchBar.kt.
    search: SearchBarState,
    // Names the "current" side of the scope filter: whatever the list was showing when search
    // opened.
    currentScopeLabel: String,
    // How far back the active search reached, or null when the list is not a search. The core
    // decides it from the sync depths of the accounts the scope covered.
    searchHorizon: SearchHorizon? = null,
    onRefresh: () -> Unit,
    onShowMore: () -> Unit,
    // Opens a message, or resumes it in a composer when its row says it is a draft
    // (docs/drafts.md).
    onOpen: (opened: OpenedMessage, draft: Boolean) -> Unit,
    onOpenThread: (ThreadRow) -> Unit,
    // What a swipe runs, through its undo window.
    onSetFlagged: (account: String, key: String, flagged: Boolean) -> Unit,
    onDelete: (account: String, key: String) -> Unit,
    // One action over every selected row, as a single batch in the core: one optimistic hide and
    // one sync per account, rather than one of each per row (docs/list-selection.md, rule 8).
    onActOnSelection: (rows: List<SelectedRow>, action: BulkAction) -> Unit,
    // Opens the composer from the compose button (MainActivityComposer.kt).
    onCompose: (ComposeOrigin) -> Unit,
    // What a row's menu holds, which the core answers as it opens, and what picking an item runs
    // (docs/row-actions.md). Defaulted for a test or a screenshot run, whose menus hold nothing.
    rowMenu: (List<SelectedRow>) -> RowMenu = { RowMenu(emptyList()) },
    onRowPick: (RowPick) -> Unit = {},
    // Where a row's Move to folder… may file it, or null to leave the item disabled.
    messageFiling: MessageFiling? = null,
    // The persisted per-direction swipe actions, which live in the Rust core.
    swipe: SwipeSettings,
    onArchive: (account: String, key: String) -> Unit,
    timeZone: TimeZoneSnapshot?,
    onAcceptTimeZoneChange: () -> Unit,
    onDismissTimeZoneChange: () -> Unit,
    syncProgress: SyncProgressSnapshot?,
    // Why the list has no rows, or null whenever it has some (`docs/folder-pane.md`, rule 20).
    emptyReason: EmptyReason? = null,
    offline: Boolean,
    unreachableAccounts: List<String>,
    connectionIssues: List<ConnectionIssue>,
    mailReauth: List<NamedAccount> = emptyList(),
    onReconnectMail: (accountId: String) -> Unit = {},
    signInExpired: List<ExpiredSignIn> = emptyList(),
    onSignInExpired: (account: ExpiredSignIn) -> Unit = {},
    // Opens an account's page in Settings, for a sign-in only a new password or token mends.
    onOpenAccountPage: (accountId: String) -> Unit = {},
    // What an account's removal names in its confirmation.
    removalOf: (accountId: String) -> AccountRemoval? = { null },
    onOpenSettings: () -> Unit,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    // The swipe undo window. Delete/Archive dispatch nothing until the Snackbar settles, so Undo is
    // exact; Star applies at once and Undo un-stars. The rules live in SwipeUndoController.
    val snackbarHostState = remember { SnackbarHostState() }
    val swipeUndo = remember(onDelete, onArchive, onSetFlagged) {
        SwipeUndoController(onDelete = onDelete, onArchive = onArchive, onSetFlagged = onSetFlagged)
    }
    val visibleRows = remember(rows, swipeUndo.hiddenRowKeys) { swipeUndo.visibleRows(rows) }

    // The list's multi-selection. A long press enters it, a tap then adds or removes a row, and
    // Back leaves it; the rules live in MailSelectionState (docs/list-selection.md).
    val selection = remember { MailSelectionState() }
    // A row that has left the list (archived, deleted, or filtered away by a folder change or a
    // search) leaves the selection with it, so an action can never reach a message nobody can see.
    LaunchedEffect(rows) { selection.retainListed(rows) }
    BackHandler(enabled = selection.active) { selection.clear() }
    val actOnSelection: (BulkAction) -> Unit = { action ->
        onActOnSelection(selection.selectedRows(), action)
        // A move empties the selection: the rows it named are leaving. A keyword edit keeps it,
        // because the user is usually part-way through working through the set.
        if (action.removesRows()) selection.clear()
    }
    // Every row's menu. A batch over the selection runs the bar's own path, which lets go of the
    // rows a move takes.
    val menus = RowMenus(
        menu = rowMenu,
        rowsFor = { row ->
            val selected = selection.selectedRows()
            if (row in selected) selected else listOf(row)
        },
        filing = messageFiling?.let { filing ->
            MessageFiling(filing.targets) { moved, account, folder ->
                filing.move(moved, account, folder)
                if (moved == selection.selectedRows()) selection.clear()
            }
        },
        onPick = { pick ->
            if (pick is RowPick.Rows && pick.rows == selection.selectedRows()) {
                actOnSelection(pick.action)
            } else {
                onRowPick(pick)
            }
        },
    )

    SwipeUndoEffect(
        pending = swipeUndo.pending,
        snackbarHostState = snackbarHostState,
        onCommit = { swipe ->
            swipeUndo.commit(swipe)
            // The core hides the row itself now. Drop our own hide shortly after, so a rejected
            // edit (which makes the core restore the row) doesn't leave it invisible.
            if (swipe.hidesRow) {
                scope.launch {
                    delay(COMMIT_HIDE_GRACE_MS)
                    swipeUndo.releaseHide(swipe)
                }
            }
        },
        onRevert = swipeUndo::revert,
    )
    val onSwipe: (String, String, SwipeActionKind) -> Unit = swipeUndo::onSwipe

    // Pull-to-refresh: the gesture dispatches a mail sync and spins the indicator until that sync
    // settles. The spinner is held briefly then cleared; a sync that starts within the window
    // re-keys the effect (syncActive -> true) and keeps it up until the sync finishes, while a
    // refresh that starts no sync (offline / nothing new) just clears after the short hold.
    var refreshing by remember { mutableStateOf(false) }
    val syncActive = syncProgress?.active == true
    LaunchedEffect(refreshing, syncActive) {
        if (refreshing && !syncActive) {
            delay(700)
            refreshing = false
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Column(modifier = Modifier.fillMaxSize()) {
            SendStatusBanner(sendStatus, ctx)
            // While rows are selected the contextual bar takes this row's place: a phone has no
            // modifier keys, so selecting is a mode, and the bar is how it says so and how it is
            // left (docs/list-selection.md).
            if (selection.active) {
                MailSelectionBar(
                    selection = selection,
                    rows = visibleRows,
                    actions = SelectionBarActions.of(rowMenu(selection.selectedRows())),
                    onAct = actOnSelection,
                )
            } else {
                MailboxTopBar(
                    search = search,
                    accounts = accounts,
                    selectedAccount = selectedAccount,
                    unreachableAccounts = unreachableAccounts,
                    onSelectAccount = onSelectAccount,
                    onRemoveAccount = onRemoveAccount,
                    onOpenDrawer = { scope.launch { drawerState.open() } },
                    onOpenSettings = onOpenSettings,
                    removalOf = removalOf,
                )
            }
            // While searching, the scope filter sits under the field: "this folder" (whatever the
            // list was showing) or all mail. It is offered from the first keystroke, so narrowing
            // never means retyping the query.
            if (search.open) {
                SearchScopeFilter(state = search, currentLabel = currentScopeLabel)
                // …and under it, how far back the results reach. Only while there is a query:
                // the core leaves the horizon unset for a list nobody searched.
                SearchHorizonNotice(horizon = searchHorizon, onOpenSettings = onOpenSettings)
            }
            HorizontalDivider()
            // A persistent banner while the device is offline, so the user knows the mail on screen
            // is the last-synced copy; it clears itself when connectivity returns and the core
            // auto-refreshes.
            OfflineBanner(offline, ctx)
            // While the device is online but one or more accounts can't reach their server: a
            // friendly banner naming them, with a Details action and Try again to re-dial.
            ConnectionIssuesBanner(connectionIssues, onRefresh, ctx)
            // A standing permission gap (not an outage): a Microsoft account whose grant lacks the
            // mail write/send scopes, so a send or mail action was refused. "Reconnect" re-runs its
            // sign-in with the full scope set; the banner clears once a send/action succeeds.
            MailReauthBanner(mailReauth, onReconnectMail, ctx)
            // An account whose stored sign-in has stopped being accepted (an expired or revoked
            // OAuth grant, a refused password). Not an outage, the server answered, so "Try
            // again" would never help; only a fresh sign-in does.
            SignInExpiredBanner(signInExpired, onSignInExpired, ctx) { onOpenAccountPage(it.id) }
            val listState = position.listState
            // Infinite scroll: when the last loaded row nears view, ask the core for the next page.
            // `onShowMore` is guarded in the host (it no-ops once every row is shown); keying the
            // effect on `rows.size` re-arms it after each page so scrolling keeps loading.
            val nearEnd by remember(listState) {
                derivedStateOf {
                    val info = listState.layoutInfo
                    val last = info.visibleItemsInfo.lastOrNull()?.index ?: -1
                    info.totalItemsCount > 0 && last >= info.totalItemsCount - 5
                }
            }
            LaunchedEffect(nearEnd, visibleRows.size) {
                if (nearEnd) onShowMore()
            }
            // Where a settled scroll left the list. Reported on every settle rather than read on
            // demand, and on entry too, so returning from a message picks the position back up.
            LaunchedEffect(listState) {
                snapshotFlow { listState.isScrollInProgress }.collect { scrolling ->
                    if (!scrolling) {
                        position.scrollSettled(
                            listState.firstVisibleItemIndex,
                            listState.firstVisibleItemScrollOffset,
                        )
                    }
                }
            }
            // Incoming-mail behaviour (IMAP IDLE, a sync, a new-account download). The head-of-list
            // key changes whenever a newer message lands at the top: pull the list up if the user
            // was already there, otherwise raise a pill so they can jump up without losing their
            // place. MailListPosition decides which, and it is what knows that an unchanged head
            // means the user merely came back from reading, not that mail arrived.
            val topRowKey = visibleRows.firstOrNull()?.let(::listKey)
            LaunchedEffect(topRowKey) {
                if (position.headOfList(topRowKey)) listState.animateScrollToItem(0)
            }
            // Pull down to sync, the standard gesture that replaces the old footer refresh button.
            PullToRefreshBox(
                isRefreshing = refreshing,
                onRefresh = {
                    refreshing = true
                    onRefresh()
                },
                modifier = Modifier.weight(1f),
            ) {
                LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
                    // A stable key per row ties each LazyColumn slot (and its remembered
                    // SwipeToDismissBox state, whose confirmValueChange closes over the row's
                    // key) to a specific message. Without it, rows are reused positionally and a
                    // pending swipe would Trash whatever message later lands at that index.
                    itemsIndexed(visibleRows, key = { _, row -> listKey(row) }) { _, row ->
                        when (row) {
                            is SnapshotRow.Flat -> SwipeableFlatMessageRow(
                                message = row.row,
                                activeZoneId = timeZone?.active,
                                swipe = swipe,
                                onSwipe = onSwipe,
                                selected = selection.contains(row),
                                selecting = selection.active,
                                onToggleSelect = { selection.toggle(row) },
                                onOpen = onOpen,
                                menus = menus,
                            )
                            is SnapshotRow.Thread -> ThreadConversationRow(
                                thread = row.row,
                                activeZoneId = timeZone?.active,
                                selected = selection.contains(row),
                                selecting = selection.active,
                                onToggleSelect = { selection.toggle(row) },
                                // Open the conversation's newest message; the reading screen shows
                                // the others as a strip that opens each on tap.
                                onOpenThread = { onOpenThread(row.row) },
                                menus = menus,
                            )
                        }
                    }
                }
                // Over the rows rather than in place of the list, so the pull-to-sync gesture
                // survives: an empty folder is one a pull is a reasonable answer to.
                MailboxEmpty(reason = emptyReason, onOpenSettings = onOpenSettings)
                // A floating "new mail" pill at the top of the list: appears when mail arrives
                // while the user is scrolled down, and pulls the list to the top on tap
                // (Gmail-style). It dismisses itself once the top is reached, so it never
                // permanently covers content. Wrapped in a Box so the pill (whose own
                // AnimatedVisibility resolves top-level, not against this Box/Column scope) can be
                // aligned to the top-centre of the list.
                Box(modifier = Modifier.align(Alignment.TopCenter).padding(top = 8.dp)) {
                    NewMailPill(
                        visible = position.showNewMailPill,
                        label = L10n.mailbox_new_mail(ctx),
                        onClick = {
                            scope.launch { listState.animateScrollToItem(0) }
                            position.dismissNewMailPill()
                        },
                    )
                }
            }
            // Background-download progress: a thin bar with a "downloading Y of X" count, shown
            // only while a sync is fetching mail (the rows arrive on their own MAILBOX_LIST signal).
            //
            // Under the list, not above it. Above, the bar appearing and disappearing resized the
            // list and shifted every row under the user's thumb, for a background pass they did
            // not start. Outside the PullToRefreshBox, so it neither scrolls nor moves with the
            // refresh gesture.
            SyncProgressBar(syncProgress, ctx)
            // The background-sync hint shares that strip: a pass nobody started says so in a
            // caption rather than a bar. The two are mutually exclusive in the core, an awaited
            // download is already explained by the bar, so they never stack.
            SyncStatus(syncProgress, accounts, ctx)
        }
        // The familiar bottom-right floating action button for composing a new message.
        FloatingActionButton(
            // A new message on the selected account; in the unified inbox the core decides, from
            // the app-level default send account.
            onClick = { onCompose(ComposeOrigin.New(selectedAccount)) },
            modifier = Modifier
                .align(Alignment.BottomEnd)
                .padding(16.dp),
        ) {
            Icon(painterResource(R.drawable.ic_edit), contentDescription = L10n.action_compose(ctx))
        }
        // The swipe-action Snackbar with its Undo, sitting above the FAB so the button never covers
        // the action. The Snackbar's own timeout is what commits a deferred Delete/Archive.
        SnackbarHost(
            hostState = snackbarHostState,
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 88.dp),
        ) { data ->
            Snackbar(snackbarData = data)
        }
    }

    // Shown (as an overlay) only when the core reports a pending device-zone change.
    TimeZoneChangePrompt(
        timeZone = timeZone,
        onAccept = onAcceptTimeZoneChange,
        onDismiss = onDismissTimeZoneChange,
    )
}
