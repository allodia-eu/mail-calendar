// One learned style, shown back in plain words (docs/ai.md, "The reveal"): what was noticed per
// language, then the name, the person's own notes, and forgetting it. Opened straight after a
// learning run and from a library row. Full screen, as a signature's editor is on this platform,
// because the description runs to a screenful and a notes field wants room.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import java.text.NumberFormat
import java.util.Locale
import uniffi.mailcal_bindings.HabitRow
import uniffi.mailcal_bindings.LanguageStyleRow

// One line of the reveal; `share` is a habit's rough frequency, already a percentage for display.
internal data class RevealLine(val text: String, val share: String? = null)

// One field of a language's section. A heading of null is a line that names itself.
internal data class RevealField(val heading: String?, val lines: List<RevealLine>)

// A language's fields in the reveal's order, each left out when the model said nothing about it.
internal fun revealFields(ctx: Context, language: LanguageStyleRow, locale: Locale): List<RevealField> {
    val percent = NumberFormat.getPercentInstance(locale)
    fun habits(rows: List<HabitRow>) = rows.map { RevealLine(it.text, percent.format(it.share.toInt() / 100.0)) }
    fun text(value: String) = listOf(value).filter { it.isNotBlank() }.map { RevealLine(it) }
    fun items(values: List<String>) = values.filter { it.isNotBlank() }.map { RevealLine(it) }
    val length = if (language.typicalWords > 0u) {
        listOf(RevealLine(L10n.reveal_length(ctx, count = language.typicalWords.toInt())))
    } else {
        emptyList()
    }
    return listOf(
        RevealField(L10n.reveal_greetings(ctx), habits(language.greetings)),
        RevealField(L10n.reveal_sign_offs(ctx), habits(language.signOffs)),
        RevealField(L10n.reveal_signs_as(ctx), text(language.signsAs)),
        RevealField(L10n.reveal_register(ctx), text(language.register)),
        RevealField(null, length),
        RevealField(L10n.reveal_shape(ctx), text(language.shape)),
        RevealField(L10n.reveal_punctuation(ctx), text(language.punctuation)),
        RevealField(L10n.reveal_structure(ctx), text(language.structure)),
        RevealField(L10n.reveal_moves(ctx), text(language.moves)),
        RevealField(L10n.reveal_phrases(ctx), items(language.phrases)),
        RevealField(L10n.reveal_avoid(ctx), items(language.avoid)),
    ).filter { it.lines.isNotEmpty() }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun WritingStyleRevealDialog(id: String, actions: WritingStyleActions, onClose: () -> Unit) {
    val ctx = LocalContext.current
    val locale = LocalConfiguration.current.locales[0]
    val detail = remember(id) { actions.detail(id) }
    // Forgotten from elsewhere while this was on its way up: there is nothing left to show.
    if (detail == null) {
        LaunchedEffect(id) { onClose() }
        return
    }
    var name by remember(id) { mutableStateOf(detail.row.name) }
    var notes by remember(id) { mutableStateOf(detail.notes) }
    var forgetting by remember { mutableStateOf(false) }

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
                        title = { Text(detail.row.name) },
                        navigationIcon = {
                            IconButton(onClick = onClose) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_close),
                                    contentDescription = L10n.action_close(ctx),
                                )
                            }
                        },
                        actions = {
                            // A style with no name cannot be told apart in the account pickers.
                            IconButton(
                                enabled = name.isNotBlank(),
                                onClick = {
                                    actions.save(id, name.trim(), notes)
                                    onClose()
                                },
                            ) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_check),
                                    contentDescription = L10n.reveal_save(ctx),
                                )
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
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(L10n.reveal_title(ctx), style = MaterialTheme.typography.titleLarge)
                    detail.languages.forEach { language ->
                        Text(
                            L10n.languageName(ctx, language.language),
                            style = MaterialTheme.typography.titleMedium,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                        revealFields(ctx, language, locale).forEach { RevealFieldView(it) }
                    }
                    OutlinedTextField(
                        value = name,
                        onValueChange = { name = it },
                        modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
                        singleLine = true,
                        label = { Text(L10n.writing_style_name_label(ctx)) },
                    )
                    OutlinedTextField(
                        value = notes,
                        onValueChange = { notes = it },
                        modifier = Modifier.fillMaxWidth(),
                        minLines = 3,
                        label = { Text(L10n.writing_style_notes(ctx)) },
                        supportingText = { Text(L10n.writing_style_notes_hint(ctx)) },
                    )
                    TextButton(
                        onClick = { forgetting = true },
                        colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
                    ) {
                        Text(L10n.writing_style_forget(ctx))
                    }
                }
            }
            if (forgetting) {
                AlertDialog(
                    onDismissRequest = { forgetting = false },
                    title = { Text(L10n.writing_style_forget_title(ctx, detail.row.name)) },
                    text = { Text(L10n.writing_style_forget_message(ctx)) },
                    confirmButton = {
                        TextButton(
                            onClick = {
                                forgetting = false
                                actions.forget(id)
                                onClose()
                            },
                            colors = ButtonDefaults.textButtonColors(
                                contentColor = MaterialTheme.colorScheme.error,
                            ),
                        ) {
                            Text(L10n.writing_style_forget(ctx))
                        }
                    },
                    dismissButton = {
                        TextButton(onClick = { forgetting = false }) { Text(L10n.action_cancel(ctx)) }
                    },
                )
            }
        }
    }
}

@Composable
private fun RevealFieldView(field: RevealField) {
    Column(modifier = Modifier.fillMaxWidth()) {
        field.heading?.let { Text(it, style = MaterialTheme.typography.labelLarge) }
        field.lines.forEach { line ->
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(line.text, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
                line.share?.let {
                    Text(
                        it,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}
