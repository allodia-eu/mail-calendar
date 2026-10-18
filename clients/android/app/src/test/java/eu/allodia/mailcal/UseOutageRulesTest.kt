// The line a calendar or address book that did not connect draws above its screen: the title,
// the reason the account's own page gives for the same problem, and "last synced", in that order,
// for every problem and both uses (docs/accounts.md rule 16).
package eu.allodia.mailcal

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ConnectivitySnapshot
import uniffi.mailcal_bindings.UseOutage
import uniffi.mailcal_bindings.UseProblem

@RunWith(RobolectricTestRunner::class)
class UseOutageRulesTest {
    private fun ctx(): Context = ApplicationProvider.getApplicationContext()

    private fun expected(title: String, reason: String) =
        "$title $reason ${L10n.use_outage_last_synced(ctx())}"

    @Test
    fun a_calendar_says_its_title_then_the_settings_reason_then_last_synced() {
        val title = L10n.calendar_outage_title(ctx(), "ada@example.com")
        val reasons = mapOf(
            UseProblem.NEEDS_OWN_SIGN_IN to L10n.settings_account_use_own_sign_in_calendar(ctx()),
            UseProblem.SIGN_IN_REFUSED to L10n.settings_account_use_refused_calendar(ctx()),
            UseProblem.UNREACHABLE to L10n.settings_account_use_unreachable_calendar(ctx()),
        )
        assertEquals(UseProblem.entries.toSet(), reasons.keys)
        for ((problem, reason) in reasons) {
            assertEquals(
                expected(title, reason),
                UseOutageRules.text(ctx(), OutageUse.CALENDAR, "ada@example.com", problem),
            )
        }
    }

    @Test
    fun an_address_book_says_its_title_then_the_settings_reason_then_last_synced() {
        val title = L10n.contacts_outage_title(ctx(), "ada@example.com")
        val reasons = mapOf(
            UseProblem.NEEDS_OWN_SIGN_IN to L10n.settings_account_use_own_sign_in_contacts(ctx()),
            UseProblem.SIGN_IN_REFUSED to L10n.settings_account_use_refused_contacts(ctx()),
            UseProblem.UNREACHABLE to L10n.settings_account_use_unreachable_contacts(ctx()),
        )
        assertEquals(UseProblem.entries.toSet(), reasons.keys)
        for ((problem, reason) in reasons) {
            assertEquals(
                expected(title, reason),
                UseOutageRules.text(ctx(), OutageUse.CONTACTS, "ada@example.com", problem),
            )
        }
    }

    @Test
    fun the_english_line_reads_as_three_sentences() {
        assertEquals(
            "The calendar for ada@example.com isn’t connected. " +
                "Your calendar is on a server that needs a sign-in of its own. " +
                "You’re seeing what was last synced.",
            UseOutageRules.text(
                ctx(),
                OutageUse.CALENDAR,
                "ada@example.com",
                UseProblem.NEEDS_OWN_SIGN_IN,
            ),
        )
    }

    private fun snapshot(calendar: List<UseOutage>, contacts: List<UseOutage>) =
        ConnectivitySnapshot(
            offline = false,
            unreachableAccounts = emptyList(),
            calendarReauthAccounts = emptyList(),
            mailReauthAccounts = emptyList(),
            signinExpiredAccounts = emptyList(),
            calendarOutages = calendar,
            contactsOutages = contacts,
        )

    @Test
    fun each_outage_is_one_line_in_the_snapshot_order_named_by_its_address() {
        val addresses = mapOf("acct-b" to "bob@example.com", "acct-a" to "ada@example.com")
        val connectivity = snapshot(
            calendar = listOf(
                UseOutage("acct-b", UseProblem.UNREACHABLE),
                UseOutage("acct-a", UseProblem.SIGN_IN_REFUSED),
            ),
            contacts = emptyList(),
        )
        val lines = UseOutageRules.lines(ctx(), OutageUse.CALENDAR, connectivity) {
            addresses.getValue(it)
        }

        assertEquals(listOf("acct-b", "acct-a"), lines.map { it.accountId })
        assertTrue(lines[0].text.startsWith(L10n.calendar_outage_title(ctx(), "bob@example.com")))
        assertTrue(lines[1].text.startsWith(L10n.calendar_outage_title(ctx(), "ada@example.com")))
    }

    @Test
    fun each_screen_reads_only_its_own_use() {
        val connectivity = snapshot(
            calendar = emptyList(),
            contacts = listOf(UseOutage("acct-a", UseProblem.NEEDS_OWN_SIGN_IN)),
        )

        assertTrue(UseOutageRules.lines(ctx(), OutageUse.CALENDAR, connectivity) { it }.isEmpty())
        val contacts = UseOutageRules.lines(ctx(), OutageUse.CONTACTS, connectivity) { it }
        assertEquals(listOf("acct-a"), contacts.map { it.accountId })
    }

    @Test
    fun a_snapshot_without_an_outage_draws_no_line() {
        val connectivity = snapshot(calendar = emptyList(), contacts = emptyList())
        for (use in OutageUse.entries) {
            assertTrue(UseOutageRules.lines(ctx(), use, connectivity) { it }.isEmpty())
            assertTrue(UseOutageRules.lines(ctx(), use, null) { it }.isEmpty())
        }
    }
}
