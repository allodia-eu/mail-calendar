// Settings → Accounts as a person meets it (docs/accounts.md rules 11 to 14): every account from
// the core's snapshot, mail or not, each opening a page whose switches, pickers, servers and
// removal ask the core rather than deciding anything themselves.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsAccountsTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val uses = mutableListOf<Triple<String, AccountCapability, Boolean>>()
    private val links = mutableListOf<Triple<String, LinkSlot, String?>>()
    private val signIns = mutableListOf<Pair<String, List<AccountCapability>>>()
    private val saved = mutableListOf<AccountEndpoints>()
    private val removed = mutableListOf<String>()
    private var added = 0

    private val calendarOnly = accountEntry(
        CoreAccountKind.DAV,
        AccountCapability.MAIL to CapabilityState.OFF,
        AccountCapability.CALENDAR to CapabilityState.ON,
        AccountCapability.CONTACTS to CapabilityState.ON,
        id = "alice@dav:cloud.example",
        address = "alice@cloud.example",
    )

    private fun show(vararg accounts: AccountEntry) {
        state.snapshot = AccountsSnapshot(accounts.toList())
        val ui = AccountsSettingsUi(
            state = state,
            onAddAccount = { added++ },
            onSetUse = { id, capability, on -> uses += Triple(id, capability, on) },
            onSetLink = { id, slot, target -> links += Triple(id, slot, target) },
            onSignInAgain = { id, adding -> signIns += id to adding },
            onSaveServers = { _, endpoints -> saved += endpoints },
            onRemove = { removed += it },
        )
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { entry ->
                    AccountPage(entry, ui, syncMode = null, onSetSyncMode = {}) {}
                }
            }
        }
        compose.waitForIdle()
    }

    @Test
    fun every_account_is_listed_mail_or_not_and_add_account_comes_first() {
        show(mailboxOnly(), calendarOnly)

        compose.onNodeWithText("alice@example.org").assertExists()
        // An account without mail is listed by its address, never its id (rule 8).
        compose.onNodeWithText("alice@cloud.example").assertExists()
        compose.onNodeWithText("alice@dav:cloud.example").assertDoesNotExist()
        compose.onNodeWithText("CalDAV/CardDAV · Calendar, Contacts").assertExists()

        compose.onNodeWithTag("accounts-add").performClick()
        assertEquals(1, added)
    }

    @Test
    fun a_row_opens_its_account_page() {
        show(mailboxOnly(), calendarOnly)

        compose.onNodeWithTag("account-row-alice@dav:cloud.example").performClick()
        compose.waitForIdle()

        assertEquals("alice@dav:cloud.example", state.page)
        compose.onNodeWithTag("account-use-CALENDAR").assertIsOn()
        compose.onNodeWithTag("account-use-MAIL").assertIsOff()
    }

    @Test
    fun switching_a_use_off_is_confirmed_first() {
        state.page = calendarOnly.id
        show(calendarOnly)

        compose.onNodeWithTag("account-use-CONTACTS").performClick()
        // Nothing is asked of the core until the person confirms what it deletes.
        assertEquals(emptyList<Any>(), uses)
        compose.onNodeWithTag("account-use-off-confirm").performClick()

        assertEquals(listOf(Triple(calendarOnly.id, AccountCapability.CONTACTS, false)), uses)
    }

    @Test
    fun the_last_use_cannot_be_switched_off() {
        state.page = mailboxOnly().id
        show(mailboxOnly())

        compose.onNodeWithTag("account-use-MAIL").assertIsNotEnabled()
        compose.onNodeWithText("An account is used for at least one of these. Remove it instead.")
            .assertExists()
    }

    @Test
    fun switching_on_a_withheld_use_asks_the_provider_again() {
        val google = accountEntry(
            CoreAccountKind.GOOGLE,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.NEEDS_PERMISSION,
            AccountCapability.CONTACTS to CapabilityState.OFF,
            AccountCapability.COLLEAGUES to CapabilityState.OFF,
            id = "alice@gmail.com@google",
            address = "alice@gmail.com",
        )
        state.page = google.id
        show(google)

        compose.onNodeWithTag("account-use-CALENDAR").assertIsOff().performClick()

        assertEquals(listOf(google.id to listOf(AccountCapability.CALENDAR)), signIns)
        assertEquals(emptyList<Any>(), uses)
    }

    @Test
    fun a_suggested_link_is_named_and_linked_only_when_pressed() {
        val entry = mailboxOnly(
            candidates = LinkCandidates(
                listOf(linked("bob@dav:cloud.example"), linked("alice@dav:cloud.example")),
                emptyList(),
                emptyList(),
                listOf("alice@dav:cloud.example"),
            ),
        )
        state.page = entry.id
        show(entry)

        // The picker still reads None: a suggestion is the person's to confirm (rule 12).
        compose.onNodeWithText("None").assertExists()
        compose.onNodeWithText("Suggested: alice@dav:cloud.example").assertExists()
        compose.onNodeWithTag("account-link-suggested-CALENDAR").performScrollTo().performClick()

        assertEquals(listOf(Triple(entry.id, LinkSlot.CALENDAR, "alice@dav:cloud.example")), links)
    }

    @Test
    fun removing_an_account_names_the_accounts_that_lose_their_link() {
        val linkedTo = calendarOnly.copy(linkedFrom = listOf(linked("alice@example.org")))
        state.page = linkedTo.id
        show(linkedTo)

        compose.onNodeWithTag("account-remove").performScrollTo().performClick()
        compose.onNodeWithText("It is also unlinked from alice@example.org.", substring = true)
            .assertExists()
        compose.onNodeWithTag("account-remove-confirm").performClick()

        assertEquals(listOf(linkedTo.id), removed)
    }

    @Test
    fun saving_servers_sends_what_was_typed_and_keeps_the_stored_password() {
        val entry = mailboxOnly(endpoints = imapEndpoints())
        state.page = entry.id
        show(entry)

        compose.onNodeWithText("imap.example.org").performScrollTo().performTextReplacement("mail.example.org")
        compose.onNodeWithTag("account-servers-save").performScrollTo().performClick()

        val endpoints = saved.single()
        assertEquals("mail.example.org", endpoints.imapHost)
        // A port the person never touched travels as stored.
        assertEquals("smtp.example.org:2525", endpoints.smtpHost)
        assertNull(endpoints.password)
    }

    @Test
    fun a_saved_password_leaves_the_form() {
        val entry = mailboxOnly(endpoints = imapEndpoints())
        state.page = entry.id
        show(entry)

        compose.onNodeWithText("New password").performScrollTo().performTextInput("hunter2")
        compose.onNodeWithText("•••••••", substring = true).assertExists()
        // The core kept it: the form starts again from what was saved, which holds no password.
        state.serversSaved += 1
        compose.waitForIdle()
        compose.onNodeWithText("•••••••", substring = true).assertDoesNotExist()
    }

    @Test
    fun a_failure_waits_to_be_dismissed() {
        state.notice = AccountNotice(NoticeKind.FAILURE, "Couldn’t change this account", "refused")
        show(mailboxOnly())

        compose.onNodeWithText("refused").assertExists()
        compose.onNodeWithText("Close").performClick()
        compose.waitForIdle()

        assertNull(state.notice)
    }

    @Test
    fun a_first_sign_in_taking_the_redirect_ends_a_sign_in_again() {
        state.consent = PendingConsent("acct", google = false, handle = "h")
        state.signingIn = "acct"

        state.yieldConsent()

        // Nothing is left to finish it, so "Sign in again" is offered again.
        assertNull(state.consent)
        assertNull(state.signingIn)
    }

    @Test
    fun a_sign_in_again_already_answering_is_left_alone() {
        // Its redirect arrived: the consent is spent, and the connect still runs.
        state.signingIn = "acct"

        state.yieldConsent()

        assertEquals("acct", state.signingIn)
    }
}
