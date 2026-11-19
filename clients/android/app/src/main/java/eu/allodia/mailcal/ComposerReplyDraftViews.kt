// Draft a reply among the composer's notes (docs/ai.md rules 17, 18, 20 and 23), under the address
// fields and above the message: the control, then what the record says now. A drafting line with
// Stop, a failure with its one remedy, or the card a draft brought, which is never part of the mail.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AiFailure
import uniffi.mailcal_bindings.ReplyDraftItem
import uniffi.mailcal_bindings.ReplyDraftState
import uniffi.mailcal_bindings.ReplyDraftTaskKind

internal const val REPLY_DRAFT_CONTROL_TAG = "reply-draft-control"
internal const val REPLY_DRAFT_STOP_TAG = "reply-draft-stop"
internal const val REPLY_DRAFT_REMEDY_TAG = "reply-draft-remedy"
internal const val REPLY_DRAFT_CARD_TAG = "reply-draft-card"
internal const val REPLY_DRAFT_DRAFTING_TAG = "reply-draft-drafting"

internal fun replyDraftItemTag(index: UInt) = "reply-draft-item-$index"

/** The words an item is shown with: a gap as the reply carries it, the rest as the core wrote them. */
internal fun replyDraftItemText(ctx: Context, item: ReplyDraftItem): String = when (item.kind) {
    ReplyDraftTaskKind.FILL_IN -> L10n.composer_task_fill_in(ctx, item.text)
    ReplyDraftTaskKind.ATTACH, ReplyDraftTaskKind.DO -> item.text
}

@Composable
internal fun ComposerReplyDraftNotes(host: ReplyDraftHost, leave: () -> Unit) {
    val ctx = LocalContext.current
    val record = host.record
    val reduced = rememberReducedMotion()
    Column(modifier = Modifier.padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        ControlAssistChip(
            label = L10n.composer_draft_reply(ctx),
            icon = R.drawable.ic_stylus_note,
            control = record.control,
            tag = REPLY_DRAFT_CONTROL_TAG,
        ) { host.open() }
        AnimatedContent(
            targetState = record.state,
            contentKey = { it::class },
            transitionSpec = {
                if (reduced) {
                    fadeIn(tween(0)) togetherWith fadeOut(tween(0))
                } else {
                    fadeIn(tween(220, 80)) togetherWith fadeOut(tween(120))
                }
            },
            label = "reply-draft",
        ) { state ->
            when (state) {
                is ReplyDraftState.Unavailable -> if (state.failure == AiFailure.MessageGone) {
                    Text(
                        aiFailureText(ctx, state.failure),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                ReplyDraftState.Ready -> Unit
                ReplyDraftState.Drafting -> DraftingLine(host)
                is ReplyDraftState.Drafted -> DraftCard(host, state)
                is ReplyDraftState.Failed -> NoteSurface(MaterialTheme.colorScheme.errorContainer) {
                    Text(
                        aiFailureText(ctx, state.failure),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        modifier = Modifier.weight(1f),
                    )
                    state.remedy?.let { offered ->
                        ControlTextButton(aiRemedyText(ctx, offered.remedy), offered.control, REPLY_DRAFT_REMEDY_TAG) {
                            host.remedy(offered.remedy, leave)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun NoteSurface(colour: Color, content: @Composable RowScope.() -> Unit) {
    Surface(color = colour, shape = RoundedCornerShape(16.dp), modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.padding(start = 16.dp, end = 8.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            content = content,
        )
    }
}

@Composable
private fun DraftingLine(host: ReplyDraftHost) {
    val ctx = LocalContext.current
    NoteSurface(MaterialTheme.colorScheme.secondaryContainer) {
        CircularProgressIndicator(modifier = Modifier.size(18.dp), strokeWidth = 2.dp)
        Text(
            L10n.composer_drafting(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSecondaryContainer,
            modifier = Modifier.weight(1f).padding(start = 12.dp).testTag(REPLY_DRAFT_DRAFTING_TAG),
        )
        ControlTextButton(L10n.composer_draft_stop(ctx), host.record.stop, REPLY_DRAFT_STOP_TAG) { host.stop() }
    }
}

// What the message asks, then what is left to do before sending. Beside the mail, never in it.
@Composable
private fun DraftCard(host: ReplyDraftHost, drafted: ReplyDraftState.Drafted) {
    val ctx = LocalContext.current
    val colours = MaterialTheme.colorScheme
    Surface(
        color = colours.surfaceContainerHigh,
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier.fillMaxWidth().testTag(REPLY_DRAFT_CARD_TAG),
    ) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (drafted.summary.isNotEmpty()) {
                CardHeading(L10n.composer_draft_summary_heading(ctx), R.drawable.ic_stylus_note)
                Text(drafted.summary, style = MaterialTheme.typography.bodyMedium)
            }
            if (drafted.checklist.isNotEmpty()) {
                CardHeading(L10n.composer_checklist_heading(ctx), R.drawable.ic_task_alt)
                drafted.checklist.forEach { item ->
                    ControlCheckRow(
                        checked = item.done,
                        control = item.tick,
                        tag = replyDraftItemTag(item.index),
                        content = {
                            Icon(
                                painterResource(kindIcon(item.kind)),
                                contentDescription = null,
                                tint = colours.onSurfaceVariant,
                                modifier = Modifier.size(18.dp),
                            )
                            Text(
                                replyDraftItemText(ctx, item),
                                style = MaterialTheme.typography.bodyMedium.copy(
                                    textDecoration = if (item.done) TextDecoration.LineThrough else null,
                                ),
                                color = if (item.done) colours.onSurfaceVariant else colours.onSurface,
                                modifier = Modifier.padding(start = 8.dp),
                            )
                        },
                    ) { done -> host.tick(item.index, done) }
                }
            }
        }
    }
}

@Composable
private fun CardHeading(text: String, icon: Int) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(top = 2.dp).semantics(mergeDescendants = true) { heading() },
    ) {
        Icon(painterResource(icon), contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.size(18.dp))
        Text(
            text,
            style = MaterialTheme.typography.titleSmall,
            color = MaterialTheme.colorScheme.primary,
            modifier = Modifier.padding(start = 8.dp),
        )
    }
}

private fun kindIcon(kind: ReplyDraftTaskKind): Int = when (kind) {
    ReplyDraftTaskKind.FILL_IN -> R.drawable.ic_text_fields
    ReplyDraftTaskKind.ATTACH -> R.drawable.ic_attachment
    ReplyDraftTaskKind.DO -> R.drawable.ic_task_alt
}
