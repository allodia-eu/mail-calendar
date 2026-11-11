// An account's page where links join it to another (docs/accounts.md rule 12): a use a link fills
// is the account it comes from, a row that opens its page; the accounts taking a use from it are
// listed under "Linked to", each with Stop linking, asked first; and what a completed separate
// sign-in did is explained until it is dismissed.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountChangeNotice
import uniffi.mailcal_bindings.AccountChangeRefusal
import uniffi.mailcal_bindings.AccountDistinction
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountLinkedFrom
import uniffi.mailcal_bindings.AccountPage
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class SettingsAccountLinkedTest {
    @get:Rule val compose = createComposeRule()

    private val ctx = RuntimeEnvironment.getApplication()
    private val state = AccountsUiState()
    private val changes = mutableListOf<AccountPageIntent>()

    private val mailbox = mailboxOnly()
    private val cloud = accountEntry(
        CoreAccountKind.DAV,
        AccountCapability.MAIL to CapabilityState.OFF,
        AccountCapability.CALENDAR to CapabilityState.ON,
        AccountCapability.CONTACTS to CapabilityState.ON,
        id = "alice@dav:cloud.example",
        address = "alice@example.org",
        distinction = AccountDistinction.Kind(CoreAccountKind.DAV),
    )
    private val cloudLinked = linked(cloud.id, AccountDistinction.Kind(CoreAccountKind.DAV))
        .copy(address = cloud.address)

    /** Shows [accounts] with [page] open; opening another page draws it from [pageOf]. */
    private fun show(vararg accounts: AccountEntry, page: AccountPage) {
        state.snapshot = AccountsSnapshot(accounts.toList())
        state.page = page.account
        state.accountPage = page
        val ui = AccountsSettingsUi(
            state = state,
            onOpenPage = { id ->
                state.page = id
                state.accountPage = id?.let(state::entry)?.let { pageOf(it) }
            },
            onChange = { changes += it },
        )
        compose.setContent {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                AccountsSettings(ui, allodiaSyncCard = {}) { shown, record ->
                    AccountPage(shown, record, ui, syncMode = null, onSetSyncMode = {})
                }
            }
        }
        compose.waitForIdle()
    }

    private fun linkedUse(control: ControlState = FREE) = pageOf(
        mailbox,
        uses = listOf(
            useSwitch(AccountCapability.MAIL, on = true),
            useSwitch(AccountCapability.CALENDAR, on = false, control = control, linked = cloudLinked),
        ),
    )

    private fun taking(
        uses: List<AccountCapability>,
        leavesWithout: List<AccountCapability> = emptyList(),
        stop: ControlState = FREE,
    ) = AccountLinkedFrom(linked(mailbox.id).copy(address = mailbox.address), uses, leavesWithout, stop)

    @Test
    fun a_linked_use_is_a_row_naming_its_account_that_opens_its_page() {
        show(mailbox, cloud, page = linkedUse())

        compose.onNodeWithTag("account-use-CALENDAR").assertDoesNotExist()
        // One button, read with both lines, the account named apart from its namesake.
        compose.onNodeWithTag("account-use-linked-CALENDAR")
            .assert(hasText(L10n.nav_calendar(ctx)))
            .assert(hasText("From alice@example.org (CalDAV/CardDAV)"))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
            .assertIsEnabled()
            .performClick()
        compose.waitForIdle()

        assertEquals(cloud.id, state.page)
        assertEquals(emptyList<Any>(), changes)
    }

    @Test
    fun a_linked_use_is_held_while_a_change_runs_on_the_page() {
        show(mailbox, cloud, page = linkedUse(control = ControlState(enabled = false, running = true)))

        compose.onNodeWithTag("account-use-linked-CALENDAR").assertIsNotEnabled()
        compose.onNodeWithTag("account-use-linked-CALENDAR-running").assertDoesNotExist()
    }

    @Test
    fun linked_to_names_each_account_with_what_it_takes() {
        val bob = AccountLinkedFrom(
            linked("bob@example.org"),
            listOf(AccountCapability.CONTACTS),
            emptyList(),
            FREE,
        )
        show(
            cloud,
            page = pageOf(cloud, linkedFrom = listOf(taking(listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)), bob)),
        )

        compose.onNodeWithText("Linked to").performScrollTo()
        compose.onNodeWithText("Uses this account’s calendar and address book").assertExists()
        compose.onNodeWithText("bob@example.org").assertExists()
        compose.onNodeWithText(L10n.settings_account_linked_from_contacts(ctx)).assertExists()
    }

    @Test
    fun no_account_taking_a_use_draws_no_group() {
        show(cloud, page = pageOf(cloud))

        compose.onNodeWithText(L10n.settings_account_linked_from_heading(ctx)).assertDoesNotExist()
    }

    @Test
    fun stop_linking_asks_first_and_sends_only_when_confirmed() {
        show(cloud, page = pageOf(cloud, linkedFrom = listOf(taking(listOf(AccountCapability.CALENDAR)))))

        compose.onNodeWithTag("account-stop-linking-${mailbox.id}").performScrollTo().performClick()
        compose.onNodeWithText("Stop linking alice@example.org?").assertExists()
        compose.onNodeWithTag("account-stop-linking-message")
            .assert(hasText(L10n.settings_account_stop_linking_calendar(ctx)))
            .assert(hasText(L10n.settings_account_stop_linking_left_without(ctx), substring = true).not())
        compose.onNodeWithText(L10n.action_cancel(ctx)).performClick()
        compose.waitForIdle()
        assertEquals(emptyList<Any>(), changes)

        compose.onNodeWithTag("account-stop-linking-${mailbox.id}").performScrollTo().performClick()
        compose.onNodeWithTag("account-stop-linking-confirm").performClick()

        assertEquals(listOf(AccountPageIntent.StopLinking(cloud.id, mailbox.id)), changes)
    }

    @Test
    fun stopping_says_what_the_account_is_left_without() {
        val both = listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)
        assertEquals(
            "Its calendar and address book will no longer come from this account.",
            stopLinkingMessage(ctx, taking(both)),
        )
        assertEquals(
            L10n.settings_account_stop_linking_contacts(ctx) + " " +
                L10n.settings_account_stop_linking_left_without(ctx),
            stopLinkingMessage(ctx, taking(listOf(AccountCapability.CONTACTS), listOf(AccountCapability.CONTACTS))),
        )
    }

    @Test
    fun stop_linking_spins_while_it_runs_and_is_held() {
        val running = ControlState(enabled = false, running = true)
        show(cloud, page = pageOf(cloud, linkedFrom = listOf(taking(listOf(AccountCapability.CALENDAR), stop = running))))

        compose.onNodeWithTag("account-stop-linking-${mailbox.id}").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("account-stop-linking-${mailbox.id}-running", useUnmergedTree = true).assertExists()
    }

    @Test
    fun what_stopping_and_a_separate_sign_in_came_to_is_said() {
        val failed = changeNotice(ctx, AccountChangeNotice.StopLinkingFailed(AccountChangeRefusal.Other("locked")))
        assertEquals(AccountNotice(NoticeKind.FAILURE, "Couldn’t change this account", "locked"), failed)

        val both = changeNotice(
            ctx,
            AccountChangeNotice.SignedInSeparately(listOf(AccountCapability.CALENDAR, AccountCapability.CONTACTS)),
        )
        assertEquals(
            AccountNotice(
                NoticeKind.EXPLANATION,
                L10n.settings_account_signed_in_separately_title(ctx),
                L10n.settings_account_signed_in_separately_both(ctx),
            ),
            both,
        )
        val contacts = changeNotice(ctx, AccountChangeNotice.SignedInSeparately(listOf(AccountCapability.CONTACTS)))
        assertEquals(L10n.settings_account_signed_in_separately_contacts(ctx), contacts.message)
        val calendar = changeNotice(ctx, AccountChangeNotice.SignedInSeparately(listOf(AccountCapability.CALENDAR)))
        assertEquals(L10n.settings_account_signed_in_separately_calendar(ctx), calendar.message)
    }

    @Test
    fun a_separate_sign_in_is_explained_until_it_is_dismissed() {
        state.notice = changeNotice(ctx, AccountChangeNotice.SignedInSeparately(listOf(AccountCapability.CALENDAR)))
        show(mailbox, page = pageOf(mailbox))

        compose.onNodeWithText("Signed in separately").assertExists()
        compose.mainClock.advanceTimeBy(10_000)
        compose.onNodeWithText(L10n.settings_account_signed_in_separately_calendar(ctx)).assertExists()
        compose.onNodeWithText(L10n.action_close(ctx)).performClick()
        compose.waitForIdle()

        assertNull(state.notice)
    }
}
