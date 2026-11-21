// Settings → Writing style (docs/ai.md rules 8 to 10), drawn from the core's `WritingStylePage`:
// Learn, or the run in its place, then the library, then each account's style, as the Signatures
// category draws its library and its slots. Every control is the page's, drawn through the control
// components (docs/waiting.md); a change is a `WritingStyleIntent` that returns at once.
//
// An empty library is an invitation rather than an empty list: the feature in one picture and a
// sentence, with Learn under it.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.AiRemedyControl
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
internal const val WRITING_STYLE_EMPTY_TAG = "writing-style-empty"

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
    // A run that failed with its host closed: the flow takes its remedy first, as it does in the host
    // (docs/ai.md rule 11), opening on the step a remedy that is one names; one that leads elsewhere
    // is then opened there.
    val runRemedy = { chosen: AiRemedy ->
        if (chosen == AiRemedy.CHOOSE_ACCOUNT || chosen == AiRemedy.CHOOSE_RANGE) ui.learn(StyleLearningIntent.Open)
        ui.learn(StyleLearningIntent.Remedy)
        when (remedyAction(chosen)) {
            RemedyAction.SIGN_IN -> ui.onSignIn()
            RemedyAction.OPEN_ENDPOINT -> onOpenEndpoint()
            RemedyAction.MOVE_FLOW, RemedyAction.LEARN, RemedyAction.CHOOSE_STYLE -> Unit
        }
    }

    if (page.styles.isEmpty() && page.run == null) {
        EmptyLibrary(page, ui, remedy)
    } else {
        Text(
            L10n.writing_style_intro(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(modifier = Modifier.height(16.dp))
        LearnArea(page, ui, remedy, runRemedy)
        Spacer(modifier = Modifier.height(16.dp))
        SettingsGroupCard(L10n.writing_style_library_heading(ctx), null) {
            StyleLibrary(page, ui) { entry ->
                when (entry.opens) {
                    WritingStyleOpens.REVEAL -> revealing = entry.id
                    WritingStyleOpens.DETAIL -> detailOf = entry.id
                }
            }
        }
        if (page.accounts.isNotEmpty()) {
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(L10n.writing_style_accounts_heading(ctx), null) { StyleSlots(page, ui) }
        }
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

// The feature in one picture and a sentence, with Learn, or why it cannot be pressed, under it.
@Composable
private fun EmptyLibrary(page: WritingStylePage, ui: WritingStyleUi, remedy: (AiRemedy) -> Unit) {
    val ctx = LocalContext.current
    val colours = MaterialTheme.colorScheme
    Column(
        modifier = Modifier.fillMaxWidth().padding(top = 24.dp, bottom = 8.dp).testTag(WRITING_STYLE_EMPTY_TAG),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Box(
            modifier = Modifier.size(136.dp).background(colours.secondaryContainer.copy(alpha = 0.5f), CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            Box(
                modifier = Modifier.size(104.dp).background(colours.primaryContainer, CircleShape),
                contentAlignment = Alignment.Center,
            ) {
                Icon(
                    painterResource(R.drawable.ic_text_format),
                    contentDescription = null,
                    tint = colours.onPrimaryContainer,
                    modifier = Modifier.size(52.dp),
                )
            }
        }
        Text(
            L10n.settings_category_writing_style_summary(ctx),
            style = MaterialTheme.typography.headlineSmall,
            textAlign = TextAlign.Center,
            modifier = Modifier.semantics { heading() },
        )
        Text(
            L10n.writing_style_intro(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = colours.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Blocked(page, remedy, centred = true)
        ControlButton(L10n.writing_style_learn(ctx), page.learn, WRITING_STYLE_LEARN_TAG) { ui.learn(StyleLearningIntent.Open) }
    }
}

// Learn, with why it cannot be pressed and its one remedy; or, while a run goes, the run in its
// place with Stop; or a run that failed after its host was closed, above Learn, which reopens it.
@Composable
private fun LearnArea(
    page: WritingStylePage,
    ui: WritingStyleUi,
    remedy: (AiRemedy) -> Unit,
    runRemedy: (AiRemedy) -> Unit,
) {
    val ctx = LocalContext.current
    when (val run = page.run) {
        is WritingStyleRun.Learning -> {
            Card(
                modifier = Modifier.fillMaxWidth().testTag(WRITING_STYLE_RUN_TAG),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer),
            ) {
                Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        learnProgressText(ctx, run.done, run.total),
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                    )
                    Text(
                        L10n.learn_progress_account(ctx, run.email),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                    )
                    val fraction = learnProgressFraction(run.done, run.total)
                    if (fraction == null) {
                        LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                    } else {
                        LinearProgressIndicator(progress = { fraction }, modifier = Modifier.fillMaxWidth())
                    }
                    ControlButton(L10n.learn_stop(ctx), run.stop, WRITING_STYLE_STOP_TAG, outlined = true) {
                        ui.learn(StyleLearningIntent.Stop)
                    }
                }
            }
            return
        }
        is WritingStyleRun.Failed -> {
            FailureLine(L10n.writing_style_run_failed(ctx) + " " + aiFailureText(ctx, run.failure))
            run.remedy?.let { RemedyButton(it, runRemedy) }
        }
        null -> Unit
    }
    Blocked(page, remedy, centred = false)
    ControlButton(L10n.writing_style_learn(ctx), page.learn, WRITING_STYLE_LEARN_TAG) { ui.learn(StyleLearningIntent.Open) }
}

// Why Learn cannot be pressed, with its one remedy.
@Composable
private fun Blocked(page: WritingStylePage, remedy: (AiRemedy) -> Unit, centred: Boolean) {
    val ctx = LocalContext.current
    when (val blocked = page.learnBlocked) {
        WritingStyleLearnBlock.NoMailAccount -> FailureLine(L10n.writing_style_learn_no_account(ctx), centred)
        is WritingStyleLearnBlock.CannotSend -> {
            FailureLine(aiFailureText(ctx, blocked.failure), centred)
            blocked.remedy?.let { RemedyButton(it, remedy) }
        }
        null -> Unit
    }
}

@Composable
private fun FailureLine(text: String, centred: Boolean = false) {
    Text(
        text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.error,
        textAlign = if (centred) TextAlign.Center else TextAlign.Start,
        modifier = Modifier.padding(bottom = 4.dp),
    )
}

// A failure's remedy, from its control: held while a change to the library runs, never running.
@Composable
private fun RemedyButton(offered: AiRemedyControl, onRemedy: (AiRemedy) -> Unit) {
    val ctx = LocalContext.current
    ControlTextButton(
        label = aiRemedyText(ctx, offered.remedy),
        control = offered.control,
        tag = WRITING_STYLE_REMEDY_TAG,
    ) { onRemedy(offered.remedy) }
}

// The library: each style a row that opens its reveal or its detail, as the row says.
@Composable
private fun StyleLibrary(page: WritingStylePage, ui: WritingStyleUi, onOpen: (WritingStyleEntry) -> Unit) {
    val ctx = LocalContext.current
    val locale = catalogLocale(ctx)
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
            icon = R.drawable.ic_text_format,
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
