// The mailbox screen: the account switcher + search/settings top bar, the message list
// (pull-to-refresh), and the compose FAB + timezone prompt. Split out of MainActivity.kt to
// keep each file under the 500-line limit (gradle auto-globs the package). State lives in the
// Rust core; this dispatches intents through the callbacks.
package eu.allodia.mailcal

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.material3.Surface
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
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.MailtoPrefill
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.RecipientSuggestion
import uniffi.mailcal_bindings.Recipients
import uniffi.mailcal_bindings.SearchScope
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
    onAddAccount: () -> Unit,
    onRemoveAccount: (String) -> Unit,
    // The folder navigation drawer state, owned by the caller (FolderDrawerScaffold). The
    // hamburger icon here opens it; the scaffold handles folder selection and rendering.
    drawerState: DrawerState,
    onSearch: (query: String?) -> Unit,
    // The search scope filter: which folders an active search covers. The core owns the scope
    // (and resets it when the search is cleared); this reports the toggle and the label for its
    // "current" side, which names whatever the list was showing when search opened.
    onSetSearchScope: (SearchScope) -> Unit,
    currentScopeLabel: String,
    // How far back the active search reached, or null when the list is not a search. The core
    // decides it from the sync depths of the accounts the scope covered.
    searchHorizon: SearchHorizon? = null,
    onRefresh: () -> Unit,
    onShowMore: () -> Unit,
    onOpen: (OpenedMessage) -> Unit,
    onOpenThread: (ThreadRow) -> Unit,
    onSetRead: (account: String, key: String, read: Boolean) -> Unit,
    onSetFlagged: (account: String, key: String, flagged: Boolean) -> Unit,
    onDelete: (account: String, key: String) -> Unit,
    onPermanentlyDelete: (account: String, key: String) -> Unit,
    inJunkFolder: Boolean,
    onMarkAsSpam: (account: String, key: String) -> Unit,
    onMarkAsNotSpam: (account: String, key: String) -> Unit,
    onArchiveThread: (account: String, threadId: String) -> Unit,
    onReply: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        subject: String,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    onForward: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        subject: String,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    replyRecipients: (account: String, key: String, replyAll: Boolean) -> RecipientSuggestion?,
    suggestionsFor: ((String) -> List<RecipientMatch>)? = null,
    // The signature library + lookups for the reply/forward composer, or null to leave signatures
    // out (a screenshot run, a test).
    signatures: ComposerSignatures? = null,
    onSubmitRich: (
        from: String?,
        recipients: Recipients,
        subject: String,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    // The persisted per-direction swipe actions, and the app-level default send account (the
    // composer's From opens on it in the unified inbox). Both live in the Rust core.
    swipe: SwipeSettings,
    onArchive: (account: String, key: String) -> Unit,
    defaultSendAccount: String?,
    timeZone: TimeZoneSnapshot?,
    onAcceptTimeZoneChange: () -> Unit,
    onDismissTimeZoneChange: () -> Unit,
    syncProgress: SyncProgressSnapshot?,
    offline: Boolean,
    unreachableAccounts: List<String>,
    connectionIssues: List<ConnectionIssue>,
    mailReauthEmails: List<String> = emptyList(),
    onReconnectMail: (email: String) -> Unit = {},
    signInExpired: List<ExpiredSignIn> = emptyList(),
    onSignInExpired: (account: ExpiredSignIn) -> Unit = {},
    onOpenSettings: () -> Unit,
    // A mail link (`mailto:`) the OS handed us, already decoded by the shared core: non-null
    // opens the composer pre-filled with it. `onMailtoConsumed` is called when that composer
    // closes, so the link is spent and does not re-open on the next recomposition.
    mailtoPrefill: MailtoPrefill? = null,
    onMailtoConsumed: () -> Unit = {},
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    // The search chrome's state (field shown, query, scope). The results themselves live in Rust,
    // which re-projects the snapshot as this dispatches; the field is a magnifier icon until
    // opened, so the top bar stays compact. Behaviour + invariants live in SearchBar.kt.
    val search = remember(onSearch, onSetSearchScope) { SearchBarState(onSearch, onSetSearchScope) }
    var showingCompose by remember { mutableStateOf(false) }
    // A mail link opens the composer. Guarded on `!showingCompose` rather than opening
    // unconditionally: the dialog reads its initial values once, so re-entering here while a
    // draft is up would neither pre-fill it nor warn, leaving the draft untouched is the
    // honest outcome, and the link stays pending until that composer closes.
    LaunchedEffect(mailtoPrefill) {
        if (mailtoPrefill != null && !showingCompose) {
            showingCompose = true
        }
    }
    // The swipe undo window. Delete/Archive dispatch nothing until the Snackbar settles, so Undo is
    // exact; Star applies at once and Undo un-stars. The rules live in SwipeUndoController.
    val snackbarHostState = remember { SnackbarHostState() }
    val swipeUndo = remember(onDelete, onArchive, onSetFlagged) {
        SwipeUndoController(onDelete = onDelete, onArchive = onArchive, onSetFlagged = onSetFlagged)
    }
    val visibleRows = remember(rows, swipeUndo.hiddenRowKeys) { swipeUndo.visibleRows(rows) }

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
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                if (search.open) {
                    SearchField(state = search, modifier = Modifier.weight(1f))
                } else {
                    // Hamburger always opens the folder navigation drawer.
                    IconButton(onClick = { scope.launch { drawerState.open() } }) {
                        Icon(
                            painter = painterResource(R.drawable.ic_menu),
                            contentDescription = L10n.a11y_open_folders(ctx),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    // The account switcher keeps a guaranteed share of the width so it never
                    // collapses behind the search + settings icons on a narrow phone.
                    AccountSwitcher(
                        accounts = accounts,
                        selectedAccount = selectedAccount,
                        unreachableAccounts = unreachableAccounts,
                        onSelectAccount = onSelectAccount,
                        onAddAccount = onAddAccount,
                        onRemoveAccount = onRemoveAccount,
                        modifier = Modifier.weight(1f),
                    )
                    // Search collapses to a magnifier to save space, expanding to the field on tap.
                    IconButton(onClick = search::openSearch) {
                        Icon(
                            painter = painterResource(R.drawable.ic_search),
                            contentDescription = L10n.search_placeholder(ctx),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    // Conversation grouping, language, time zone, per-account fetch depth + sync
                    // behaviour, the default quote style, and the database reset live in Settings.
                    IconButton(onClick = onOpenSettings) {
                        Icon(
                            painter = painterResource(R.drawable.ic_settings),
                            contentDescription = L10n.settings_title(ctx),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
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
            MailReauthBanner(mailReauthEmails, onReconnectMail, ctx)
            // An account whose stored sign-in has stopped being accepted (an expired or revoked
            // OAuth grant, a refused password). Not an outage, the server answered, so "Try
            // again" would never help; only a fresh sign-in does.
            SignInExpiredBanner(signInExpired, onSignInExpired, ctx)
            // Infinite scroll: when the last loaded row nears view, ask the core for the next page.
            // `onShowMore` is guarded in the host (it no-ops once every row is shown); keying the
            // effect on `rows.size` re-arms it after each page so scrolling keeps loading.
            val listState = rememberLazyListState()
            val nearEnd by remember {
                derivedStateOf {
                    val info = listState.layoutInfo
                    val last = info.visibleItemsInfo.lastOrNull()?.index ?: -1
                    info.totalItemsCount > 0 && last >= info.totalItemsCount - 5
                }
            }
            LaunchedEffect(nearEnd, visibleRows.size) {
                if (nearEnd) onShowMore()
            }
            // Incoming-mail behaviour (IMAP IDLE, a sync, a new-account download). The head-of-list
            // key changes whenever a newer message lands at the top.
            val topRowKey = visibleRows.firstOrNull()?.let { row ->
                when (row) {
                    is SnapshotRow.Flat -> "m:${row.row.account}:${row.row.key}"
                    is SnapshotRow.Thread -> "t:${row.row.account}:${row.row.threadId}"
                }
            }
            // Whether the list is pinned to the very top. Recorded only when a scroll SETTLES (a
            // user drag/fling or a programmatic scroll), never on a data change. That matters
            // because prepending a row re-anchors LazyColumn to keep the old top item in view,
            // bumping firstVisibleItemIndex to 1; reading the position after that would wrongly
            // conclude the user had scrolled away. Starts true so a cold start lands at the top.
            var pinnedToTop by remember { mutableStateOf(true) }
            LaunchedEffect(listState) {
                snapshotFlow { listState.isScrollInProgress }.collect { scrolling ->
                    if (!scrolling) {
                        pinnedToTop = listState.firstVisibleItemIndex == 0 &&
                            listState.firstVisibleItemScrollOffset == 0
                    }
                }
            }
            // When new mail arrives at the head: pull the list up if the user was already at the
            // top, otherwise surface a tappable pill so they can jump up without losing their place.
            var showNewMailPill by remember { mutableStateOf(false) }
            LaunchedEffect(topRowKey) {
                if (topRowKey == null) return@LaunchedEffect
                if (pinnedToTop) listState.animateScrollToItem(0) else showNewMailPill = true
            }
            // Dismiss the pill once the top is reached (via the pill itself or a manual scroll up).
            LaunchedEffect(pinnedToTop) {
                if (pinnedToTop) showNewMailPill = false
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
                    itemsIndexed(
                        visibleRows,
                        key = { _, row ->
                            // The account is part of the key: a provider key / thread id is unique
                            // only WITHIN an account, so two accounts can collide on one in the
                            // unified view, and reusing a slot across them would misroute a swipe.
                            when (row) {
                                is SnapshotRow.Flat -> "m:${row.row.account}:${row.row.key}"
                                is SnapshotRow.Thread -> "t:${row.row.account}:${row.row.threadId}"
                            }
                        },
                    ) { _, row ->
                        when (row) {
                            is SnapshotRow.Flat -> SwipeableFlatMessageRow(
                                message = row.row,
                                activeZoneId = timeZone?.active,
                                inJunkFolder = inJunkFolder,
                                swipe = swipe,
                                onSwipe = onSwipe,
                                accounts = accounts,
                                onOpen = onOpen,
                                onSetRead = onSetRead,
                                onSetFlagged = onSetFlagged,
                                onDelete = onDelete,
                                onPermanentlyDelete = onPermanentlyDelete,
                                onMarkAsSpam = onMarkAsSpam,
                                onMarkAsNotSpam = onMarkAsNotSpam,
                                onReply = onReply,
                                onForward = onForward,
                                replyRecipients = replyRecipients,
                                suggestionsFor = suggestionsFor,
                                signatures = signatures,
                            )
                            is SnapshotRow.Thread -> ThreadConversationRow(
                                thread = row.row,
                                activeZoneId = timeZone?.active,
                                // Open the conversation's latest message; the reading screen shows
                                // the older ones as a strip that opens each on tap.
                                onOpenThread = { onOpenThread(row.row) },
                                onArchiveThread = {
                                    onArchiveThread(row.row.account, row.row.threadId)
                                },
                            )
                        }
                    }
                }
                // A floating "new mail" pill at the top of the list: appears when mail arrives
                // while the user is scrolled down, and pulls the list to the top on tap
                // (Gmail-style). It dismisses itself once the top is reached, so it never
                // permanently covers content. Wrapped in a Box so the pill (whose own
                // AnimatedVisibility resolves top-level, not against this Box/Column scope) can be
                // aligned to the top-centre of the list.
                Box(modifier = Modifier.align(Alignment.TopCenter).padding(top = 8.dp)) {
                    NewMailPill(
                        visible = showNewMailPill,
                        label = L10n.mailbox_new_mail(ctx),
                        onClick = {
                            scope.launch { listState.animateScrollToItem(0) }
                            showNewMailPill = false
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
            SyncHint(syncProgress, accounts, ctx)
        }
        // The familiar bottom-right floating action button for composing a new message.
        FloatingActionButton(
            onClick = { showingCompose = true },
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

    if (showingCompose) {
        RichComposeMessageDialog(
            suggestionsFor = suggestionsFor,
            signatures = signatures,
            mode = RichComposeMode.New,
            accounts = accounts,
            // Composing while one mailbox is open sends from that account; in the unified inbox
            // there is no such context, so the app-level default send account decides (and the
            // composer falls back to the first account when none is set).
            initialFrom = selectedAccount ?: defaultSendAccount,
            // Empty for the FAB's blank composer; a mail link fills what it named. The core has
            // already dropped every header a link may not set.
            initialTo = mailtoPrefill?.to.orEmpty(),
            initialCc = mailtoPrefill?.cc.orEmpty(),
            initialBcc = mailtoPrefill?.bcc.orEmpty(),
            initialSubject = mailtoPrefill?.subject.orEmpty(),
            initialBody = mailtoPrefill?.body.orEmpty(),
            onDismiss = {
                showingCompose = false
                onMailtoConsumed()
            },
            onSubmitRich = onSubmitRich,
        )
    }

    // Shown (as an overlay) only when the core reports a pending device-zone change.
    TimeZoneChangePrompt(
        timeZone = timeZone,
        onAccept = onAcceptTimeZoneChange,
        onDismiss = onDismissTimeZoneChange,
    )
}

// The floating "new mail" pill (a rounded, elevated primary-coloured chip with an up-arrow). Kept
// as its own composable so `AnimatedVisibility` binds to the plain top-level overload rather than
// the caller's Box/Column scope, which would otherwise reject the implicit receiver.
@androidx.compose.runtime.Composable
private fun NewMailPill(visible: Boolean, label: String, onClick: () -> Unit) {
    AnimatedVisibility(visible = visible) {
        Surface(
            onClick = onClick,
            shape = RoundedCornerShape(50),
            color = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
            shadowElevation = 4.dp,
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(painterResource(R.drawable.ic_keyboard_arrow_up), contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(label)
            }
        }
    }
}
