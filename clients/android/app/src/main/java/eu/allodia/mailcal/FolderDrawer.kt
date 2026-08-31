// Navigation drawer showing every account's folder tree plus an "All Inboxes" entry.
// A ModalNavigationDrawer wraps the mailbox screen, a hamburger icon opens it. The drawer
// is always available regardless of which account is selected. Each account is an
// expandable section: tap once to expand, tap again to select its all-mail view. Tapping
// a folder closes the drawer and navigates. The list is scrollable for long folder trees.
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
import androidx.annotation.DrawableRes
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.DrawerState
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.NavigationDrawerItem
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.AccountFolderRow
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.FolderRole
import uniffi.mailcal_bindings.FolderRow

// Wraps `content` in a ModalNavigationDrawer. Always available, even in the unified
// all-inboxes view the drawer lists all accounts and lets the user dive into any of them.
@Composable
internal fun FolderDrawerScaffold(
    drawerState: DrawerState,
    accounts: List<AccountRow>,
    accountFolders: List<AccountFolderRow>,
    selectedAccount: String?,
    selectedFolder: String?,
    unifiedUnread: UInt,
    onSelectAccount: (id: String?) -> Unit,
    onSelectFolder: (account: String, key: String) -> Unit,
    onSetExpanded: (id: String, expanded: Boolean) -> Unit,
    content: @Composable () -> Unit,
) {
    val scope = rememberCoroutineScope()
    ModalNavigationDrawer(
        drawerState = drawerState,
        gesturesEnabled = true,
        drawerContent = {
            FolderDrawerSheet(
                drawerState = drawerState,
                accounts = accounts,
                accountFolders = accountFolders,
                selectedAccount = selectedAccount,
                selectedFolder = selectedFolder,
                unifiedUnread = unifiedUnread,
                onSelectAccount = onSelectAccount,
                onSelectFolder = onSelectFolder,
                onSetExpanded = onSetExpanded,
            )
        },
        content = {
            // Back in the mailbox, from the outside in: an open drawer shuts, then a folder
            // narrowing returns to the unified inbox, and only from THERE does the press reach
            // the platform and close the app. Search is not in this list because it registers
            // later (inside `content`), so it still unwinds before either of these.
            //
            // The drawer half is OUR handler, not the one ModalNavigationDrawer registers for its
            // predictive-back animation: on a Galaxy Note 20 (Android 13) that one never fired, so
            // an open drawer swallowed nothing and the press closed the app instead. Declared
            // inside `content` on purpose, that puts it after the drawer's own callback, so ours
            // wins where both are live.
            //
            // The folder half exists because the core opens every launch on the unified inbox
            // (`selected_account` starts `None` and is never persisted), so that view, not
            // whichever folder you wandered into, is the mailbox's home.
            val narrowed = selectedAccount != null || selectedFolder != null
            BackHandler(enabled = drawerState.isOpen || narrowed) {
                if (drawerState.isOpen) {
                    scope.launch { drawerState.close() }
                } else {
                    // One step, undoing ONE drawer tap: selecting the unified list drops the
                    // folder with it, since a folder belongs to an account. Going
                    // folder -> account -> unified would make the user press back twice to
                    // reverse something they did once.
                    onSelectAccount(null)
                }
            }
            content()
        },
    )
}

