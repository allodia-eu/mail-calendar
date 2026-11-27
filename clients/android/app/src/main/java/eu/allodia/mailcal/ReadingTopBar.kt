// The reading screen's actions, in the top app bar (docs/reading-actions.md): archive and delete;
// the default answer, with a chevron beside it for the other answers; then the overflow for what
// acts on the message as a document.
//
// The bottom of the screen is the navigation bar's, which stays while a message is open so the
// calendar is one tap away. A second bar of actions above it would stack two bars of buttons, and
// a navigation bar is for moving between sections, never for acting on what is shown.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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

internal const val READING_DEFAULT_ANSWER_TAG = "reading-default-answer"
internal const val READING_MORE_REPLIES_TAG = "reading-more-replies"

/**
 * The bar over an open message.
 *
 * [defaultReplyAll] is the core's answer (`ReadingSnapshot.defaultReplyAll`): reply-all when it
 * would reach someone a plain reply does not, else reply. Until the body arrives it is false, so
 * the bar opens on Reply. [overflow] is the menu drawn last of all.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ReadingTopBar(
    defaultReplyAll: Boolean,
    onBack: () -> Unit,
    onArchive: () -> Unit,
    onDelete: () -> Unit,
    onReply: () -> Unit,
    onReplyAll: () -> Unit,
    onForward: () -> Unit,
    overflow: @Composable () -> Unit,
) {
    val ctx = LocalContext.current
    val reply = Answer(R.drawable.ic_reply, L10n.action_reply(ctx), onReply)
    val replyAll = Answer(R.drawable.ic_reply_all, L10n.action_reply_all(ctx), onReplyAll)
    val forward = Answer(R.drawable.ic_forward, L10n.action_forward(ctx), onForward)
    val (default, other) = if (defaultReplyAll) replyAll to reply else reply to replyAll
    TopAppBar(
        title = {},
        navigationIcon = {
            IconButton(onClick = onBack) {
                Icon(painterResource(R.drawable.ic_arrow_back), contentDescription = L10n.a11y_back(ctx))
            }
        },
        actions = {
            IconButton(onClick = onArchive) {
                Icon(painterResource(R.drawable.ic_archive), contentDescription = L10n.action_archive(ctx))
            }
            IconButton(onClick = onDelete) {
                Icon(
                    painter = painterResource(R.drawable.ic_delete),
                    contentDescription = L10n.action_delete(ctx),
                    tint = MaterialTheme.colorScheme.error,
                )
            }
            // The groups stand apart, so the answers do not read as more mailbox actions.
            Spacer(modifier = Modifier.width(8.dp))
            IconButton(onClick = default.action, modifier = Modifier.testTag(READING_DEFAULT_ANSWER_TAG)) {
                Icon(painterResource(default.icon), contentDescription = default.label)
            }
            // The other answer and forward wait behind a chevron that says there are more, so the
            // usual answer is one tap and nothing hides behind a long press nobody discovers.
            MoreReplies(other, forward)
            Spacer(modifier = Modifier.width(8.dp))
            overflow()
        },
    )
}

private class Answer(val icon: Int, val label: String, val action: () -> Unit)

@Composable
private fun MoreReplies(vararg answers: Answer) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }, modifier = Modifier.testTag(READING_MORE_REPLIES_TAG)) {
            Icon(
                painter = painterResource(R.drawable.ic_keyboard_arrow_down),
                contentDescription = L10n.a11y_more_replies(ctx),
            )
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            answers.forEach { answer ->
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
}
