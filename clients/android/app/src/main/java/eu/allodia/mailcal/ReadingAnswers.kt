// The answers to an open message, as large buttons at the foot of the screen, above the navigation
// bar, where a thumb reaches them on a phone held in one hand (docs/reading-actions.md): the
// default answer, Forward, and a chevron holding the other answer.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.TextAutoSize
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

internal const val READING_DEFAULT_ANSWER_TAG = "reading-default-answer"
internal const val READING_FORWARD_TAG = "reading-forward"
internal const val READING_MORE_REPLIES_TAG = "reading-more-replies"

/**
 * The row of answers.
 *
 * [defaultReplyAll] is the core's answer (`ReadingSnapshot.defaultReplyAll`): reply-all when it
 * would reach someone a plain reply does not, else reply. Until the body arrives it is false, so
 * the row opens on Reply.
 */
@Composable
internal fun ReadingAnswers(
    defaultReplyAll: Boolean,
    onReply: () -> Unit,
    onReplyAll: () -> Unit,
    onForward: () -> Unit,
) {
    val ctx = LocalContext.current
    val reply = Answer(R.drawable.ic_reply, L10n.action_reply(ctx), onReply)
    val replyAll = Answer(R.drawable.ic_reply_all, L10n.action_reply_all(ctx), onReplyAll)
    val (default, other) = if (defaultReplyAll) replyAll to reply else reply to replyAll
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // The default answer is the one most often wanted, so it is given the most room.
        AnswerButton(default, Modifier.weight(1.4f).testTag(READING_DEFAULT_ANSWER_TAG))
        AnswerButton(
            Answer(R.drawable.ic_forward, L10n.action_forward(ctx), onForward),
            Modifier.weight(1f).testTag(READING_FORWARD_TAG),
        )
        // The other answer waits behind a chevron that says there is more, so the usual answer is
        // one tap and nothing hides behind a long press nobody discovers.
        OtherAnswer(other)
    }
}

private class Answer(val icon: Int, val label: String, val action: () -> Unit)

@Composable
private fun AnswerButton(answer: Answer, modifier: Modifier) {
    FilledTonalButton(
        onClick = answer.action,
        modifier = modifier.heightIn(min = 48.dp),
        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 10.dp),
    ) {
        Icon(painterResource(answer.icon), contentDescription = null, modifier = Modifier.size(18.dp))
        Spacer(modifier = Modifier.width(8.dp))
        // One line in every language: a longer label is set smaller rather than cut short.
        Text(answer.label, maxLines = 1, autoSize = TextAutoSize.StepBased(minFontSize = 11.sp, maxFontSize = 14.sp))
    }
}

@Composable
private fun OtherAnswer(answer: Answer) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Box {
        FilledTonalIconButton(
            onClick = { open = true },
            modifier = Modifier.size(48.dp).testTag(READING_MORE_REPLIES_TAG),
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_keyboard_arrow_down),
                contentDescription = L10n.a11y_more_replies(ctx),
            )
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text(answer.label) },
                leadingIcon = { Icon(painterResource(answer.icon), contentDescription = null) },
                onClick = {
                    open = false
                    answer.action()
                },
            )
        }
    }
}
