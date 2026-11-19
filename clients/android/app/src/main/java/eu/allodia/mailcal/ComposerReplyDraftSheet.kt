// Draft a reply's instruction input and its three questions (docs/ai.md rules 19, 22 and 24). The
// input is a bottom sheet, the platform's control for one line and what it needs (the flow host
// table): the line, the core's suggestions under it, Draft. While the core holds Draft for a
// reason the person can act on, the reason and its remedy stand in the line's place. Drawn inside
// the composer's own Dialog, so back reaches each before the composer.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ReplyDraftSuggestion

internal const val REPLY_DRAFT_SHEET_TAG = "reply-draft-sheet"
internal const val REPLY_DRAFT_LINE_TAG = "reply-draft-line"
internal const val REPLY_DRAFT_DRAFT_TAG = "reply-draft-draft"
internal const val REPLY_DRAFT_REPLACE_TAG = "reply-draft-replace"
internal const val REPLY_DRAFT_ARRIVED_TAG = "reply-draft-arrived"
internal const val REPLY_DRAFT_SEND_TAG = "reply-draft-send-question"

internal fun replyDraftSuggestionTag(suggestion: ReplyDraftSuggestion) = "reply-draft-suggestion-$suggestion"

/** A suggestion as its chip says it, which is also what it fills the line with. */
internal fun replyDraftSuggestionText(ctx: Context, suggestion: ReplyDraftSuggestion): String =
    when (suggestion) {
        ReplyDraftSuggestion.YES -> L10n.composer_draft_chip_yes(ctx)
        ReplyDraftSuggestion.NO -> L10n.composer_draft_chip_no(ctx)
        ReplyDraftSuggestion.MORE_INFORMATION -> L10n.composer_draft_chip_more_info(ctx)
        ReplyDraftSuggestion.HOLDING -> L10n.composer_draft_chip_later(ctx)
    }

@Composable
internal fun ComposerReplyDraftDialogs(host: ReplyDraftHost, submit: () -> Unit) {
    val ctx = LocalContext.current
    if (host.sheetOpen) InstructionSheet(host)
    if (host.askingReplace) {
        ReplaceQuestion(REPLY_DRAFT_REPLACE_TAG, host::answerReplace)
    } else if (host.record.replaceArrived) {
        ReplaceQuestion(REPLY_DRAFT_ARRIVED_TAG, host::answerArrived)
    }
    val question = host.record.sendQuestion
    if (host.askingSend && question != null) {
        AlertDialog(
            onDismissRequest = { host.answerSend(sendAnyway = false, submit) },
            title = { Text(L10n.composer_send_open_title(ctx)) },
            text = { Text(L10n.composer_send_open_message(ctx, question.open.toInt()), modifier = Modifier.testTag("$REPLY_DRAFT_SEND_TAG-message")) },
            confirmButton = {
                TextButton(
                    onClick = { host.answerSend(sendAnyway = true, submit) },
                    modifier = Modifier.testTag("$REPLY_DRAFT_SEND_TAG-anyway"),
                ) { Text(L10n.composer_send_anyway(ctx)) }
            },
            dismissButton = {
                TextButton(
                    onClick = { host.answerSend(sendAnyway = false, submit) },
                    modifier = Modifier.testTag("$REPLY_DRAFT_SEND_TAG-keep"),
                ) { Text(L10n.composer_keep_editing(ctx)) }
            },
        )
    }
}

// Whether the draft replaces what the person wrote above the signature and the quote; back or a
// tap outside keeps their text.
@Composable
private fun ReplaceQuestion(tag: String, onAnswer: (Boolean) -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = { onAnswer(false) },
        title = { Text(L10n.composer_draft_replace_title(ctx)) },
        text = { Text(L10n.composer_draft_replace_message(ctx)) },
        confirmButton = {
            TextButton(onClick = { onAnswer(true) }, modifier = Modifier.testTag("$tag-confirm")) {
                Text(L10n.composer_draft_replace(ctx))
            }
        },
        dismissButton = {
            TextButton(onClick = { onAnswer(false) }, modifier = Modifier.testTag("$tag-cancel")) {
                Text(L10n.action_cancel(ctx))
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun InstructionSheet(host: ReplyDraftHost) {
    val ctx = LocalContext.current
    val record = host.record
    val sheet = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    val focus = remember { FocusRequester() }
    ModalBottomSheet(
        onDismissRequest = { host.sheetOpen = false },
        sheetState = sheet,
        modifier = Modifier.testTag(REPLY_DRAFT_SHEET_TAG),
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().navigationBarsPadding().padding(horizontal = 24.dp).padding(bottom = 16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(L10n.composer_draft_reply(ctx), style = MaterialTheme.typography.titleLarge)
            val held = record.heldBy
            if (held != null) {
                Text(aiFailureText(ctx, held.failure), style = MaterialTheme.typography.bodyLarge)
                held.remedy?.let { offered ->
                    ControlButton(aiRemedyText(ctx, offered.remedy), offered.control, REPLY_DRAFT_REMEDY_TAG) {
                        host.sheetOpen = false
                        host.remedy(offered.remedy)
                    }
                }
                return@Column
            }
            OutlinedTextField(
                value = host.line,
                onValueChange = { host.line = it },
                singleLine = true,
                placeholder = { Text(L10n.composer_draft_intent_hint(ctx)) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { if (record.draft.enabled) host.pressDraft() }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus).testTag(REPLY_DRAFT_LINE_TAG),
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(0.dp)) {
                record.suggestions.forEach { suggestion ->
                    val words = replyDraftSuggestionText(ctx, suggestion)
                    SuggestionChip(
                        onClick = { host.line = words },
                        label = { Text(words) },
                        modifier = Modifier.testTag(replyDraftSuggestionTag(suggestion)),
                    )
                }
            }
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = { host.sheetOpen = false }) { Text(L10n.action_cancel(ctx)) }
                ControlButton(L10n.composer_draft_create(ctx), record.draft, REPLY_DRAFT_DRAFT_TAG) { host.pressDraft() }
            }
        }
    }
    LaunchedEffect(Unit) { if (record.heldBy == null) runCatching { focus.requestFocus() } }
}
