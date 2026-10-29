// The Accounts category: every account, mail or not, from the core's one snapshot, each opening a
// page of its own (docs/accounts.md rule 11, docs/settings.md category 9). The page's sections are
// files of their own: what the account is used for and linked to (SettingsAccountPage.kt), how it
// signs in (SettingsAccountSignIn.kt), its servers and secret (SettingsAccountServers.kt). Add account is first, and the only
// place a later account is added.
//
// State lives in the core. A change forwards to it off the main thread and the page is drawn again
// from the next snapshot; what is being typed on it is remembered against the account, so a
// redraw leaves it alone.
package eu.allodia.mailcal

import android.content.Context
import android.widget.Toast
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.MailServerKind

/**
 * What Settings → Accounts draws beyond each account's mail settings: the snapshot and the page
 * open ([state]), which accounts' sign-in has expired, which take
 * a pasted secret, and what each change asks the activity to do.
 */
internal class AccountsSettingsUi(
    val state: AccountsUiState,
    val expired: Set<String> = emptySet(),
    // A JMAP account holding a pasted password or API token, which a new one replaces.
    val pastedSecret: Set<String> = emptySet(),
    val standardPort: ((MailServerKind, ConnectionSecurity) -> Int)? = null,
    val onAddAccount: () -> Unit = {},
    val onOpenPage: (String?) -> Unit = { state.page = it },
    val onSetUse: (accountId: String, AccountCapability, on: Boolean) -> Unit = { _, _, _ -> },
    val onSetLink: (accountId: String, LinkSlot, target: String?) -> Unit = { _, _, _ -> },
    val onSignInAgain: (accountId: String, adding: List<AccountCapability>) -> Unit = { _, _ -> },
    val onSaveServers: (accountId: String, AccountEndpoints) -> Unit = { _, _ -> },
    val onReplaceSecret: (accountId: String, secret: String) -> Unit = { _, _ -> },
    // An IMAP or JMAP account's sign-in (docs/accounts.md rule 17): what its server takes, asked
    // as its page opens; a browser sign-in, again or in place of a password ([switching]); and a
    // password in place of signing in.
    val onAskSignInOptions: (accountId: String) -> Unit = {},
    val onAccountSignIn: (accountId: String, switching: Boolean) -> Unit = { _, _ -> },
    val onSwitchToPassword: (accountId: String, password: String) -> Unit = { _, _ -> },
    // What a switch's sign-in will not reach, answered before the browser opens, and a sign-in of
    // their own for a calendar or address book that needs one (docs/accounts.md rule 18): the
    // form, its Connect, whether its server is asked to offer a sign-in at its own provider once it
    // is drawn, and that sign-in (docs/mail-oauth.md rule 13).
    val onUnreachedChoice: (UnreachedChoice) -> Unit = {},
    val onSeparateSignIn: (accountId: String) -> Unit = {},
    val onConnectSeparate: (username: String, password: String) -> Unit = { _, _ -> },
    val onSeparateFormShown: () -> Unit = {},
    val onSeparateProviderSignIn: () -> Unit = {},
    // A calendar or address book whose server was found on another site: the question that
    // allows it (docs/account-autodetect.md rule 14).
    val onReviewServerChange: (accountId: String) -> Unit = {},
    val onRemove: (accountId: String) -> Unit = {},
    val onDismissNotice: () -> Unit = { state.notice = null },
)

/** The open account's entry, or null while the list is showing. */
internal fun AccountsSettingsUi.openEntry(): AccountEntry? = state.page?.let(state::entry)

/** The list, or the open account's page, under what the last change said. */
@Composable
internal fun AccountsSettings(
    ui: AccountsSettingsUi,
    allodiaSyncCard: @Composable () -> Unit,
    page: @Composable (AccountEntry) -> Unit,
) {
    val ctx = LocalContext.current
    AccountNoticeView(ui.state.notice, ui.onDismissNotice)
    val entry = ui.openEntry()
    if (entry != null) {
        page(entry)
        return
    }
    Button(
        onClick = ui.onAddAccount,
        modifier = Modifier.fillMaxWidth().testTag("accounts-add"),
    ) { Text(L10n.action_add_account(ctx)) }
    Spacer(modifier = Modifier.height(8.dp))
    // What the person's other devices have to say, above their own accounts: an offer becomes one.
    allodiaSyncCard()
    val accounts = ui.state.snapshot.accounts
    if (accounts.isEmpty()) {
        Text(
            L10n.settings_accounts_empty(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(vertical = 8.dp),
        )
        return
    }
    accounts.forEach { account ->
        AccountListRow(account) { ui.onOpenPage(account.id) }
        Spacer(modifier = Modifier.height(8.dp))
    }
}

/** One account in the list: its address, then its kind, uses and links, and an arrow to its page. */
@Composable
private fun AccountListRow(entry: AccountEntry, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onOpen)
            .testTag("account-row-${entry.id}"),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(entry.address, style = MaterialTheme.typography.titleMedium)
                Text(
                    accountSummary(ctx, entry),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.width(8.dp))
            Icon(
                painter = painterResource(R.drawable.ic_keyboard_arrow_right),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

/** The list row's second line: the kind and uses, what it is linked to, and what needs the person. */
internal fun accountSummary(ctx: Context, entry: AccountEntry): String {
    val uses = AccountSettingsRules.inUse(entry).joinToString(", ") { useName(ctx, it) }
    val lines = mutableListOf("${kindLine(ctx, entry)} · $uses")
    AccountSettingsRules.linked(entry).forEach { (use, account) ->
        lines += L10n.settings_account_linked_line(ctx, useName(ctx, use), accountName(ctx, account))
    }
    if (AccountSettingsRules.needsPermission(entry)) {
        lines += L10n.settings_account_needs_permission(ctx)
    }
    return lines.joinToString("\n")
}

/** A use's name, as the app's own sections name it. */
internal fun useName(ctx: Context, capability: AccountCapability): String = when (capability) {
    AccountCapability.MAIL -> L10n.nav_mail(ctx)
    AccountCapability.CALENDAR -> L10n.nav_calendar(ctx)
    AccountCapability.CONTACTS -> L10n.nav_contacts(ctx)
    AccountCapability.COLLEAGUES -> L10n.account_use_colleagues(ctx)
    AccountCapability.SHARED_MAILBOXES -> L10n.account_use_shared_mailboxes(ctx)
}

/**
 * What the last change said. A success is said in passing; a warning or failure waits to be
 * dismissed, because what it answers was usually pressed far down the page.
 */
@Composable
private fun AccountNoticeView(notice: AccountNotice?, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    when (notice?.kind) {
        null -> Unit
        NoticeKind.SUCCESS -> LaunchedEffect(notice) {
            Toast.makeText(ctx, notice.title, Toast.LENGTH_SHORT).show()
            onDismiss()
        }
        NoticeKind.WARNING, NoticeKind.FAILURE -> AlertDialog(
            onDismissRequest = onDismiss,
            title = notice.title.takeIf { it.isNotEmpty() }?.let { title -> { Text(title) } },
            text = { Text(notice.message) },
            confirmButton = {
                TextButton(onClick = onDismiss) { Text(L10n.action_close(ctx)) }
            },
        )
    }
}
