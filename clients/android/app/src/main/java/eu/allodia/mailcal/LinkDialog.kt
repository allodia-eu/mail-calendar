// The link dialog: the words a link is shown as and where it points, the way Outlook asks. The
// address is completed and checked by the core as it is typed (`composerLinkAddress`), the same
// rule the answer is held to, so Apply is enabled exactly when the link can be made.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ComposerLinkAnswer
import uniffi.mailcal_bindings.composerLinkAddress

@Composable
internal fun LinkDialog(
    request: LinkDialogRequest,
    // The address as the link it would make, or null when it cannot be one. A parameter only so the
    // JVM suite can run the dialog without the native library.
    linkAddress: (String) -> String? = ::composerLinkAddress,
    finish: (ComposerLinkAnswer) -> Unit,
) {
    val ctx = LocalContext.current
    var text by remember(request.id) { mutableStateOf(request.text) }
    var address by remember(request.id) { mutableStateOf(request.address) }
    val canApply = linkAddress(address) != null
    // Said only once something has been typed: an empty field is unfinished, not wrong.
    val showsHint = address.isNotBlank() && !canApply
    val apply = { if (canApply) finish(ComposerLinkAnswer.Apply(text, address)) }
    val addressFocus = remember { FocusRequester() }
    LaunchedEffect(request.id) { addressFocus.requestFocus() }

    AlertDialog(
        onDismissRequest = { finish(ComposerLinkAnswer.Cancel) },
        title = {
            Text(
                if (request.address.isEmpty()) {
                    L10n.editor_link_dialog_insert(ctx)
                } else {
                    L10n.editor_link_dialog_edit(ctx)
                },
            )
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    label = { Text(L10n.editor_link_text(ctx)) },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
                    modifier = Modifier.testTag("link-text"),
                )
                OutlinedTextField(
                    value = address,
                    onValueChange = { address = it },
                    label = { Text(L10n.editor_link_address(ctx)) },
                    singleLine = true,
                    isError = showsHint,
                    supportingText = if (showsHint) {
                        { Text(L10n.editor_link_invalid(ctx)) }
                    } else {
                        null
                    },
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Done,
                        autoCorrectEnabled = false,
                    ),
                    keyboardActions = KeyboardActions(onDone = { apply() }),
                    modifier = Modifier.focusRequester(addressFocus).testTag("link-address"),
                )
            }
        },
        confirmButton = {
            TextButton(onClick = apply, enabled = canApply) { Text(L10n.editor_link_apply(ctx)) }
        },
        dismissButton = {
            Row {
                if (request.removable) {
                    TextButton(onClick = { finish(ComposerLinkAnswer.Remove) }) {
                        Text(L10n.editor_link_remove(ctx))
                    }
                }
                TextButton(onClick = { finish(ComposerLinkAnswer.Cancel) }) {
                    Text(L10n.action_cancel(ctx))
                }
            }
        },
    )
}
