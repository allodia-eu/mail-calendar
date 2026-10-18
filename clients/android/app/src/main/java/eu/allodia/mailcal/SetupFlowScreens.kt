// The two steps a connect can end on (docs/onboarding.md, "After the connect"): the uses a JMAP
// server offers, chosen once it has said, and the link step, where an account that lacks a calendar
// or contacts can use another account's. What each decides is SetupFlow's; these draw it.
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountCapability

@Composable
internal fun StepColumn(content: @Composable () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .systemBarsPadding()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) { content() }
}

/** The uses a JMAP server offers this account, each starting where the core said. */
@Composable
internal fun SetupUsesStepScreen(step: UsesStep, onContinue: (List<AccountCapability>) -> Unit) {
    val ctx = LocalContext.current
    val toggles = remember(step) { UseToggles(step.offer) }
    var sent by remember(step) { mutableStateOf(false) }
    fun answer() {
        if (sent) return
        sent = true
        onContinue(toggles.chosen().uses.orEmpty())
    }
    // The account is added already: back keeps what it offers and goes on.
    BackHandler { answer() }
    StepColumn {
        Text(L10n.setup_signed_in_uses_title(ctx), style = MaterialTheme.typography.headlineSmall)
        Text(L10n.setup_signed_in_uses_note(ctx), style = MaterialTheme.typography.bodyMedium)
        UsesChoices(toggles)
        Text(
            L10n.setup_links_note(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Button(
            onClick = { answer() },
            enabled = !sent,
            modifier = Modifier.fillMaxWidth().testTag("setup-uses-continue"),
        ) { Text(if (sent) L10n.setup_signed_in_uses_saving(ctx) else L10n.setup_detect_action(ctx)) }
    }
}

/**
 * The link step: a picker per slot, a suggestion picked already and named under it, the servers
 * found beside the account's own where its server lacks them, and "Add another account".
 * Continue links what is picked; Skip, and back, link nothing.
 */
@Composable
internal fun SetupLinkStepScreen(
    step: SetupLinkStep,
    onPick: (picker: Int, option: Int?) -> Unit,
    onAddAnother: () -> Unit,
    onBeside: () -> Unit,
    onDone: (link: Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    // One answer: a second press while the first is being acted on would link and name twice.
    var answered by remember(step.account) { mutableStateOf(false) }
    fun done(link: Boolean) {
        if (answered) return
        answered = true
        onDone(link)
    }
    BackHandler { done(false) }
    StepColumn {
        Text(L10n.setup_links_title(ctx), style = MaterialTheme.typography.headlineSmall)
        Text(L10n.settings_account_links_description(ctx), style = MaterialTheme.typography.bodyMedium)
        step.pickers.forEachIndexed { index, picker ->
            StepPicker(picker, step.picked.getOrNull(index)) { onPick(index, it) }
        }
        step.beside?.let { (beside, fills) ->
            val host = beside.host
            Text(
                when (fills) {
                    Fills.CALENDAR -> L10n.setup_links_beside_calendar_note(ctx, host)
                    Fills.CONTACTS -> L10n.setup_links_beside_contacts_note(ctx, host)
                    Fills.BOTH -> L10n.setup_links_beside_note(ctx, host)
                },
                style = MaterialTheme.typography.bodyMedium,
            )
            OutlinedButton(onClick = onBeside, modifier = Modifier.testTag("setup-links-beside")) {
                Text(
                    when (fills) {
                        Fills.CALENDAR -> L10n.setup_links_beside_calendar(ctx, host)
                        Fills.CONTACTS -> L10n.setup_links_beside_contacts(ctx, host)
                        Fills.BOTH -> L10n.setup_links_beside(ctx, host)
                    },
                )
            }
        }
        Text(
            L10n.setup_links_note(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        TextButton(onClick = onAddAnother, modifier = Modifier.testTag("setup-links-add")) {
            Text(L10n.setup_links_add(ctx))
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
            OutlinedButton(
                onClick = { done(false) },
                modifier = Modifier.weight(1f).testTag("setup-links-skip"),
            ) { Text(L10n.setup_sender_name_skip(ctx)) }
            Button(
                onClick = { done(true) },
                modifier = Modifier.weight(1f).testTag("setup-links-continue"),
            ) { Text(L10n.setup_detect_action(ctx)) }
        }
    }
}

/** One slot's picker: None first, then the accounts it may name. */
@Composable
private fun StepPicker(picker: LinkPicker, picked: Int?, onPick: (Int?) -> Unit) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    Text(linkTitle(ctx, picker.slot), style = MaterialTheme.typography.labelLarge)
    Box {
        OutlinedButton(
            onClick = { expanded = true },
            modifier = Modifier.fillMaxWidth().testTag("setup-link-${picker.slot.name}"),
        ) {
            Text(
                picked?.let { picker.options.getOrNull(it)?.address } ?: L10n.settings_account_link_none(ctx),
                modifier = Modifier.weight(1f),
            )
            Icon(painter = painterResource(R.drawable.ic_arrow_drop_down), contentDescription = null)
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(L10n.settings_account_link_none(ctx)) },
                onClick = {
                    expanded = false
                    onPick(null)
                },
            )
            picker.options.forEachIndexed { index, option ->
                DropdownMenuItem(
                    text = { Text(option.address) },
                    onClick = {
                        expanded = false
                        onPick(index)
                    },
                )
            }
        }
    }
    picker.suggested?.let { picker.options.getOrNull(it) }?.let { suggestion ->
        Text(
            L10n.settings_account_link_suggested(ctx, suggestion.address),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
