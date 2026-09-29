// Draft a reply's controls in the composer: the app-bar button beside the signature's, the small
// dialog that takes an intent, the question before a draft replaces what was written, and the
// lines under the address fields while it drafts and after. The state is ComposerReplyDraft.kt;
// the card a draft brings is ComposerDraftCard.kt.
package eu.allodia.mailcal

import android.webkit.WebView
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.PlainTooltip
import androidx.compose.material3.SuggestionChip
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TooltipAnchorPosition
import androidx.compose.material3.TooltipBox
import androidx.compose.material3.TooltipDefaults
import androidx.compose.material3.rememberTooltipState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.unit.dp

// The composer's draft control, or null when this composer offers none: not a reply, AI not
// available, or no message to answer.
@Composable
internal fun rememberReplyDraft(
    mode: RichComposeMode,
    target: ReplyTarget?,
    webView: () -> WebView?,
): ReplyDraftControl? {
    val drafting = LocalReplyDrafting.current
    if (drafting == null || target == null || !replyDraftOffered(mode)) return null
    return remember(drafting, target) {
        ReplyDraftControl(target, drafting, webViewDraftEditor(webView))
    }
}

// Disabled while the From account has no style, and then says why on a long press and to a
// screen reader, since a greyed icon alone explains nothing.
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ComposerDraftAction(control: ReplyDraftControl, from: String?) {
    val ctx = LocalContext.current
    val hasStyle = control.hasStyle(from)
    val hint = L10n.ai_error_no_style(ctx)
    val button = @Composable {
        IconButton(
            onClick = control::open,
            enabled = hasStyle && !control.busy,
            modifier = if (hasStyle) Modifier else Modifier.semantics { stateDescription = hint },
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_stylus_note),
                contentDescription = L10n.composer_draft_reply(ctx),
            )
        }
    }
    if (hasStyle) {
        button()
    } else {
        TooltipBox(
            positionProvider = TooltipDefaults.rememberTooltipPositionProvider(TooltipAnchorPosition.Below),
            tooltip = { PlainTooltip { Text(hint) } },
            state = rememberTooltipState(),
        ) {
            button()
        }
    }
}

// The intent dialog, the replace question and Send's question about open items. Drawn inside the
// composer's own Dialog, so back reaches them rather than the composer under them.
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ComposerDraftDialogs(control: ReplyDraftControl, from: String?) {
    val ctx = LocalContext.current
    if (control.sheetOpen) {
        val chips = listOf(
            L10n.composer_draft_chip_yes(ctx),
            L10n.composer_draft_chip_no(ctx),
            L10n.composer_draft_chip_more_info(ctx),
            L10n.composer_draft_chip_later(ctx),
        )
        AlertDialog(
            onDismissRequest = control::dismiss,
            title = { Text(L10n.composer_draft_reply(ctx)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = control.intent,
                        onValueChange = { control.intent = it },
                        modifier = Modifier.fillMaxWidth(),
                        singleLine = true,
                        placeholder = { Text(L10n.composer_draft_intent_hint(ctx)) },
                    )
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        chips.forEach { chip ->
                            SuggestionChip(onClick = { control.intent = chip }, label = { Text(chip) })
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { control.create(from) }) {
                    Text(L10n.composer_draft_create(ctx))
                }
            },
            dismissButton = {
                TextButton(onClick = control::dismiss) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
    if (control.confirmingReplace) {
        AlertDialog(
            onDismissRequest = control::keep,
            title = { Text(L10n.composer_draft_replace_title(ctx)) },
            text = { Text(L10n.composer_draft_replace_message(ctx)) },
            confirmButton = {
                TextButton(onClick = control::replace) { Text(L10n.composer_draft_replace(ctx)) }
            },
            dismissButton = {
                TextButton(onClick = control::keep) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
    ComposerSendOpenQuestion(control)
}

// Under the address fields, above the body: drafting, what went wrong, or the gaps to fill, then
// the card a draft brought.
@Composable
internal fun ComposerDraftStatus(control: ReplyDraftControl) {
    val ctx = LocalContext.current
    val failure = control.failure
    when {
        control.busy -> Row(
            modifier = Modifier.padding(bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
            Text(L10n.composer_drafting(ctx), style = MaterialTheme.typography.bodySmall)
        }
        failure != null -> Text(
            text = writingStyleFailureText(ctx, failure, control.route),
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(bottom = 8.dp),
        )
        control.checkBrackets -> Text(
            text = L10n.composer_draft_check_brackets(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(bottom = 8.dp),
        )
    }
    ComposerDraftCard(control)
}
