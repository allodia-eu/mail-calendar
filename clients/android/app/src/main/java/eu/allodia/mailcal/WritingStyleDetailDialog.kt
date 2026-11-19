// An existing style's detail (docs/ai.md rule 16): what it was learned from, the reveal's sections
// on one page, its name with Rename, its notes, and Forget, asked first and naming the accounts that
// draft in it. A full-screen dialog over Settings, as a signature's editor is; it closes once the
// style is forgotten, here or elsewhere.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import uniffi.mailcal_bindings.WritingStyleChangeKind
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleResult
import uniffi.mailcal_bindings.WritingStyleRevealPage
import uniffi.mailcal_bindings.WritingStyleSection

internal const val DETAIL_NAME_TAG = "style-detail-name"
internal const val DETAIL_RENAME_TAG = "style-detail-rename"
internal const val DETAIL_NOTES_TAG = "style-detail-notes"
internal const val DETAIL_FORGET_TAG = "style-detail-forget"
internal const val DETAIL_FORGET_CONFIRM_TAG = "style-detail-forget-confirm"
internal const val DETAIL_CLOSE_TAG = "style-detail-close"

private fun WritingStyleSection.page(): WritingStyleRevealPage = when (this) {
    WritingStyleSection.LETTER -> WritingStyleRevealPage.LETTER
    WritingStyleSection.HABITS -> WritingStyleRevealPage.HABITS
    WritingStyleSection.VOICE -> WritingStyleRevealPage.VOICE
    WritingStyleSection.PHRASES -> WritingStyleRevealPage.PHRASES
}

/** Forget's question: what it does, then the accounts that draft in the style now. */
internal fun forgetMessage(ctx: Context, detail: WritingStyleDetail): String =
    listOfNotNull(
        L10n.writing_style_forget_message(ctx),
        detail.draftingAccounts.takeIf { it.isNotEmpty() }
            ?.let { L10n.writing_style_forget_accounts(ctx, it.joinToString(", ")) },
    ).joinToString("\n\n")

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun WritingStyleDetailDialog(id: String, ui: WritingStyleUi, onClose: () -> Unit) {
    val ctx = LocalContext.current
    val detail = remember(id, ui.version) { ui.detail(id) }
    val close by rememberUpdatedState(onClose)
    if (detail == null) {
        LaunchedEffect(id) { close() }
        return
    }
    var name by remember(id) { mutableStateOf(detail.name) }
    var language by remember(id) { mutableIntStateOf(0) }
    var forgetting by remember { mutableStateOf(false) }
    var refused by remember { mutableStateOf<WritingStyleRefusal?>(null) }
    val serials = remember(id) { OutcomeSerials(ui.page?.outcome?.serial) }
    val outcome = ui.page?.outcome
    LaunchedEffect(outcome) {
        if (outcome == null || outcome.style != id || !serials.fresh(outcome.serial)) return@LaunchedEffect
        when (val result = outcome.result) {
            is WritingStyleResult.Refused -> refused = result.reason
            WritingStyleResult.Done, WritingStyleResult.NotWritten ->
                if (outcome.kind == WritingStyleChangeKind.FORGET) close()
        }
    }

    Dialog(
        onDismissRequest = onClose,
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        SystemBarsMatchTheme()
        Surface(modifier = Modifier.fillMaxSize()) {
            Scaffold(
                modifier = Modifier.imePadding(),
                topBar = {
                    TopAppBar(
                        title = { Text(detail.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        navigationIcon = {
                            IconButton(onClick = onClose, modifier = Modifier.testTag(DETAIL_CLOSE_TAG)) {
                                Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = L10n.a11y_back(ctx))
                            }
                        },
                    )
                },
            ) { padding ->
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(padding)
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = FLOW_GUTTER, vertical = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(20.dp),
                ) {
                    StyleFactsView(detail.facts, ui.zone, shown = true)
                    HorizontalDivider()
                    DetailName(detail, name, { name = it; refused = null }, refused, ui)
                    ControlTextField(
                        title = L10n.writing_style_notes(ctx),
                        text = detail.notes,
                        control = detail.notesControl,
                        tag = DETAIL_NOTES_TAG,
                        minLines = 3,
                        supporting = L10n.writing_style_notes_hint(ctx),
                    ) { ui.onChange(WritingStyleIntent.SetNotes(id, it)) }
                    LanguageControl(detail.languages, language) { language = it }
                    val chosen = detail.languages.getOrNull(language)
                    detail.sections.map { it.page() }.forEach { page ->
                        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                            Text(
                                revealPageTitle(ctx, page),
                                style = MaterialTheme.typography.titleMedium,
                                modifier = Modifier.semantics { heading() },
                            )
                            LanguageSection(page, chosen, shown = true)
                        }
                    }
                    HorizontalDivider()
                    ControlButton(
                        label = L10n.writing_style_forget(ctx),
                        // Opens the question and starts nothing; the question's answer is the change.
                        control = detail.forget,
                        tag = DETAIL_FORGET_TAG,
                        destructive = true,
                        outlined = true,
                    ) { forgetting = true }
                }
            }
            if (forgetting) {
                ConfirmDialog(
                    title = L10n.writing_style_forget_title(ctx, detail.name),
                    message = forgetMessage(ctx, detail),
                    action = L10n.writing_style_forget(ctx),
                    tag = DETAIL_FORGET_CONFIRM_TAG,
                ) { confirmed ->
                    forgetting = false
                    if (confirmed) ui.onChange(WritingStyleIntent.Forget(id))
                }
            }
        }
    }
}

// The name, and Rename beside it: held while the core refuses the name typed, and while it is the
// name the style already has.
@Composable
private fun DetailName(
    detail: WritingStyleDetail,
    name: String,
    onName: (String) -> Unit,
    refused: WritingStyleRefusal?,
    ui: WritingStyleUi,
) {
    val ctx = LocalContext.current
    val refusal = ui.nameRefusal(name) ?: refused
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(modifier = Modifier.weight(1f)) {
            HeldFields(detail.rename) { enabled ->
                OutlinedTextField(
                    value = name,
                    onValueChange = onName,
                    enabled = enabled,
                    singleLine = true,
                    isError = refusal != null,
                    label = { Text(L10n.writing_style_name_label(ctx)) },
                    supportingText = refusal?.let { { Text(writingStyleRefusalText(ctx, it)) } },
                    modifier = Modifier.fillMaxWidth().testTag(DETAIL_NAME_TAG),
                )
            }
        }
        ControlButton(
            label = L10n.writing_style_rename(ctx),
            control = detail.rename,
            tag = DETAIL_RENAME_TAG,
            ready = ui.nameRefusal(name) == null && name != detail.name,
        ) { ui.onChange(WritingStyleIntent.Rename(detail.id, name)) }
    }
}
