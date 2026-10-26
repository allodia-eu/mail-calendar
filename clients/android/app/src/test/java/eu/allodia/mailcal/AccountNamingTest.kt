// Accounts that share an address, as Settings names them (docs/accounts.md rule 8). The same rules
// the Apple, Linux and Windows suites pin.
package eu.allodia.mailcal

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountDistinction
import uniffi.mailcal_bindings.AccountLinksView
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

@RunWith(RobolectricTestRunner::class)
class AccountNamingTest {
    private val ctx: Context = ApplicationProvider.getApplicationContext()

    @Test
    fun an_address_no_other_account_has_is_named_by_itself() {
        assertEquals("alice@example.org", accountName(ctx, linked("alice@example.org")))
    }

    @Test
    fun a_kind_no_namesake_shares_is_named_beside_the_address() {
        val calendar = linked("alice@example.org", AccountDistinction.Kind(CoreAccountKind.DAV))
        assertEquals("alice@example.org (CalDAV/CardDAV)", accountName(ctx, calendar))
    }

    @Test
    fun a_shared_kind_is_told_apart_by_its_server() {
        val other = linked("alice@example.org", AccountDistinction.Server("mail.example.org"))
        assertEquals("alice@example.org (mail.example.org)", accountName(ctx, other))
    }

    @Test
    fun the_kind_line_gains_the_host_for_a_server_distinction_only() {
        assertEquals("IMAP", kindLine(ctx, mailboxOnly()))
        val byKind = accountEntry(
            CoreAccountKind.IMAP,
            AccountCapability.MAIL to CapabilityState.ON,
            distinction = AccountDistinction.Kind(CoreAccountKind.IMAP),
        )
        assertEquals("IMAP", kindLine(ctx, byKind))
        val byServer = byKind.copy(distinction = AccountDistinction.Server("mail.example.org"))
        assertEquals("IMAP · mail.example.org", kindLine(ctx, byServer))
        assertEquals("IMAP · mail.example.org · Mail", accountSummary(ctx, byServer))
    }

    @Test
    fun a_mailbox_and_its_split_off_calendar_do_not_read_as_one_account() {
        val calendar = linked("alice@example.org", AccountDistinction.Kind(CoreAccountKind.DAV))
        val mailbox = mailboxOnly(
            links = AccountLinksView(calendar, null, null),
            linkedFrom = listOf(calendar),
        )
        assertEquals(
            "IMAP · Mail\nCalendar: alice@example.org (CalDAV/CardDAV)",
            accountSummary(ctx, mailbox),
        )
        assertEquals(
            AccountRemoval(name = "alice@example.org", unlinked = "alice@example.org (CalDAV/CardDAV)"),
            accountRemoval(ctx, mailbox),
        )
    }

    @Test
    fun a_removal_names_the_account_it_removes_apart_from_its_namesake() {
        val mailbox = linked("alice@example.org", AccountDistinction.Kind(CoreAccountKind.IMAP))
        val calendar = accountEntry(
            CoreAccountKind.DAV,
            AccountCapability.CALENDAR to CapabilityState.ON,
            linkedFrom = listOf(mailbox),
            distinction = AccountDistinction.Kind(CoreAccountKind.DAV),
        )
        assertEquals("alice@example.org (CalDAV/CardDAV)", accountName(ctx, calendar))
        assertEquals(
            AccountRemoval(
                name = "alice@example.org (CalDAV/CardDAV)",
                unlinked = "alice@example.org (IMAP)",
            ),
            accountRemoval(ctx, calendar),
        )
        val byServer = calendar.copy(distinction = AccountDistinction.Server("dav.example.org"))
        assertEquals("alice@example.org (dav.example.org)", accountRemoval(ctx, byServer).name)
    }

    @Test
    fun a_prompt_to_sign_in_again_names_a_listed_account_apart_from_its_namesake() {
        val calendar = accountEntry(
            CoreAccountKind.DAV,
            AccountCapability.CALENDAR to CapabilityState.ON,
            id = "alice@example.org@dav:cloud.example",
            distinction = AccountDistinction.Kind(CoreAccountKind.DAV),
        )
        val state = AccountsUiState()
        state.snapshot = AccountsSnapshot(listOf(calendar, mailboxOnly()))
        assertEquals("alice@example.org (CalDAV/CardDAV)", state.nameOf(ctx, calendar.id))
        assertEquals("alice@example.org", state.nameOf(ctx, mailboxOnly().id))
        // Not listed yet: the switcher's address, then the id.
        assertEquals("bob@example.org", state.nameOf(ctx, "bob@imap", "bob@example.org"))
        assertEquals("bob@imap", state.nameOf(ctx, "bob@imap"))
    }
}
