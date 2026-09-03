// The contacts-write half of the running app: the two sheets the contacts tab can raise, and the
// off-UI-thread read that seeds the editor.
//
// Split out of MainActivityMailboxTab.kt, which is at the file-length limit; and because this is
// the one place the "an edit names a card, never a person" rule turns into control flow. A person
// is several accounts' cards, so the editor is seeded from ONE of them, chosen by the user when
// there is more than one, and never from the merged detail on screen (docs/contacts.md §3).
package eu.allodia.mailcal

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.ContactWriteStatus
import uniffi.mailcal_bindings.MailcalApp

/** The contacts surface's write state: what a create may file into, and what is open over it. */
internal class ContactWriteState {
    /**
     * Where a new contact could be filed, read off the UI thread when the tab is entered. Empty
     * means nowhere, and the screen then offers no create at all rather than one that cannot
     * succeed.
     */
    var targets by mutableStateOf<List<ContactBookChoice>>(emptyList())

    /** The open editor, or null. */
    var editor by mutableStateOf<ContactEditorState?>(null)

    /** The "which account?" question that precedes it for a person filed in more than one. */
    var cardChoice by mutableStateOf<List<ContactCardChoice>?>(null)

    /**
     * The person whose card is being chosen or edited: the row's id, which an edit carries so a
     * card retired by a merge still resolves.
     */
    var person by mutableStateOf<String?>(null)

    /** The result of the user's last create or edit, pulled on a CONTACTS_STATUS signal. */
    var status by mutableStateOf(ContactWriteStatus.IDLE)

    /**
     * What the list says about the most recent write, or null when there is nothing to say.
     *
     * `Failed` means "we could not confirm this saved", never "rejected": a write whose server
     * call succeeded and whose reconcile did not has already landed. `Invalid` is stated under the
     * form the user is still looking at, so nothing repeats it here.
     */
    fun line(context: android.content.Context): String? = when (status) {
        ContactWriteStatus.SAVING -> L10n.contacts_saving(context)
        ContactWriteStatus.SAVED -> L10n.contacts_saved(context)
        ContactWriteStatus.FAILED -> L10n.contacts_save_unconfirmed(context)
        else -> null
    }

    /** Closes whatever is open over the list. */
    fun close() {
        editor = null
        cardChoice = null
        person = null
    }
}

/** The editor and the "which account?" question, whichever is open. */
@Composable
internal fun MainActivity.ContactWriteSheets(instance: MailcalApp, scope: CoroutineScope) {
    contactWrites.cardChoice?.let { cards ->
        ContactCardChoiceSheet(
            cards = cards,
            onPick = { card ->
                contactWrites.cardChoice = null
                contactWrites.person?.let { person ->
                    openContactEditor(instance, scope, person, card)
                }
            },
            onDismiss = { contactWrites.close() },
        )
    }
    contactWrites.editor?.let { state ->
        ContactEditorSheet(
            state = state,
            onSave = { intent ->
                contactWrites.close()
                instance.dispatch(intent)
            },
            onDismiss = { contactWrites.close() },
        )
    }
}

/**
 * Reads one card's values off the UI thread, then opens the editor on them.
 *
 * A card that has gone (a sync deleted it between the tap and the read) opens no editor: seeding
 * one from nothing would offer to save a blank card over it.
 */
internal fun MainActivity.openContactEditor(
    instance: MailcalApp,
    scope: CoroutineScope,
    person: String,
    card: ContactCardChoice,
) {
    contactWrites.person = person
    scope.launch {
        val seed = withContext(Dispatchers.IO) {
            instance.contactCard(person, card.account, card.card)
        }
        if (seed == null) {
            contactWrites.person = null
            return@launch
        }
        contactWrites.editor = ContactEditorState.edit(
            EditCardTarget(person = person, account = card.account, card = card.card),
            seed,
        )
    }
}
