// What the composer needs to seed, swap and override a signature (docs/signatures.md), plus the two
// pure rules the client owns. They live here rather than inside the composer so the JVM suite can
// pin them without composing a screen, the same reason `signatureSlot` is a free function on Apple.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import org.json.JSONObject
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.SignatureBody
import uniffi.mailcal_bindings.SignatureRow
import uniffi.mailcal_bindings.SignatureSlotKind

// The library to list plus the two lookups the core answers. Passed as a value rather than the whole
// app so the composer stays free of it; `null` turns signatures off entirely, which is what a
// screenshot run and the JVM tests want.
internal data class ComposerSignatures(
    // The library, for the picker.
    val library: List<SignatureRow>,
    // The signature `account` uses in `slot`, or null when that slot is unassigned.
    val forAccount: (String, SignatureSlotKind) -> SignatureBody?,
    // One signature by id, the per-message override.
    val byId: (String) -> SignatureBody?,
)

// Binds the composer's three needs to the live core. `library` comes from the snapshot the SETTINGS
// signal refreshes (names only); the two lookups are cheap in-memory core reads, run when the
// composer opens and again whenever From changes, never cached, because the assignment can change
// under an open composer.
internal fun composerSignatures(app: MailcalApp, library: List<SignatureRow>) = ComposerSignatures(
    library = library,
    forAccount = { account, slot -> app.resolveSignature(account, slot) },
    byId = { id -> app.signatureBody(id) },
)

// What this one message's signature should be. `null` (the initial state) means FOLLOW THE ACCOUNT:
// it re-resolves whenever the From dropdown changes, which is what a user who never touched the
// picker expects, their work signature when sending from work.
//
// Once they pick explicitly, that choice sticks even across a From change: they chose it *for this
// message*, and silently replacing it would undo a deliberate act. (Outlook re-swaps regardless,
// which is its most complained-about composer behaviour.)
internal sealed interface SignatureChoice {
    // No signature on this message.
    data object NoSignature : SignatureChoice

    // This specific signature, by id.
    data class Named(val id: String) : SignatureChoice
}

// Which signature slot a composer opened in `mode` seeds from. A reply, a reply-all and a forward
// share one slot (Outlook's grouping): all three continue an existing message, and splitting them
// makes a setting nobody sets.
internal fun signatureSlot(mode: RichComposeMode): SignatureSlotKind =
    if (mode == RichComposeMode.New) SignatureSlotKind.NEW_MESSAGE else SignatureSlotKind.REPLY_FORWARD

// The signature on this message right now: the user's explicit choice if they made one, else
// whatever `account` assigns for this mode.
internal fun ComposerSignatures.resolve(
    choice: SignatureChoice?,
    account: String?,
    mode: RichComposeMode,
): SignatureBody? = when (choice) {
    null -> account?.let { forAccount(it, signatureSlot(mode)) }
    SignatureChoice.NoSignature -> null
    is SignatureChoice.Named -> byId(choice.id)
}

// The `setComposerSignature` payload: the shape the Rust composer's `Block::Signature` round-trips,
// so what the editor hands back on submit is what the core already understands. A null body means
// "no signature", which the editor seam reads as "remove the region".
internal fun signatureSeedJson(body: SignatureBody?): String? = body?.let {
    JSONObject()
        .put("body_html", it.bodyHtml)
        .put("body_plain", it.bodyPlain)
        .toString()
}

// The signature control: one app-bar button that opens the library plus "None". The current choice
// is a checkmark inside the menu rather than a label on the button, so the bar stays a row of icons
// a button labelled with whichever signature is selected would say nothing about what it does.
@Composable
internal fun ComposerSignatureAction(
    library: List<SignatureRow>,
    selected: String?,
    expanded: Boolean,
    onExpand: (Boolean) -> Unit,
    onSelect: (String?) -> Unit,
) {
    val ctx = LocalContext.current
    Box {
        IconButton(onClick = { onExpand(true) }) {
            Icon(
                painter = painterResource(R.drawable.ic_signature),
                contentDescription = L10n.compose_signature_label(ctx),
            )
        }
        DropdownMenu(expanded = expanded, onDismissRequest = { onExpand(false) }) {
            SignatureMenuItem(L10n.settings_signatures_none(ctx), selected == null) {
                onExpand(false)
                onSelect(null)
            }
            library.forEach { signature ->
                SignatureMenuItem(signature.name, selected == signature.id) {
                    onExpand(false)
                    onSelect(signature.id)
                }
            }
        }
    }
}

@Composable
private fun SignatureMenuItem(label: String, selected: Boolean, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { Text(label) },
        trailingIcon = {
            if (selected) {
                Icon(painter = painterResource(R.drawable.ic_check), contentDescription = null)
            }
        },
        onClick = onClick,
    )
}
