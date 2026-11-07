// The core's notices drawn (docs/notices.md): one Material outlined card at the top of the screen
// on show, holding the first notice that belongs there, and the rest in a bottom sheet behind its
// "N more issues" button. Material 3 has no banner component, and a snackbar goes by itself, which
// a standing notice must not. Which notice goes where is NoticeRules'.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedCard
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeScreens
import uniffi.mailcal_bindings.NoticeSeverity
import uniffi.mailcal_bindings.NoticeText

/** What the notice views run: [running] names the notice whose remedy is under way. */
internal class NoticeHandlers(
    val running: String? = null,
    val onRun: (Notice) -> Unit = {},
    val onDismiss: (Notice) -> Unit = {},
)

/** A notice's words, through the lookup generated from the catalog (rule 11). */
internal fun noticeWords(ctx: Context, text: NoticeText): String = L10n.notice(ctx, text.key, text.args)

/** The colour a severity's glyph takes: only Blocked takes the warning colour (rule 4). */
@Composable
private fun severityTint(severity: NoticeSeverity): Color =
    if (severity == NoticeSeverity.BLOCKED) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant

/** The card on [screen], and the Issues sheet behind it; nothing when no notice belongs there. */
@Composable
internal fun NoticeArea(notices: List<Notice>, screen: NoticeScreens, handlers: NoticeHandlers) {
    val first = NoticeRules.prominent(notices, screen) ?: return
    var showingIssues by rememberSaveable { mutableStateOf(false) }
    NoticeCard(first, NoticeRules.more(notices, screen), handlers) { showingIssues = true }
    if (showingIssues) IssuesSheet(notices, handlers) { showingIssues = false }
}

/**
 * Says each newly raised notice to assistive technology once (rule 12): a polite live region,
 * which is how Android announces now that a view's own announcement is deprecated. [text] is what
 * arrived with the last change, and empty otherwise, so a notice that goes and comes back changes
 * it again.
 */
@Composable
internal fun NoticeAnnouncer(text: String) {
    Box(
        modifier = Modifier.size(1.dp).testTag("notice-announcer").semantics {
            liveRegion = LiveRegionMode.Polite
            if (text.isNotEmpty()) contentDescription = text
        },
    )
}

@Composable
private fun NoticeCard(notice: Notice, more: Int, handlers: NoticeHandlers, onShowIssues: () -> Unit) {
    val ctx = LocalContext.current
    val border = if (notice.severity == NoticeSeverity.BLOCKED) {
        MaterialTheme.colorScheme.error
    } else {
        MaterialTheme.colorScheme.outlineVariant
    }
    OutlinedCard(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 6.dp).testTag("notice-card"),
        border = BorderStroke(1.dp, border),
        colors = CardDefaults.outlinedCardColors(),
    ) {
        val actions = more > 0 || NoticeRules.offers(notice)
        // The same margin on both sides; only a Close button, which carries its own, sits closer.
        val end = if (notice.dismissible) 4.dp else 16.dp
        Column(modifier = Modifier.padding(start = 16.dp, end = end, top = 12.dp, bottom = if (actions) 8.dp else 12.dp)) {
            NoticeBody(notice, handlers)
            if (actions) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(top = 4.dp, end = 16.dp - end),
                    horizontalArrangement = Arrangement.End,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (more > 0) {
                        TextButton(onClick = onShowIssues) { Text(L10n.notices_more(ctx, more)) }
                        Spacer(modifier = Modifier.width(8.dp))
                    }
                    NoticeActionButton(notice, handlers, describe = false)
                }
            }
        }
    }
}

