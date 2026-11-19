// Settings → Signatures (docs/signatures.md, "Changing the library"): the library above each
// account's two slots, drawn from the core's `SignaturesPage`. Every control is the page's, drawn
// through the control components (docs/waiting.md); a change is a `SignatureIntent` that returns at
// once, and the page read after it says what it came to.
//
// The library comes first because an account picker with nothing to pick says nothing. "None" is a
// choice in both pickers rather than a separate switch: None in both already says "no signature on
// this account".
package eu.allodia.mailcal

import android.content.Context
import android.text.format.Formatter
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.SignatureEntry
import uniffi.mailcal_bindings.SignatureImageRefusal
import uniffi.mailcal_bindings.SignatureIntent
import uniffi.mailcal_bindings.SignaturePicker
import uniffi.mailcal_bindings.SignatureRefusal
import uniffi.mailcal_bindings.SignatureSlotKind
import uniffi.mailcal_bindings.SignaturesPage

internal const val SIGNATURE_ADD_TAG = "signature-add"
internal const val SIGNATURE_DELETE_CONFIRM_TAG = "signature-delete-confirm"

internal fun signatureEditTag(id: String) = "signature-edit-$id"

internal fun signatureDeleteTag(id: String) = "signature-delete-$id"

internal fun signatureSlotTag(account: String, slot: SignatureSlotKind) = "signature-slot-$account-$slot"

/** What Settings → Signatures draws and what it asks of the core. */
internal data class SignaturesUi(
    // Null until the first read.
    val page: SignaturesPage?,
    // One signature's stored HTML, fetched when its editor opens: the page carries names only, so
    // drawing the list never carries an embedded logo across the FFI.
    val bodyHtml: (String) -> String?,
    val onChange: (SignatureIntent) -> Unit,
    // The core's `signatureNameRefusal` and `signatureImageRefusal`, passed in so the JVM suite,
    // which never loads the cdylib, can draw the editor.
    val nameRefusal: (String) -> SignatureRefusal?,
    val imageRefusal: (mediaType: String, size: Long) -> SignatureImageRefusal?,
)

/** Why a signature was not saved, in the person's words. */
internal fun signatureRefusalText(ctx: Context, refusal: SignatureRefusal): String = when (refusal) {
    SignatureRefusal.BlankName -> L10n.settings_signatures_name_required(ctx)
    SignatureRefusal.Gone -> L10n.settings_signatures_gone(ctx)
    is SignatureRefusal.Image -> signatureImageRefusalText(ctx, refusal.refusal)
}

/** Why a picture may not go into a signature, in the person's words. */
internal fun signatureImageRefusalText(ctx: Context, refusal: SignatureImageRefusal): String = when (refusal) {
    SignatureImageRefusal.Format -> L10n.settings_signatures_image_format(ctx)
    is SignatureImageRefusal.TooLarge -> L10n.settings_signatures_image_too_large(
        ctx,
        limit = Formatter.formatShortFileSize(ctx, refusal.limitBytes.toLong()),
    )
}

// The signature an editor is open on: the id it saves under (the page's `newId` for a new one) and
// the serial of the outcome in force when it opened, so only a newer one for this id is acted on.
private data class EditingSignature(
    val id: String,
    val title: String,
    val name: String,
    val bodyHtml: String?,
    val openedSerial: ULong,
)

