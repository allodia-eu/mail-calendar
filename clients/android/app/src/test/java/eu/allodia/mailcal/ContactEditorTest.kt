// What the contact editor's state turns a form into, and what it refuses. A plain-JVM suite: the
// state is a plain class precisely so this needs no composition (AGENTS.md).
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.ContactCardRef
import uniffi.mailcal_bindings.ContactEdit
import uniffi.mailcal_bindings.ContactTarget
import uniffi.mailcal_bindings.Intent

private fun target(account: String, book: String, name: String, isDefault: Boolean) = ContactTarget(
    account = account,
    addressBook = book,
    name = name,
    isDefault = isDefault,
)

private val labels = mapOf(
    "personal" to "me@example.test",
    "work" to "me@work.test",
)

class ContactEditorTest {
    /** The picker opens on the account's own default book, not on whichever came first. */
    @Test
    fun `a create opens on the default book and files the chosen one`() {
        val books = ContactBookChoice.from(
            listOf(
                target("personal", "personal-book", "Personal", false),
                target("work", "work-book", "Work", true),
            ),
            labels,
        )
        val state = ContactEditorState.create(books)
        assertEquals("work", state.target?.account)
        assertTrue(state.picksTarget)

        state.givenName = "Grace"
        state.surname = "Hopper"
        state.emails[0] = "grace@example.test"
        state.target = books.first { it.account == "personal" }
        val intent = state.intent() as Intent.CreateContact
        assertEquals("personal", intent.account)
        assertEquals("personal-book", intent.addressBook)
        assertEquals("Grace", intent.edit.givenName)
        assertEquals(listOf("grace@example.test"), intent.edit.emails)
    }

    /** One book is a fact, not a decision, so the picker is not shown for it. */
    @Test
    fun `one address book is not a choice`() {
        val books = ContactBookChoice.from(
            listOf(target("personal", "personal-book", "Personal", true)),
            labels,
        )
        assertEquals(false, ContactEditorState.create(books).picksTarget)
    }

    /** A book's own name earns a place only where one account offers several. */
    @Test
    fun `a book is labelled by its account, and by its name only where that repeats`() {
        val one = ContactBookChoice.from(
            listOf(
                target("personal", "personal-book", "Personal", true),
                target("work", "work-book", "Work", false),
            ),
            labels,
        )
        assertEquals(listOf("me@example.test", "me@work.test"), one.map { it.label })

        val several = ContactBookChoice.from(
            listOf(
                target("work", "work-book", "Personal", true),
                target("work", "team-book", "Team", false),
            ),
            labels,
        )
        assertEquals(
            listOf("me@work.test (Personal)", "me@work.test (Team)"),
            several.map { it.label },
        )
    }

    /**
     * An edit names the card it was opened on, never the person: a person is several accounts'
     * cards, and saving without naming one files the work details in the personal book.
     */
    @Test
    fun `an edit carries the card it was opened on`() {
        val state = ContactEditorState.edit(
            EditCardTarget(person = "7", account = "work", card = "c-work"),
            ContactEdit(
                givenName = "Ada",
                surname = "Lovelace",
                organization = "",
                title = "",
                emails = listOf("ada@example.test"),
                phones = emptyList(),
            ),
        )
        assertEquals(false, state.picksTarget)
        state.surname = "King"
        val intent = state.intent() as Intent.UpdateContact
        assertEquals("7", intent.person)
        assertEquals("work", intent.account)
        assertEquals("c-work", intent.card)
        assertEquals("King", intent.edit.surname)
    }

    /** A company contact has no person's name; a card with none of the three is a blank row. */
    @Test
    fun `an organization alone is enough and nothing at all is not`() {
        val state = ContactEditorState.create(emptyList())
        assertEquals(ContactFormError.EMPTY, state.error)
        assertNull(state.intent())
        state.organization = "Analytical Engines"
        assertNull(state.error)
    }

    /** The two refusals are different sentences on screen, so they are different values here. */
    @Test
    fun `a malformed address is its own refusal`() {
        for (malformed in listOf("ada", "@example.test", "ada@", "ada@@example.test", "ada@.test")) {
            val state = ContactEditorState.create(emptyList())
            state.givenName = "Ada"
            state.emails[0] = malformed
            assertEquals("$malformed was accepted", ContactFormError.EMAIL, state.error)
        }
    }

    /**
     * A row the user emptied is a row they removed: it must not fail validation as a blank
     * address, and must not reach the core as one.
     */
    @Test
    fun `blank rows are dropped rather than refused`() {
        val state = ContactEditorState.create(emptyList())
        state.givenName = "Ada"
        state.emails[0] = "  "
        state.emails.add(" ada@example.test ")
        state.phones[0] = ""
        val intent = state.intent() as Intent.CreateContact
        assertEquals(listOf("ada@example.test"), intent.edit.emails)
        assertTrue(intent.edit.phones.isEmpty())
    }

    /**
     * A contact with no addresses opens with one empty row, so the field is something to type
     * into rather than a heading over a button; one that has them opens on what it has.
     */
    @Test
    fun `the value lists open on what the card holds, or on one empty row`() {
        val empty = ContactEditorState.create(emptyList())
        assertEquals(listOf(""), empty.emails.toList())

        val seeded = ContactEditorState.edit(
            EditCardTarget("1", "work", "c1"),
            ContactEdit(
                givenName = "Ada",
                surname = "",
                organization = "",
                title = "",
                emails = listOf("a@example.test", "b@example.test"),
                phones = emptyList(),
            ),
        )
        assertEquals(listOf("a@example.test", "b@example.test"), seeded.emails.toList())
        assertEquals(listOf(""), seeded.phones.toList())
    }

    /** A card is labelled by the account the user knows it by, never by the core's internal id. */
    @Test
    fun `a card choice is labelled by its account`() {
        val cards = ContactCardChoice.from(
            listOf(
                ContactCardRef(account = "work", card = "c-work"),
                ContactCardRef(account = "unknown", card = "c-gone"),
            ),
            labels,
        )
        assertEquals(listOf("me@work.test", "unknown"), cards.map { it.label })
    }
}
