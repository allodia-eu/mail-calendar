// The contacts screen: what a merged row discloses, how the two empty states differ, and that
// clearing the search resets the CORE rather than only the field.
//
// The core does the merging, the ordering and the matching, so none of that is re-tested here.
// What is tested is the part a client can get wrong on its own, and the "In N accounts" badge is
// the one that matters, because it is the product rule that stops a silent merge (docs/contacts.md).
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTextInput
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ContactDetail
import uniffi.mailcal_bindings.ContactRow
import uniffi.mailcal_bindings.ContactValue

private fun row(
    id: String,
    name: String,
    email: String = "$id@example.test",
    section: String = name.take(1).uppercase(),
    avatarInitials: String = name
        .split(" ")
        .mapNotNull { it.firstOrNull()?.uppercase() }
        .take(2)
        .joinToString(""),
    accountCount: UInt = 1u,
) = ContactRow(
    id = id,
    displayName = name,
    primaryEmail = email,
    section = section,
    avatar = stubAvatar(avatarInitials),
    accountCount = accountCount,
)

@RunWith(RobolectricTestRunner::class)
class ContactsScreenTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun `a person in two accounts is one row that says so`() {
        compose.setContent {
            ContactsScreen(
                rows = listOf(row("1", "Ada Lovelace", accountCount = 2u)),
                onSearch = {},
                detailFor = { null },
            )
        }
        compose.onNodeWithText("Ada Lovelace").assertIsDisplayed()
        // The disclosure. Without it the merge is invisible and the user's second card looks lost.
        compose.onNodeWithText("In 2 accounts").assertIsDisplayed()
    }

    @Test
    fun `an ordinary contact carries no account badge`() {
        compose.setContent {
            ContactsScreen(
                rows = listOf(row("1", "Ada Lovelace")),
                onSearch = {},
                detailFor = { null },
            )
        }
        compose.onNodeWithText("Ada Lovelace").assertIsDisplayed()
        // "In 1 accounts" on every normal contact would be noise, and ungrammatical noise at that.
        assertEquals(
            0,
            compose.onAllNodesWithText("In 1 accounts").fetchSemanticsNodes().size,
        )
    }

    @Test
    fun `a nameless contact gets the localized placeholder, not a blank row`() {
        // The core leaves `displayName` empty for a card that carries an address and no name,
        // precisely so the placeholder can be localised here. Rendering it raw would give a row
        // with a blank first line; baking the text into the core would give a Dutch user English.
        compose.setContent {
            ContactsScreen(
                rows = listOf(
                    row("1", "", email = "anon@example.test", section = "#", avatarInitials = ""),
                ),
                onSearch = {},
                detailFor = { null },
            )
        }
        compose.onNodeWithText("(no name)").assertIsDisplayed()
        compose.onNodeWithText("anon@example.test").assertIsDisplayed()
    }

    @Test
    fun `a row draws the cores avatar`() {
        compose.setContent {
            ContactsScreen(
                rows = listOf(row("1", "Ada Lovelace", avatarInitials = "AL")),
                onSearch = {},
                detailFor = { null },
            )
        }

        compose.onNodeWithTag("contact-avatar", useUnmergedTree = true).assertIsDisplayed()
        compose.onNodeWithText("AL", useUnmergedTree = true).assertIsDisplayed()
    }

    @Test
    fun `section headers mark where the alphabet turns over`() {
        compose.setContent {
            ContactsScreen(
                rows = listOf(
                    row("1", "Ada Lovelace"),
                    row("2", "Alan Turing"),
                    row("3", "Grace Hopper"),
                ),
                onSearch = {},
                detailFor = { null },
            )
        }
        // One "A" for the two A-names, and a "G" where the letter changes.
        compose.onNodeWithText("A").assertIsDisplayed()
        compose.onNodeWithText("G").assertIsDisplayed()
    }

    @Test
    fun `an empty list says something different when a search is what emptied it`() {
        compose.setContent {
            ContactsScreen(rows = emptyList(), onSearch = {}, detailFor = { null })
        }
        // Nothing typed: this account simply has no contacts yet.
        compose.onNodeWithText("No contacts yet").assertIsDisplayed()

        compose.onNodeWithTag("contacts-search").performTextInput("zzz")
        // Typed: telling this user "no contacts yet" would read as though theirs had vanished.
        compose.onNodeWithText("No contacts match your search").assertIsDisplayed()
    }

    @Test
    fun `typing pushes the query into the core, and clearing resets it there too`() {
        val queries = mutableListOf<String>()
        compose.setContent {
            ContactsScreen(rows = emptyList(), onSearch = { queries.add(it) }, detailFor = { null })
        }
        compose.onNodeWithTag("contacts-search").performTextInput("ada")
        assertEquals("ada", queries.last())

        compose.onNodeWithContentDescription("Clear search").performClick()
        // The clear must reach the CORE, not just blank the field: a narrowing the user can no
        // longer see must never shrink the next search (the rule mail search follows).
        assertEquals("", queries.last())
    }

    @Test
    fun `tapping a row opens its detail with the accounts it came from`() {
        val detail = ContactDetail(
            id = "1",
            displayName = "Ada Lovelace",
            avatar = stubAvatar("AL"),
            emails = listOf(
                ContactValue("ada@example.test", listOf("work", "home")),
                ContactValue("ada@work.test", listOf("work")),
            ),
            phones = emptyList(),
            organizations = emptyList(),
            titles = emptyList(),
            accounts = listOf("work", "home"),
        )
        compose.setContent {
            ContactsScreen(
                rows = listOf(row("1", "Ada Lovelace", accountCount = 2u)),
                onSearch = {},
                detailFor = { detail },
            )
        }
        compose.onNodeWithText("Ada Lovelace").performClick()
        compose.onNodeWithTag("contact-detail-avatar", useUnmergedTree = true).assertIsDisplayed()
        compose.onNodeWithText("ada@work.test").assertIsDisplayed()
        // "Also in" is the explanation behind the list row's badge.
        compose.onNodeWithText("Also in").assertIsDisplayed()
        // And this release says plainly that it cannot edit, rather than offering dead buttons.
        compose.onNodeWithText("Contacts are read-only in this version.")
            .performScrollTo()
            .assertIsDisplayed()
    }
}
