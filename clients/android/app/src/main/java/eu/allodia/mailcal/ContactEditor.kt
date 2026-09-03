// The contact editor's state and the intent it produces: a plain class, deliberately, so the
// validation and the create-versus-edit split are testable on the JVM without composing a screen
// (AGENTS.md).
//
// The one rule that is load-bearing here: **an edit names a card, never a person.** The list and
// the detail show people, which the core assembled from the cards several accounts hold, so an
// editor opened on a merged person has to say which card it is editing and be seeded from that
// card alone (docs/contacts.md §3). That is why [EditCardTarget] carries an account and a card id
// and not just the row's id.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.ContactCardRef
import uniffi.mailcal_bindings.ContactEdit
import uniffi.mailcal_bindings.ContactTarget
import uniffi.mailcal_bindings.Intent

/** The card an editor is editing (absent when creating). */
internal data class EditCardTarget(
    /** The person the row carried, so a card retired by a merge still resolves. */
    val person: String,
    val account: String,
    val card: String,
)

/**
 * One address book a create can file into, labelled the way the user knows it.
 *
 * The account's address is what a person recognises; the book's name only earns a place when one
 * account offers several, where the address alone would repeat down the list.
 */
internal data class ContactBookChoice(
    val account: String,
    val addressBook: String,
    val label: String,
    val isDefault: Boolean,
) {
    companion object {
        /** Labels every writable book, given the addresses the accounts are known by. */
        fun from(
            targets: List<ContactTarget>,
            accountLabels: Map<String, String>,
        ): List<ContactBookChoice> {
            val perAccount = targets.groupingBy { it.account }.eachCount()
            return targets.map { target ->
                val account = accountLabels[target.account] ?: target.account
                ContactBookChoice(
                    account = target.account,
                    addressBook = target.addressBook,
                    label = if ((perAccount[target.account] ?: 0) > 1 && target.name.isNotEmpty()) {
                        "$account (${target.name})"
                    } else {
                        account
                    },
                    isDefault = target.isDefault,
                )
            }
        }
    }
}

/** One card an edit could go to, labelled by the account the user knows it by. */
internal data class ContactCardChoice(
    val account: String,
    val card: String,
    val label: String,
) {
    companion object {
        fun from(
            cards: List<ContactCardRef>,
            accountLabels: Map<String, String>,
        ): List<ContactCardChoice> = cards.map { card ->
            ContactCardChoice(
                account = card.account,
                card = card.card,
                label = accountLabels[card.account] ?: card.account,
            )
        }
    }
}

/** Why a form cannot be saved, so the sheet can pick its sentence. */
internal enum class ContactFormError {
    /** Nothing to file the card under: no name, no organisation, no address. */
    EMPTY,

    /** A value in the address list is not an address. */
    EMAIL,
}

/**
 * The mutable state of an open contact editor. Construct via [create] or [edit].
 *
 * The validation is a **copy** of the core's, and deliberately so: the core refuses a card with
 * nothing to file it under, but it has no locale and cannot choose the sentence to put under the
 * form. The client decides what to say; the core stays the backstop.
 */
internal class ContactEditorState private constructor(
    val editing: EditCardTarget?,
    /** Where a create may file the contact. Empty on an edit, which files nowhere new. */
    val targets: List<ContactBookChoice>,
    seed: ContactEdit,
    initialTarget: ContactBookChoice?,
) {
    var givenName by mutableStateOf(seed.givenName)
    var surname by mutableStateOf(seed.surname)
    var organization by mutableStateOf(seed.organization)
    var title by mutableStateOf(seed.title)

    /**
     * The addresses and numbers, as lists the sheet adds to and removes from.
     *
     * Their **order is the card's order**, and the first address is the person's primary one:
     * what the avatar and the list row are keyed on. A contact with none opens with one empty
     * row, so the field is something to type in rather than a heading over a button.
     */
    val emails = mutableStateListOf<String>().also { it.addAll(seed.emails.ifEmpty { listOf("") }) }
    val phones = mutableStateListOf<String>().also { it.addAll(seed.phones.ifEmpty { listOf("") }) }

    var target by mutableStateOf(initialTarget)

    val isEditing: Boolean get() = editing != null

    /** Whether the destination picker is a decision: one book is a fact, not a choice. */
    val picksTarget: Boolean get() = editing == null && targets.size > 1

    /** What is wrong with the form, or `null` when it can be saved. */
    val error: ContactFormError?
        get() {
            val edit = trimmed()
            if (edit.givenName.isEmpty() &&
                edit.surname.isEmpty() &&
                edit.organization.isEmpty() &&
                edit.emails.isEmpty()
            ) {
                return ContactFormError.EMPTY
            }
            return if (edit.emails.any { !isAddressShaped(it) }) ContactFormError.EMAIL else null
        }

    /** The intent a Save dispatches, or `null` when the form is not valid. */
    fun intent(): Intent? {
        if (error != null) {
            return null
        }
        val edit = trimmed()
        val card = editing
        return if (card != null) {
            Intent.UpdateContact(
                person = card.person,
                account = card.account,
                card = card.card,
                edit = edit,
            )
        } else {
            Intent.CreateContact(
                account = target?.account,
                addressBook = target?.addressBook,
                edit = edit,
            )
        }
    }

    /**
     * The form with every value trimmed and its blank rows dropped.
     *
     * The core trims too; doing it here as well is what makes the validation agree with the
     * refusal. A form holding one empty address row is a form with no addresses, and telling the
     * user otherwise would be a message about a row they can see is blank.
     */
    private fun trimmed(): ContactEdit = ContactEdit(
        givenName = givenName.trim(),
        surname = surname.trim(),
        organization = organization.trim(),
        title = title.trim(),
        emails = emails.map { it.trim() }.filter { it.isNotEmpty() },
        phones = phones.map { it.trim() }.filter { it.isNotEmpty() },
    )

    companion object {
        /** An empty form filing into the account's default book, else the first on offer. */
        fun create(targets: List<ContactBookChoice>): ContactEditorState = ContactEditorState(
            editing = null,
            targets = targets,
            seed = blank(),
            initialTarget = targets.firstOrNull { it.isDefault } ?: targets.firstOrNull(),
        )

        /** A form seeded with one card's values. */
        fun edit(target: EditCardTarget, seed: ContactEdit): ContactEditorState =
            ContactEditorState(
                editing = target,
                targets = emptyList(),
                seed = seed,
                initialTarget = null,
            )

        private fun blank() = ContactEdit(
            givenName = "",
            surname = "",
            organization = "",
            title = "",
            emails = emptyList(),
            phones = emptyList(),
        )

        /**
         * Whether a string is shaped like an email address; the same test the core applies.
         *
         * A backstop, not a parser: the server is the authority on what it accepts. What this
         * catches is the value that would reach it as a malformed card and come back as an opaque
         * 400.
         */
        internal fun isAddressShaped(value: String): Boolean {
            val at = value.indexOf('@')
            if (at <= 0 || at == value.length - 1) {
                return false
            }
            val domain = value.substring(at + 1)
            return !domain.contains('@') && !domain.startsWith('.')
        }
    }
}
