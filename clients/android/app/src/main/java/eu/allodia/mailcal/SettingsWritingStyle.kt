// Settings → Writing style (docs/ai.md rules 8 to 10), drawn from the core's `WritingStylePage`:
// Learn, or the run in its place, then the library, then each account's style, as the Signatures
// category draws its library and its slots. Every control is the page's, drawn through the control
// components (docs/waiting.md); a change is a `WritingStyleIntent` that returns at once.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.WritingStyleChangeKind
import uniffi.mailcal_bindings.WritingStyleEntry
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleLearnBlock
import uniffi.mailcal_bindings.WritingStyleOpens
import uniffi.mailcal_bindings.WritingStylePage
import uniffi.mailcal_bindings.WritingStyleResult
import uniffi.mailcal_bindings.WritingStyleRun

internal const val WRITING_STYLE_LEARN_TAG = "writing-style-learn"
internal const val WRITING_STYLE_STOP_TAG = "writing-style-stop"
internal const val WRITING_STYLE_REMEDY_TAG = "writing-style-remedy"
internal const val WRITING_STYLE_RUN_TAG = "writing-style-run"
internal const val WRITING_STYLE_OUTCOME_TAG = "writing-style-outcome"

internal fun writingStyleRowTag(id: String) = "writing-style-row-$id"

internal fun writingStyleSlotTag(account: String) = "writing-style-slot-$account"

@Composable
internal fun WritingStyleCategory(ui: WritingStyleUi, onOpenEndpoint: () -> Unit) {
    val ctx = LocalContext.current
    val page = ui.page ?: return
    var revealing by remember { mutableStateOf<String?>(null) }
    var detailOf by remember { mutableStateOf<String?>(null) }
    var said by remember { mutableStateOf<String?>(null) }
    val serials = remember { OutcomeSerials(page.outcome?.serial) }
    val outcome = page.outcome
    LaunchedEffect(outcome) {
        if (outcome == null || !serials.fresh(outcome.serial)) return@LaunchedEffect
        // A slot's change is the page's own to answer; a style's is its reveal's or detail's.
        val result = outcome.result
        said = if (outcome.kind == WritingStyleChangeKind.ASSIGN && result is WritingStyleResult.Refused) {
            writingStyleRefusalText(ctx, result.reason)
        } else {
            null
        }
    }
    val remedy = { chosen: AiRemedy ->
        when (remedyAction(chosen)) {
            RemedyAction.SIGN_IN -> ui.onSignIn()
            RemedyAction.OPEN_ENDPOINT -> onOpenEndpoint()
            RemedyAction.MOVE_FLOW, RemedyAction.LEARN, RemedyAction.CHOOSE_STYLE -> ui.learn(StyleLearningIntent.Open)
        }
    }

    Text(L10n.writing_style_intro(ctx), style = MaterialTheme.typography.bodyMedium)
    Spacer(modifier = Modifier.height(12.dp))
    LearnArea(page, ui, remedy)
    Spacer(modifier = Modifier.height(16.dp))
    SettingsGroupCard(L10n.writing_style_library_heading(ctx), null) { StyleLibrary(page, ui) { entry ->
        when (entry.opens) {
            WritingStyleOpens.REVEAL -> revealing = entry.id
            WritingStyleOpens.DETAIL -> detailOf = entry.id
        }
    } }
    if (page.accounts.isNotEmpty()) {
        Spacer(modifier = Modifier.height(8.dp))
        SettingsGroupCard(L10n.writing_style_accounts_heading(ctx), null) { StyleSlots(page, ui) }
    }
    said?.let {
        Text(
            it,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.padding(top = 8.dp).testTag(WRITING_STYLE_OUTCOME_TAG),
        )
    }

    WritingStyleFlow(ui, revealing, onRevealLeft = { revealing = null }, onOpenEndpoint = onOpenEndpoint)
    detailOf?.let { id -> WritingStyleDetailDialog(id, ui) { detailOf = null } }
}

// Learn, with why it cannot be pressed and its one remedy; or, while a run goes, the run in its
// place with Stop; or a run that failed after its host was closed, above Learn, which reopens it.
@Composable
private fun LearnArea(page: WritingStylePage, ui: WritingStyleUi, remedy: (AiRemedy) -> Unit) {
    val ctx = LocalContext.current
    when (val run = page.run) {
        is WritingStyleRun.Learning -> {
            Column(modifier = Modifier.fillMaxWidth().testTag(WRITING_STYLE_RUN_TAG), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                val fraction = learnProgressFraction(run.done, run.total)
                if (fraction == null) {
                    LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                } else {
                    LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
                }
                Text(learnProgressText(ctx, run.done, run.total), style = MaterialTheme.typography.bodyMedium)
                ControlButton(L10n.learn_stop(ctx), run.stop, WRITING_STYLE_STOP_TAG, outlined = true) {
                    ui.learn(StyleLearningIntent.Stop)
                }
            }
            return
        }
        is WritingStyleRun.Failed -> FailureLine(L10n.writing_style_run_failed(ctx) + " " + aiFailureText(ctx, run.failure))
        null -> Unit
    }
    when (val blocked = page.learnBlocked) {
        WritingStyleLearnBlock.NoMailAccount -> FailureLine(L10n.writing_style_learn_no_account(ctx))
        is WritingStyleLearnBlock.CannotSend -> {
            FailureLine(aiFailureText(ctx, blocked.failure))
            blocked.remedy?.let { chosen -> RemedyButton(chosen, remedy) }
        }
        null -> Unit
    }
    ControlButton(L10n.writing_style_learn(ctx), page.learn, WRITING_STYLE_LEARN_TAG) { ui.learn(StyleLearningIntent.Open) }
}

@Composable
private fun FailureLine(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.error,
        modifier = Modifier.padding(bottom = 8.dp),
    )
}

// The remedy of a block the page says. It opens a place and starts nothing, and the page gives it no
// state of its own, so it is never held.
@Composable
private fun RemedyButton(remedy: AiRemedy, onRemedy: (AiRemedy) -> Unit) {
    val ctx = LocalContext.current
    ControlTextButton(
        label = aiRemedyText(ctx, remedy),
        control = ControlState(enabled = true, running = false),
        tag = WRITING_STYLE_REMEDY_TAG,
    ) { onRemedy(remedy) }
}

// The library: each style a row that opens its reveal or its detail, as the row says.
@Composable
private fun StyleLibrary(page: WritingStylePage, ui: WritingStyleUi, onOpen: (WritingStyleEntry) -> Unit) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    if (page.styles.isEmpty()) {
        Text(
            L10n.writing_style_empty(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    page.styles.forEach { entry ->
        ControlNavRow(
            title = entry.name,
            note = styleFactsText(ctx, entry.facts, ui.zone, locale),
            control = entry.open,
            tag = writingStyleRowTag(entry.id),
        ) { onOpen(entry) }
    }
}

// Each account's style: None, then the library.
@Composable
private fun StyleSlots(page: WritingStylePage, ui: WritingStyleUi) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        page.accounts.forEach { slot ->
            Column {
                ControlDropdown(
                    title = slot.email,
                    labels = listOf(L10n.writing_style_none(ctx)) + page.styles.map { it.name },
                    selected = page.styles.indexOfFirst { it.id == slot.selected } + 1,
                    control = slot.picker,
                    tag = writingStyleSlotTag(slot.accountId),
                ) { index -> ui.onChange(WritingStyleIntent.Assign(slot.accountId, page.styles.getOrNull(index - 1)?.id)) }
            }
        }
    }
}
