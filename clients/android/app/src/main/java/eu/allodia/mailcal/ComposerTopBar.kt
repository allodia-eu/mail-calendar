// The composer's app bar: Close on the left; Draft a reply on a reply, Attach, Send and the
// overflow menu on the right. Close is leaving, which keeps the draft; Discard, in the overflow, is
// the one control that throws it away (docs/drafts.md, "Leaving a composer").
//
// Draft a reply is in the bar rather than behind More with Signature and Importance, beside which
// docs/ai.md rule 17 puts it: it runs while a draft is made, and its spinner is the one the person
// watches (docs/waiting.md rule 1), which a closed menu would hide.
//
// Attach and Send are what nearly every message needs, so they are the two drawn in the bar. Saving
// a draft by hand, changing this message's signature and throwing the message away are exceptions,
// and wait behind More: autosave and leaving already keep the draft. The overflow is part of the app
// bar, which IS this platform's action bar, where docs/signatures.md puts the signature control and
// docs/drafts.md the Save and Discard actions: each is something you do to the message, not a field
// you address it with. iPhone keeps Save as draft and Discard behind More too.
//
// Its own file rather than a block inside RichComposeScreen.kt, which is at the 500-line limit.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
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
import uniffi.mailcal_bindings.ControlState

/** The overflow button, for a test that opens the menu without depending on its localised name. */
internal const val COMPOSER_OVERFLOW_TAG = "composer-overflow"

/** Attach, whose spinner is `composer-attach-running` while it runs. */
internal const val COMPOSER_ATTACH_TAG = "composer-attach"

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun ComposerTopBar(
    title: String,
    onClose: () -> Unit,
    // Attach's state, the core's (`ComposerFiles.attach`).
    attach: ControlState,
    onAttach: () -> Unit,
    // The signature menu, opened from the overflow's Signature item and hanging from the same
    // button, or null when signatures are off for this composer or the user has written none: with
    // an empty library the menu would offer nothing but "None", a control that cannot do anything.
    signatureMenu: (@Composable (expanded: Boolean, onExpand: (Boolean) -> Unit) -> Unit)?,
    // The importance menu, opened from the overflow's Importance item and hanging from the same
    // button, or null without a core to ask (docs/importance.md).
    importanceMenu: (@Composable (expanded: Boolean, onExpand: (Boolean) -> Unit) -> Unit)? = null,
    // Saving the draft, or null when this composer keeps none. Never disabled: a draft is
    // unfinished by definition, so there is no state this refuses, and pressing it on an unchanged
    // message reaches no server (`docs/drafts.md`).
    onSaveDraft: (() -> Unit)?,
    // Throwing the draft away, or null when this composer keeps none.
    onDiscard: (() -> Unit)?,
    sendEnabled: Boolean,
    onSend: () -> Unit,
    // Draft a reply (`ComposerReplyDraftControl`), or null while the core offers none here.
    replyDraft: (@Composable () -> Unit)? = null,
) {
    val ctx = LocalContext.current
    TopAppBar(
        title = { Text(title) },
        navigationIcon = {
            IconButton(onClick = onClose) {
                Icon(
                    painter = painterResource(R.drawable.ic_close),
                    contentDescription = L10n.action_close(ctx),
                )
            }
        },
        actions = {
            replyDraft?.invoke()
            // Running while files the person chose are still being copied (docs/waiting.md).
            ControlIconButton(
                icon = R.drawable.ic_attachment,
                label = L10n.action_attach(ctx),
                control = attach,
                tag = COMPOSER_ATTACH_TAG,
                iconSize = 24.dp,
                onClick = onAttach,
            )
            IconButton(enabled = sendEnabled, onClick = onSend) {
                Icon(
                    painter = painterResource(R.drawable.ic_send),
                    contentDescription = L10n.action_send(ctx),
                )
            }
            if (onSaveDraft != null || signatureMenu != null || importanceMenu != null ||
                onDiscard != null
            ) {
                ComposerOverflow(signatureMenu, importanceMenu, onSaveDraft, onDiscard)
            }
        },
    )
}

@Composable
private fun ComposerOverflow(
    signatureMenu: (@Composable (expanded: Boolean, onExpand: (Boolean) -> Unit) -> Unit)?,
    importanceMenu: (@Composable (expanded: Boolean, onExpand: (Boolean) -> Unit) -> Unit)?,
    onSaveDraft: (() -> Unit)?,
    onDiscard: (() -> Unit)?,
) {
    val ctx = LocalContext.current
    var open by remember { mutableStateOf(false) }
    var signaturesOpen by remember { mutableStateOf(false) }
    var importanceOpen by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }, modifier = Modifier.testTag(COMPOSER_OVERFLOW_TAG)) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.a11y_more_actions(ctx),
            )
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            if (onSaveDraft != null) {
                OverflowItem(L10n.action_save_draft(ctx), R.drawable.ic_save_draft) {
                    open = false
                    onSaveDraft()
                }
            }
            if (signatureMenu != null) {
                OverflowItem(L10n.compose_signature_label(ctx), R.drawable.ic_signature) {
                    open = false
                    signaturesOpen = true
                }
            }
            if (importanceMenu != null) {
                OverflowItem(L10n.compose_importance_label(ctx), R.drawable.ic_priority_high) {
                    open = false
                    importanceOpen = true
                }
            }
            // Last, apart from the actions that keep what was written.
            if (onDiscard != null) {
                OverflowItem(L10n.action_discard(ctx), R.drawable.ic_delete) {
                    open = false
                    onDiscard()
                }
            }
        }
        signatureMenu?.invoke(signaturesOpen) { signaturesOpen = it }
        importanceMenu?.invoke(importanceOpen) { importanceOpen = it }
    }
}

@Composable
private fun OverflowItem(label: String, icon: Int, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        leadingIcon = { Icon(painter = painterResource(icon), contentDescription = null) },
        onClick = onClick,
    )
}
