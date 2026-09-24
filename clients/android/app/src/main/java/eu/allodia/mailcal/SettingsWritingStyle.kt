// Settings → Writing style (docs/ai.md, docs/settings.md row 7): the learned styles and which one
// each account drafts in, drawn the way the Signatures category draws its library and its
// per-account slots, directly before it in the hub's order. Learning opens its own full-screen
// sheet (WritingStyleLearnDialog.kt) and a style opens its own screen (WritingStyleRevealDialog.kt),
// as a signature's editor does. State lives in the core; every write re-signals WRITING_STYLE.
package eu.allodia.mailcal

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import java.time.ZoneId
import uniffi.mailcal_bindings.AccountWritingStyleRow
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.CreditBalance
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.OwnAiEndpoint
import uniffi.mailcal_bindings.OwnEndpointException
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleRow
import uniffi.mailcal_bindings.WritingStyleSnapshot

// Every core call the Writing style screens and the own endpoint card make.
internal interface WritingStyleActions : LearnCore {
    fun detail(id: String): WritingStyleDetail?

    // Renames and replaces the notes together: the reveal saves both with one button.
    fun save(id: String, name: String, notes: String)

    fun forget(id: String)

    fun assign(account: String, style: String?)

    // Why the endpoint was not saved, or null when it was.
    fun saveEndpoint(
        baseUrl: String,
        model: String,
        declared: JurisdictionClass?,
        key: String?,
    ): OwnEndpointException?

    fun removeEndpoint(): OwnEndpointException?

    // Blocking work is the implementation's to move off the main thread; a failure is ignored,
    // and the stored line stays.
    fun refreshBalance()
}

internal class WritingStyleSettings(
    // Null until the first pull; `route` null means the category is not shown.
    val snapshot: WritingStyleSnapshot?,
    val ownEndpoint: OwnAiEndpoint?,
    val actions: WritingStyleActions,
    val background: Background = ThreadBackground,
)

@Composable
internal fun WritingStyleCategory(state: WritingStyleSettings, activeZoneId: String?) {
    val ctx = LocalContext.current
    val snapshot = state.snapshot ?: return
    val zone = displayZone(activeZoneId)
    var learning by remember { mutableStateOf<LearnFlow?>(null) }
    var revealing by remember { mutableStateOf<String?>(null) }

    Text(L10n.writing_style_intro(ctx), style = MaterialTheme.typography.bodyMedium)
    Spacer(modifier = Modifier.height(8.dp))
    // A refusal is known before anything is read, so the button that would only meet it is not
    // offered.
    val refused = snapshot.refused
    if (refused != null) {
        Text(
            writingStyleRefusalText(ctx, refused.mode),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
        )
    } else {
        Button(
            onClick = {
                learning = LearnFlow(snapshot.accounts, state.actions, state.background, zone)
                    .also { it.start() }
            },
            enabled = snapshot.accounts.isNotEmpty(),
        ) {
            Text(L10n.writing_style_learn(ctx))
        }
    }
    val balance = snapshot.balance
    if (snapshot.route == AiRoute.RELAY && balance != null) {
        WritingStyleCreditsLine(balance, activeZoneId)
    }
    Spacer(modifier = Modifier.height(8.dp))
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            WritingStyleLibrary(snapshot.styles, zone) { revealing = it }
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            Text(L10n.writing_style_accounts_heading(ctx), style = MaterialTheme.typography.titleMedium)
            if (snapshot.accounts.isEmpty()) {
                Text(
                    L10n.settings_accounts_empty(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            snapshot.accounts.forEach { account ->
                AccountWritingStylePicker(account, snapshot.styles) { state.actions.assign(account.accountId, it) }
            }
        }
    }

    learning?.let { flow ->
        WritingStyleLearnDialog(
            flow = flow,
            snapshot = snapshot,
            ownEndpoint = state.ownEndpoint,
            zone = zone,
            onLearned = { id ->
                learning = null
                revealing = id
            },
            onClose = {
                flow.close()
                learning = null
            },
        )
    }
    revealing?.let { id ->
        WritingStyleRevealDialog(
            id = id,
            actions = state.actions,
            accounts = snapshot.accounts,
            zone = zone,
            onClose = { revealing = null },
        )
    }
}

@Composable
private fun WritingStyleLibrary(styles: List<WritingStyleRow>, zone: ZoneId, onOpen: (String) -> Unit) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    if (styles.isEmpty()) {
        Text(
            L10n.writing_style_empty(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    styles.forEach { style ->
        Column(modifier = Modifier.fillMaxWidth().clickable { onOpen(style.id) }.padding(vertical = 8.dp)) {
            Text(style.name, style = MaterialTheme.typography.titleMedium)
            writingStyleRowLines(ctx, style, zone, locale).forEach { line ->
                Text(
                    line,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

// One account's style: the address, then its choice, "None" included. Drawn like a signature
// slot's picker, the current choice on the button and the choices in its menu.
@Composable
private fun AccountWritingStylePicker(
    account: AccountWritingStyleRow,
    styles: List<WritingStyleRow>,
    onSelect: (String?) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    val current = styles.firstOrNull { it.id == account.style }
    Text(
        account.email,
        style = MaterialTheme.typography.titleSmall,
        maxLines = 1,
        overflow = TextOverflow.Ellipsis,
        modifier = Modifier.padding(top = 12.dp),
    )
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(
                current?.name ?: L10n.writing_style_none(ctx),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = account.email,
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(L10n.writing_style_none(ctx)) },
                onClick = {
                    expanded = false
                    onSelect(null)
                },
            )
            styles.forEach { style ->
                DropdownMenuItem(
                    text = { Text(style.name) },
                    onClick = {
                        expanded = false
                        onSelect(style.id)
                    },
                )
            }
        }
    }
}

// What the relay last said was left, and when it said it, formatted the way a list row's time is.
@Composable
internal fun WritingStyleCreditsLine(balance: CreditBalance, activeZoneId: String?) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    val asOf = relativeDate(engineTimestamp(balance.asOf), activeZoneId, LocalUse24Hour.current)
    Text(
        writingStyleCreditsText(ctx, balance, locale, asOf),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(top = 8.dp),
    )
}
