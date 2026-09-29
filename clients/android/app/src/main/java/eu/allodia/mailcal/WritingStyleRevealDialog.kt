// One learned style, shown back in plain words (docs/ai.md, "Learning" step 5): what was read,
// four pages about one language at a time, then the name, the person's own notes, and forgetting
// it. Opened straight after a learning run and from a library row. The descriptive fields arrive in
// the language of the interface and the habits in the language of the mail; this shows both as the
// core wrote them.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import java.time.ZoneId
import uniffi.mailcal_bindings.AccountWritingStyleRow
import uniffi.mailcal_bindings.LanguageStyleRow

@Composable
internal fun WritingStyleRevealDialog(
    id: String,
    actions: WritingStyleActions,
    // The configured accounts, to name the one the style was learned from.
    accounts: List<AccountWritingStyleRow>,
    zone: ZoneId,
    onClose: () -> Unit,
) {
    val ctx = LocalContext.current
    val detail = remember(id) { actions.detail(id) }
    // Forgotten from elsewhere while this was on its way up: there is nothing left to show.
    if (detail == null) {
        LaunchedEffect(id) { onClose() }
        return
    }
    var name by remember(id) { mutableStateOf(detail.row.name) }
    var notes by remember(id) { mutableStateOf(detail.notes) }
    var forgetting by remember { mutableStateOf(false) }
    var language by remember(id) { mutableIntStateOf(0) }
    val pager = remember(id) { WizardPager(RevealStep.entries.size) }
    val step = RevealStep.entries[pager.index]
    val languages = detail.languages
    val style = languages.getOrNull(language)

    WizardSheet(
        pager = pager,
        onClose = onClose,
        showsBack = !pager.isFirst,
        // A style with no name cannot be told apart in the account pickers.
        primary = if (pager.isLast) {
            WizardPrimary.Action(L10n.reveal_save(ctx), enabled = name.isNotBlank()) {
                actions.save(id, name.trim(), notes)
                onClose()
            }
        } else {
            WizardPrimary.Next(enabled = true)
        },
        allowsJump = true,
        topBar = {
            // A single language has nothing to choose between, so the control is drawn only for two
            // or more, and only over the pages about one language.
            if (languages.size > 1) {
                LanguageSwitcher(languages, language, visible = step.isPerLanguage) { language = it }
            }
        },
        dialogs = {
            if (forgetting) {
                ForgetStyleDialog(
                    name = detail.row.name,
                    onForget = {
                        forgetting = false
                        actions.forget(id)
                        onClose()
                    },
                    onCancel = { forgetting = false },
                )
            }
        },
    ) { index ->
        // Keyed by the language, so a page about another language plays in afresh.
        when (RevealStep.entries[index]) {
            RevealStep.READ -> RevealReadPage(detail, revealSourceAddress(detail.row, accounts), zone)
            RevealStep.LETTER -> style?.let { key(language) { RevealLetterPage(it) } }
            RevealStep.HABITS -> style?.let { key(language) { RevealHabitsPage(it) } }
            RevealStep.VOICE -> style?.let { key(language) { RevealVoicePage(it) } }
            RevealStep.PHRASES -> style?.let { key(language) { RevealPhrasesPage(it) } }
            RevealStep.NAME -> RevealNamePage(
                name = name,
                onName = { name = it },
                notes = notes,
                onNotes = { notes = it },
                onForget = { forgetting = true },
            )
        }
    }
}

@Composable
private fun LanguageSwitcher(
    languages: List<LanguageStyleRow>,
    selected: Int,
    visible: Boolean,
    onSelect: (Int) -> Unit,
) {
    val ctx = LocalContext.current
    val label = L10n.a11y_reveal_language(ctx)
    SingleChoiceSegmentedButtonRow(
        modifier = if (visible) {
            Modifier.semantics { contentDescription = label }
        } else {
            Modifier.alpha(0f).clearAndSetSemantics {}
        },
    ) {
        languages.forEachIndexed { index, style ->
            SegmentedButton(
                selected = index == selected,
                onClick = { onSelect(index) },
                enabled = visible,
                shape = SegmentedButtonDefaults.itemShape(index, languages.size),
                icon = {},
            ) {
                Text(L10n.languageName(ctx, style.language), maxLines = 1)
            }
        }
    }
}

@Composable
private fun RevealNamePage(
    name: String,
    onName: (String) -> Unit,
    notes: String,
    onNotes: (String) -> Unit,
    onForget: () -> Unit,
) {
    val ctx = LocalContext.current
    WizardPage(
        title = L10n.reveal_step_name(ctx),
        bottom = {
            TextButton(
                onClick = onForget,
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
            ) {
                Text(L10n.writing_style_forget(ctx))
            }
        },
    ) {
        OutlinedTextField(
            value = name,
            onValueChange = onName,
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text(L10n.writing_style_name_label(ctx)) },
        )
        OutlinedTextField(
            value = notes,
            onValueChange = onNotes,
            modifier = Modifier.fillMaxWidth(),
            minLines = 4,
            label = { Text(L10n.writing_style_notes(ctx)) },
            supportingText = { Text(L10n.writing_style_notes_hint(ctx)) },
        )
    }
}

@Composable
private fun ForgetStyleDialog(name: String, onForget: () -> Unit, onCancel: () -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onCancel,
        title = { Text(L10n.writing_style_forget_title(ctx, name)) },
        text = { Text(L10n.writing_style_forget_message(ctx)) },
        confirmButton = {
            TextButton(
                onClick = onForget,
                colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
            ) {
                Text(L10n.writing_style_forget(ctx))
            }
        },
        dismissButton = {
            TextButton(onClick = onCancel) { Text(L10n.action_cancel(ctx)) }
        },
    )
}