/** The glyph, the title, the line beneath it and, for a notice that can be closed, Close. */
@Composable
private fun NoticeBody(notice: Notice, handlers: NoticeHandlers) {
    val ctx = LocalContext.current
    Row(verticalAlignment = Alignment.Top) {
        Icon(
            painter = painterResource(NoticeRules.glyph(notice.severity)),
            contentDescription = null,
            tint = severityTint(notice.severity),
            modifier = Modifier.padding(top = 2.dp).size(20.dp),
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(noticeWords(ctx, notice.title), style = MaterialTheme.typography.bodyLarge)
            NoticeRules.beneath(notice, { noticeWords(ctx, it) }, L10n.notices_remedy_failed(ctx))?.let {
                Text(
                    it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (notice.dismissible) {
            IconButton(onClick = { handlers.onDismiss(notice) }, modifier = Modifier.size(36.dp)) {
                Icon(
                    painter = painterResource(R.drawable.ic_close),
                    contentDescription = L10n.notices_dismiss(ctx),
                    modifier = Modifier.size(20.dp),
                )
            }
        }
    }
}

/**
 * The notice's one action, held with a spinner while the sign-in it started runs (docs/waiting.md).
 * [describe] names the notice too, where several such buttons share a list.
 */
@Composable
private fun NoticeActionButton(notice: Notice, handlers: NoticeHandlers, describe: Boolean) {
    val ctx = LocalContext.current
    val label = notice.actionLabel?.takeIf { NoticeRules.offers(notice) }?.let { noticeWords(ctx, it) } ?: return
    val running = handlers.running == notice.id
    val title = noticeWords(ctx, notice.title)
    ControlButton(
        label = label,
        control = ControlState(enabled = !running, running = running),
        tag = "notice-action-${notice.id}",
        modifier = if (describe) Modifier.semantics { contentDescription = "$label: $title" } else Modifier,
        onClick = { handlers.onRun(notice) },
    )
}

/** Every notice in force, in the core's order, each with its own action (rule 2). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun IssuesSheet(notices: List<Notice>, handlers: NoticeHandlers, onClose: () -> Unit) {
    val ctx = LocalContext.current
    ModalBottomSheet(onDismissRequest = onClose) {
        Text(
            L10n.notices_issues(ctx),
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(horizontal = 24.dp).semantics { heading() },
        )
        Spacer(modifier = Modifier.height(8.dp))
        LazyColumn(modifier = Modifier.testTag("notice-issues")) {
            items(notices, key = { it.id }) { notice ->
                Column(modifier = Modifier.padding(start = 24.dp, end = 16.dp, top = 12.dp, bottom = 12.dp)) {
                    NoticeBody(notice, handlers)
                    if (NoticeRules.offers(notice)) {
                        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                            NoticeActionButton(notice, handlers, describe = true)
                        }
                    }
                }
                HorizontalDivider()
            }
        }
        Spacer(modifier = Modifier.height(24.dp))
    }
}

/**
 * The line an account's row carries under its address (rule 6): its first notice's glyph and its
 * mark, on one line. A screen reader hears every sentence instead.
 */
@Composable
internal fun AccountMarkLine(mark: AccountMark, modifier: Modifier = Modifier) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = modifier.clearAndSetSemantics { contentDescription = mark.detail },
    ) {
        Icon(
            painter = painterResource(NoticeRules.glyph(mark.severity)),
            contentDescription = null,
            tint = severityTint(mark.severity),
            modifier = Modifier.size(14.dp),
        )
        Spacer(modifier = Modifier.width(4.dp))
        Text(
            mark.label,
            style = MaterialTheme.typography.bodySmall,
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** The glyph a row in a list draws for an account with notices, for a menu with no second line. */
@Composable
internal fun AccountMarkGlyph(mark: AccountMark) {
    Icon(
        painter = painterResource(NoticeRules.glyph(mark.severity)),
        contentDescription = mark.detail,
        tint = severityTint(mark.severity),
    )
}

/** The account's notices at the top of its page in Settings, each closable where the core allows. */
@Composable
internal fun AccountNoticesGroup(notices: List<Notice>, handlers: NoticeHandlers) {
    if (notices.isEmpty()) return
    OutlinedCard(modifier = Modifier.fillMaxWidth().testTag("account-notices")) {
        val end = if (notices.any { it.dismissible }) 4.dp else 16.dp
        Column(modifier = Modifier.padding(start = 16.dp, end = end, top = 12.dp, bottom = 12.dp)) {
            notices.forEachIndexed { index, notice ->
                if (index > 0) HorizontalDivider(modifier = Modifier.padding(vertical = 8.dp))
                NoticeBody(notice, handlers)
            }
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
}