@Composable
private fun FolderDrawerSheet(
    drawerState: DrawerState,
    accounts: List<AccountRow>,
    accountFolders: List<AccountFolderRow>,
    selectedAccount: String?,
    selectedFolder: String?,
    unifiedUnread: UInt,
    onSelectAccount: (id: String?) -> Unit,
    onSelectFolder: (account: String, key: String) -> Unit,
    onSetExpanded: (id: String, expanded: Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val foldersByAccount = remember(accountFolders) {
        accountFolders.associateBy { it.accountId }
    }
    ModalDrawerSheet {
        LazyColumn {
            item {
                Spacer(modifier = Modifier.height(12.dp))
                // "All Inboxes", unified view across all accounts.
                NavigationDrawerItem(
                    label = { Text(L10n.sidebar_all_inboxes(ctx)) },
                    icon = { Icon(painterResource(R.drawable.ic_inbox), contentDescription = null) },
                    badge = unreadBadge(unifiedUnread, ctx),
                    selected = selectedAccount == null,
                    onClick = {
                        scope.launch { drawerState.close() }
                        onSelectAccount(null)
                    },
                    modifier = Modifier.padding(horizontal = 12.dp),
                )
                Spacer(modifier = Modifier.height(4.dp))
            }
            accounts.forEach { account ->
                // Expansion is the CORE's, not this composable's. It used to be a
                // `remember(accounts, selectedAccount)` seeded from the selection, which meant it
                // reset on every recomposition that touched either, so opening a folder in
                // another account shut this one, and a restart shut them all.
                val isExpanded = account.expanded
                val folders = foldersByAccount[account.id]?.folders ?: emptyList()
                // Account header: the chevron opens or shuts the tree (persisted, and nothing
                // else moves); the row itself selects the account's all-mail view.
                item(key = "header-${account.id}") {
                    AccountHeader(
                        account = account,
                        expanded = isExpanded,
                        selected = account.id == selectedAccount && selectedFolder == null,
                        onToggle = { onSetExpanded(account.id, !isExpanded) },
                        onClick = {
                            scope.launch { drawerState.close() }
                            onSelectAccount(account.id)
                        },
                        ctx = ctx,
                    )
                }
                if (isExpanded) {
                    items(folders, key = { "folder-${account.id}-${it.key}" }) { folder ->
                        NavigationDrawerItem(
                            label = { Text(folderLabel(folder.role, folder.name, ctx)) },
                            icon = {
                                Icon(
                                    painter = painterResource(folderIcon(folder.role)),
                                    contentDescription = null,
                                )
                            },
                            badge = unreadBadge(folder.unread, ctx),
                            selected = account.id == selectedAccount && folder.key == selectedFolder,
                            onClick = {
                                scope.launch { drawerState.close() }
                                onSelectFolder(account.id, folder.key)
                            },
                            modifier = Modifier.padding(start = 24.dp, end = 12.dp),
                        )
                    }
                }
            }
            item { Spacer(modifier = Modifier.height(12.dp)) }
        }
    }
}

// The trailing unread count, or null for no badge at all, which is what a zero means, and also
// what a provider reporting no count means (docs/folder-pane.md). NavigationDrawerItem takes a
// nullable badge slot, so "no badge" costs no layout rather than an empty one.
//
// The number alone reads as a list position to a screen reader, so the row carries the localized
// sentence; the Text itself is cleared, or the count would be announced twice.
private fun unreadBadge(unread: UInt, ctx: android.content.Context): (@Composable () -> Unit)? =
    if (unread == 0u) {
        null
    } else {
        {
            Text(
                text = unread.toString(),
                modifier = Modifier.semantics {
                    // Saturating: the label is decoration, and a mailbox past Int.MAX_VALUE
                    // unread should read as "a lot", not wrap to a negative number.
                    contentDescription =
                        L10n.a11y_unread_count(ctx, unread.coerceAtMost(Int.MAX_VALUE.toUInt()).toInt())
                },
            )
        }
    }

// The account row: the chevron is its own target (open/shut the tree, persisted, nothing else
// moves), the rest of the row selects the account's all-mail view.
//
// Two targets rather than the old tap-once-to-expand-tap-again-to-select: with expansion no longer
// tied to the selection, one target could no longer express both, and a second tap meaning
// something different from the first is a guess the user has to make twice.
@Composable
private fun AccountHeader(
    account: AccountRow,
    expanded: Boolean,
    selected: Boolean,
    onToggle: () -> Unit,
    onClick: () -> Unit,
    ctx: android.content.Context,
) {
    NavigationDrawerItem(
        label = { Text(account.email, maxLines = 1) },
        icon = {
            IconButton(
                onClick = onToggle,
                modifier = Modifier.size(32.dp),
            ) {
                Icon(
                    painter = painterResource(
                        if (expanded) R.drawable.ic_keyboard_arrow_down
                        else R.drawable.ic_keyboard_arrow_right,
                    ),
                    contentDescription =
                        if (expanded) L10n.a11y_collapse_account(ctx) else L10n.a11y_expand_account(ctx),
                    modifier = Modifier.size(20.dp),
                )
            }
        },
        selected = selected,
        onClick = onClick,
        modifier = Modifier.padding(horizontal = 12.dp),
    )
}

// What a folder is CALLED on screen: our own word for a known folder, the server's name for
// everything else (docs/folder-pane.md rule 12).
//
// The server's name for a special folder is not a name the user chose, it is whatever their
// provider stores, in whatever language and casing it likes: INBOX in capitals (the one name IMAP
// mandates), "Deleted Items" from Exchange, "[Gmail]/Sent Mail". Naming them ourselves is what
// every mail client does, and it is what makes the folder list follow the app's language instead
// of the server's.
//
// OTHER keeps the server name: the core collapses flagged, important and all-mail into that one
// value, so no single word is honest for it.
//
// Internal (not private) because the sync-settings folder list names the same folders.
internal fun folderLabel(role: FolderRole?, name: String, ctx: android.content.Context): String =
    when (role) {
        FolderRole.INBOX -> L10n.folder_inbox(ctx)
        FolderRole.DRAFTS -> L10n.folder_drafts(ctx)
        FolderRole.SENT -> L10n.folder_sent(ctx)
        FolderRole.ARCHIVE -> L10n.folder_archive(ctx)
        FolderRole.JUNK -> L10n.folder_junk(ctx)
        FolderRole.TRASH -> L10n.folder_trash(ctx)
        FolderRole.OTHER, null -> name
    }

// Maps a folder's special role to a recognisable Material Symbol. Keeps the drawer visually
// scannable without string comparisons.
//
// Three of these used to be stand-ins forced by what the old material-icons-core subset happened
// to contain, and Archive was the loud one: it drew a *calendar*. Vendoring the Symbols we
// actually name removes that constraint, so Inbox, Archive and "some other folder" now get their
// own glyphs instead of borrowing an unrelated one.
@DrawableRes
private fun folderIcon(role: FolderRole?): Int =
    when (role) {
        FolderRole.INBOX -> R.drawable.ic_inbox
        FolderRole.DRAFTS -> R.drawable.ic_edit
        FolderRole.SENT -> R.drawable.ic_send
        FolderRole.ARCHIVE -> R.drawable.ic_archive
        FolderRole.JUNK -> R.drawable.ic_warning
        FolderRole.TRASH -> R.drawable.ic_delete
        FolderRole.OTHER, null -> R.drawable.ic_folder
    }