// The library: every signature written, each editable and deletable, and New signature.
@Composable
internal fun SignatureLibraryCard(ui: SignaturesUi) {
    val ctx = LocalContext.current
    val page = ui.page ?: return
    var editing by remember { mutableStateOf<EditingSignature?>(null) }
    var deleting by remember { mutableStateOf<SignatureEntry?>(null) }
    val openedSerial = page.outcome?.serial ?: 0uL

    Column(modifier = Modifier.fillMaxWidth()) {
        if (page.signatures.isEmpty()) {
            Text(
                text = L10n.settings_signatures_empty(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        page.signatures.forEach { signature ->
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
                ControlTextButton(
                    label = L10n.settings_signatures_edit(ctx),
                    control = signature.edit,
                    tag = signatureEditTag(signature.id),
                ) {
                    editing = EditingSignature(
                        id = signature.id,
                        title = signature.name,
                        name = signature.name,
                        bodyHtml = ui.bodyHtml(signature.id).orEmpty(),
                        openedSerial = openedSerial,
                    )
                }
                ControlIconButton(
                    icon = R.drawable.ic_delete,
                    label = L10n.settings_signatures_delete(ctx),
                    control = signature.delete,
                    tag = signatureDeleteTag(signature.id),
                    destructive = true,
                ) { deleting = signature }
            }
        }
        ControlTextButton(
            label = L10n.settings_signatures_add(ctx),
            control = page.add,
            tag = SIGNATURE_ADD_TAG,
            icon = R.drawable.ic_add,
        ) {
            editing = EditingSignature(
                id = page.newId,
                title = L10n.settings_signatures_add(ctx),
                name = L10n.settings_signatures_default_name(ctx),
                bodyHtml = null,
                openedSerial = openedSerial,
            )
        }
    }

    editing?.let { open ->
        SignatureEditorDialog(
            id = open.id,
            title = open.title,
            initialName = open.name,
            initialBodyHtml = open.bodyHtml,
            openedSerial = open.openedSerial,
            ui = ui,
            onDismiss = { editing = null },
        )
    }

    deleting?.let { signature ->
        ConfirmDialog(
            title = L10n.settings_signatures_delete_title(ctx),
            message = L10n.settings_signatures_delete_message(ctx),
            action = L10n.settings_signatures_delete(ctx),
            tag = SIGNATURE_DELETE_CONFIRM_TAG,
        ) { confirmed ->
            deleting = null
            if (confirmed) ui.onChange(SignatureIntent.Delete(signature.id))
        }
    }
}

// Each account's two slots: which signature a new message opens with and which a reply or forward
// does, each with "None".
@Composable
internal fun AccountSignatureDefaultsCard(ui: SignaturesUi) {
    val ctx = LocalContext.current
    val page = ui.page ?: return
    if (page.accounts.isEmpty()) {
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
        page.accounts.forEach { account ->
            Column(modifier = Modifier.fillMaxWidth()) {
                // With one account the address is still shown: the setting is per account, and a
                // user who later adds a second must not have to relearn that.
                Text(
                    text = account.email,
                    style = MaterialTheme.typography.titleSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                listOf(
                    Triple(SignatureSlotKind.NEW_MESSAGE, account.newMessage, L10n.settings_signatures_new_message_label(ctx)),
                    Triple(SignatureSlotKind.REPLY_FORWARD, account.replyForward, L10n.settings_signatures_reply_forward_label(ctx)),
                ).forEach { (slot, picker, label) ->
                    SignatureSlotPicker(label, page.signatures, picker, signatureSlotTag(account.accountId, slot)) {
                        ui.onChange(SignatureIntent.Assign(account.accountId, slot, it))
                    }
                }
            }
        }
    }
}

// One slot's picker: "None", then the library. The label is drawn above the control rather than
// left to it, so two slots holding the same signature can be told apart.
@Composable
private fun SignatureSlotPicker(
    label: String,
    signatures: List<SignatureEntry>,
    picker: SignaturePicker,
    tag: String,
    onSelect: (String?) -> Unit,
) {
    val ctx = LocalContext.current
    val selected = signatures.indexOfFirst { it.id == picker.selected } + 1
    ControlDropdown(
        title = label,
        labels = listOf(L10n.settings_signatures_none(ctx)) + signatures.map { it.name },
        selected = selected,
        control = picker.control,
        tag = tag,
    ) { index -> onSelect(signatures.getOrNull(index - 1)?.id) }
}
