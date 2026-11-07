// Settings → Accounts as a person meets it (docs/accounts.md rules 11 to 14): every account from
// the core's snapshot, mail or not, each opening a page that draws what the core decided of its
// switches, pickers and removal, and asks the core for every change.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.robolectric.RuntimeEnvironment
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountChangeNotice
import uniffi.mailcal_bindings.AccountDistinction
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountLinksView
import uniffi.mailcal_bindings.AccountPage
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountUseBlock
import uniffi.mailcal_bindings.AccountUseStatus
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.LinkCandidates
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsAccountsTest {
    @get:Rule val compose = createComposeRule()

    private val state = AccountsUiState()
    private val changes = mutableListOf<AccountPageIntent>()
    private val signIns = mutableListOf<Pair<String, List<AccountCapability>>>()
    private val saved = mutableListOf<AccountEndpoints>()
    private var added = 0

    private val calendarOnly = accountEntry(
        CoreAccountKind.DAV,
        AccountCapability.MAIL to CapabilityState.OFF,
        AccountCapability.CALENDAR to CapabilityState.ON,
        AccountCapability.CONTACTS to CapabilityState.ON,
        id = "alice@dav:cloud.example",
        address = "alice@cloud.example",
    )

    /** Shows [accounts], the open one's page drawn from [page], else from [pageOf]. */
    private fun show(vararg accounts: AccountEntry, page: AccountPage? = null) {
        state.snapshot = AccountsSnapshot(accounts.toList())
        fun open(id: String?) {
            state.page = id
            state.accountPage = page?.takeIf { it.account == id }
                ?: id?.let(state::entry)?.let { pageOf(it) }
        }
        open(state.page)
        val ui = AccountsSettingsUi(
            state = state,
            onAddAccount = { added++ },
            onOpenPage = ::open,
            onChange = { changes += it },
            onSignInAgain = { id, adding -> signIns += id to adding },
            onSaveServers = { _, endpoints -> saved += endpoints },
        )
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { shown, record ->
                    AccountPage(shown, record, ui, syncMode = null, onSetSyncMode = {}) {}
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
        assertEquals(emptyList<Any>(), changes)
        compose.onNodeWithTag("account-use-off-confirm").performClick()

        assertEquals(
            listOf(AccountPageIntent.SetUse(calendarOnly.id, AccountCapability.CONTACTS, false)),
            changes,
        )
    }

    @Test
    fun a_switch_the_core_holds_says_why_beside_what_is_wrong() {
        val entry = mailboxOnly()
        state.page = entry.id
        val held = ControlState(enabled = false, running = false)
        val unreachable = AccountUseStatus.Problem(uniffi.mailcal_bindings.UseProblem.UNREACHABLE)
        show(
            entry,
            page = pageOf(
                entry,
                uses = listOf(
                    useSwitch(AccountCapability.MAIL, on = true, control = held, blocked = AccountUseBlock.LAST_USE),
                    useSwitch(AccountCapability.CALENDAR, on = true, status = unreachable),
                ),
            ),
        )

        compose.onNodeWithTag("account-use-MAIL").assertIsNotEnabled()
        compose.onNodeWithText("An account is used for at least one of these. Remove it instead.")
            .assertExists()
        compose.onNodeWithText("Couldn’t reach your calendar server", substring = true).assertExists()
    }

    @Test
    fun a_change_running_spins_beside_its_own_control() {
        val entry = calendarOnly
        state.page = entry.id
        val running = ControlState(enabled = false, running = true)
        show(
            entry,
            page = pageOf(
                entry,
                uses = listOf(useSwitch(AccountCapability.CALENDAR, on = false, control = running)),
                remove = ControlState(enabled = false, running = false),
            ),
        )

        compose.onNodeWithTag("account-use-CALENDAR-running").assertExists()
        compose.onNodeWithTag("account-use-CALENDAR").assertIsOff().assertIsNotEnabled()
        compose.onNodeWithTag("account-remove").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("account-remove-running").assertDoesNotExist()
    }

    @Test
    fun what_a_change_came_to_is_said_in_the_persons_words() {
        val ctx = RuntimeEnvironment.getApplication()
        val failed = changeNotice(ctx, AccountChangeNotice.LinkFailed(LinkSlot.CALENDAR, "refused"))
        assertEquals(AccountNotice(NoticeKind.FAILURE, "Couldn’t change this account", "refused"), failed)
        val kept = changeNotice(ctx, AccountChangeNotice.RemoveFailed("locked"))
        assertEquals("Couldn't remove this account: locked", kept.message)
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
        val calendar = useSwitch(
            AccountCapability.CALENDAR,
            on = false,
            status = AccountUseStatus.Withheld,
            asksProvider = true,
        )
        show(google, page = pageOf(google, uses = listOf(calendar)))

        compose.onNodeWithTag("account-use-CALENDAR").assertIsOff().performClick()

        assertEquals(listOf(google.id to listOf(AccountCapability.CALENDAR)), signIns)
        assertEquals(emptyList<Any>(), changes)
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
        val picker = linkPicker(LinkSlot.CALENDAR, entry.linkCandidates.calendar, suggested = 1)
        show(entry, page = pageOf(entry, links = listOf(picker)))

        // The picker still reads None: a suggestion is the person's to confirm (rule 12).
        compose.onNodeWithText("None").assertExists()
        compose.onNodeWithText("Suggested: alice@dav:cloud.example").assertExists()
        compose.onNodeWithTag("account-link-suggested-CALENDAR").performScrollTo().performClick()

        assertEquals(
            listOf(AccountPageIntent.SetLink(entry.id, LinkSlot.CALENDAR, "alice@dav:cloud.example")),
            changes,
        )
    }

    @Test
    fun a_link_to_an_account_with_the_page_s_own_address_says_what_tells_them_apart() {
        val calendar = linked("alice@example.org", AccountDistinction.Kind(CoreAccountKind.DAV))
        val entry = mailboxOnly(
            links = AccountLinksView(calendar, null, null),
            candidates = LinkCandidates(listOf(calendar), emptyList(), emptyList(), emptyList()),
        )
        state.page = entry.id
        show(entry, page = pageOf(entry, links = listOf(linkPicker(LinkSlot.CALENDAR, listOf(calendar), 0))))

        compose.onNodeWithText("alice@example.org (CalDAV/CardDAV)").assertExists()
        compose.onNodeWithTag("account-link-CALENDAR").performScrollTo().performClick()
        compose.onAllNodesWithText("alice@example.org (CalDAV/CardDAV)").assertCountEquals(2)
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

        assertEquals(listOf(AccountPageIntent.Remove(linkedTo.id)), changes)
    }

    @Test
    fun removing_an_account_with_a_namesake_names_both_apart() {
        val mailbox = linked("alice@example.org", AccountDistinction.Kind(CoreAccountKind.IMAP))
        val calendar = calendarOnly.copy(
            address = "alice@example.org",
            linkedFrom = listOf(mailbox),
            distinction = AccountDistinction.Kind(CoreAccountKind.DAV),
        )
        state.page = calendar.id
        show(calendar)

        compose.onNodeWithTag("account-remove").performScrollTo().performClick()
        compose.onNodeWithText(
            "alice@example.org (CalDAV/CardDAV) leaves this app and its stored sign-in is deleted. " +
                "You can add it again later. It is also unlinked from alice@example.org (IMAP).",
        ).assertExists()
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
