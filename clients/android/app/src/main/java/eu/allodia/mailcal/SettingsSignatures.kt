// The Signatures settings category (docs/signatures.md): the library, write once, reuse on any
// account, above the per-account defaults, one "For new messages" and one "For replies or
// forwards" picker each. State lives in the Rust core (the SignaturesSnapshot); these render it and
// dispatch the setters, which re-signal SETTINGS.
//
// Two things the layout is deliberate about, and they match macOS/iOS exactly. The library comes
// first because an account picker with nothing to pick is meaningless, a first-time user has to
// write a signature before the defaults mean anything. And "None" is a real option in both pickers
// rather than a separate enable switch: "None for both" already says "no signature on this account".
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountSignatureRow
import uniffi.mailcal_bindings.SignatureRow
import uniffi.mailcal_bindings.SignatureSlotKind

// What the editor is open for. A null `id` is a create, the editor is the same either way, only
// its title and what Save dispatches differ.
private data class EditingSignature(val id: String?, val name: String, val bodyHtml: String?)

// The library: every signature the user has written, each editable and deletable, plus the button
// that writes a new one.
@Composable
internal fun SignatureLibraryCard(
    signatures: List<SignatureRow>,
    // The stored HTML of one signature, fetched only when the editor opens, the snapshot carries
    // names, so drawing this list never drags an embedded logo across the FFI.
    bodyHtmlFor: (String) -> String?,
    onCreate: (name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onUpdate: (id: String, name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onDelete: (String) -> Unit,
) {
    val ctx = LocalContext.current
    var editing by remember { mutableStateOf<EditingSignature?>(null) }
    var deleting by remember { mutableStateOf<SignatureRow?>(null) }

    Column(modifier = Modifier.fillMaxWidth()) {
        if (signatures.isEmpty()) {
            Text(
                text = L10n.settings_signatures_empty(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            signatures.forEach { signature ->
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = signature.name,
                        modifier = Modifier.weight(1f),
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    // The whole row is not clickable: delete sits next to it, and a stray tap that
                    // opens an editor is recoverable while one that deletes is not.
                    TextButton(
                        onClick = {
                            editing = EditingSignature(
                                id = signature.id,
                                name = signature.name,
                                bodyHtml = bodyHtmlFor(signature.id).orEmpty(),
                            )
                        },
                    ) {
                        Text(L10n.settings_signatures_edit(ctx))
                    }
                    IconButton(onClick = { deleting = signature }) {
                        Icon(
                            painter = painterResource(R.drawable.ic_delete),
                            contentDescription = L10n.settings_signatures_delete(ctx),
                            tint = MaterialTheme.colorScheme.error,
                        )
                    }
                }
            }
        }
        TextButton(
            onClick = {
                editing = EditingSignature(
                    id = null,
                    name = L10n.settings_signatures_default_name(ctx),
                    bodyHtml = null,
                )
            },
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_add),
                contentDescription = null,
                modifier = Modifier.padding(end = 8.dp),
            )
            Text(L10n.settings_signatures_add(ctx))
        }
    }

    editing?.let { context ->
        SignatureEditorDialog(
            title = if (context.id == null) {
                L10n.settings_signatures_add(ctx)
            } else {
                context.name
            },
            initialName = context.name,
            initialBodyHtml = context.bodyHtml,
            onSave = { name, html, plain ->
                if (context.id == null) {
                    onCreate(name, html, plain)
                } else {
                    onUpdate(context.id, name, html, plain)
                }
                editing = null
            },
            onDismiss = { editing = null },
        )
    }

    deleting?.let { signature ->
        AlertDialog(
            onDismissRequest = { deleting = null },
            title = { Text(L10n.settings_signatures_delete_title(ctx)) },
            text = { Text(L10n.settings_signatures_delete_message(ctx)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        deleting = null
                        onDelete(signature.id)
                    },
                    colors = ButtonDefaults.textButtonColors(
                        contentColor = MaterialTheme.colorScheme.error,
                    ),
                ) {
                    Text(L10n.settings_signatures_delete(ctx))
                }
            },
            dismissButton = {
                TextButton(onClick = { deleting = null }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

// The per-account defaults: for each configured account, which signature a new message opens with
// and which a reply or forward does. Each independently, each with "None".
@Composable
internal fun AccountSignatureDefaultsCard(
    accounts: List<AccountSignatureRow>,
    signatures: List<SignatureRow>,
    onSet: (account: String, slot: SignatureSlotKind, signature: String?) -> Unit,
) {
    val ctx = LocalContext.current
    if (accounts.isEmpty()) {
        Text(
            text = L10n.settings_accounts_empty(ctx),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        return
    }
    Column(
        modifier = Modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        accounts.forEach { account ->
            Column(modifier = Modifier.fillMaxWidth()) {
                // With one account the address is still shown: the setting is per account, and a
                // user who later adds a second must not have to relearn that.
                Text(
                    text = account.email,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                SignatureSlotPicker(
                    label = L10n.settings_signatures_new_message_label(ctx),
                    signatures = signatures,
                    selected = account.newMessage,
                    onSelect = { onSet(account.accountId, SignatureSlotKind.NEW_MESSAGE, it) },
                )
                SignatureSlotPicker(
                    label = L10n.settings_signatures_reply_forward_label(ctx),
                    signatures = signatures,
                    selected = account.replyForward,
                    onSelect = { onSet(account.accountId, SignatureSlotKind.REPLY_FORWARD, it) },
                )
            }
        }
    }
}

// One slot's picker: the library plus "None". The label is drawn above the control rather than left
// to it, the same as the swipe pickers, two rows holding the same signature would otherwise be
// indistinguishable.
@Composable
private fun SignatureSlotPicker(
    label: String,
    signatures: List<SignatureRow>,
    selected: String?,
    onSelect: (String?) -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    // A slot can name a signature that has since been deleted only if the core failed to clear it
    // (it clears every assignment on delete), so falling back to "None" here is a display detail,
    // not a second teardown path.
    val current = signatures.firstOrNull { it.id == selected }
    Text(label, style = MaterialTheme.typography.labelLarge)
    Box {
        TextButton(onClick = { expanded = true }) {
            Text(
                text = current?.name ?: L10n.settings_signatures_none(ctx),
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Icon(
                painter = painterResource(R.drawable.ic_arrow_drop_down),
                contentDescription = label,
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(L10n.settings_signatures_none(ctx)) },
                onClick = {
                    expanded = false
                    onSelect(null)
                },
            )
            signatures.forEach { signature ->
                DropdownMenuItem(
                    text = { Text(signature.name) },
                    onClick = {
                        expanded = false
                        onSelect(signature.id)
                    },
                )
            }
        }
    }
}
