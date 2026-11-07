// What Settings → Accounts' list says about an account, from a snapshot entry (docs/accounts.md
// rule 11). An account's page is the core's, pinned in `account_page_tests.rs`.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountLinksView
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class AccountSettingsRulesTest {
    @Test
    fun a_use_waiting_on_permission_is_one_and_a_use_the_server_lacks_is_not() {
        val entry = accountEntry(
            CoreAccountKind.GOOGLE,
            AccountCapability.MAIL to CapabilityState.ON,
            AccountCapability.CALENDAR to CapabilityState.NEEDS_PERMISSION,
            AccountCapability.CONTACTS to CapabilityState.NOT_OFFERED,
        )
        assertTrue(AccountSettingsRules.needsPermission(entry))
        assertEquals(listOf(AccountCapability.CALENDAR), AccountSettingsRules.withheld(entry))
        // Listed as a use: it is chosen, only not granted.
        assertEquals(
            listOf(AccountCapability.MAIL, AccountCapability.CALENDAR),
            AccountSettingsRules.inUse(entry),
        )
        assertFalse(AccountSettingsRules.needsPermission(mailboxOnly()))
    }

    @Test
    fun signing_in_again_is_offered_where_the_provider_signs_in() {
        assertTrue(AccountSettingsRules.signsInAtProvider(CoreAccountKind.MICROSOFT))
        assertTrue(AccountSettingsRules.signsInAtProvider(CoreAccountKind.GOOGLE))
        listOf(CoreAccountKind.IMAP, CoreAccountKind.DAV, CoreAccountKind.JMAP).forEach {
            assertFalse(AccountSettingsRules.signsInAtProvider(it))
        }
    }

    @Test
    fun the_row_names_the_uses_in_order_and_the_links() {
        val entry = mailboxOnly(links = AccountLinksView(linked("alice@cloud.example"), null, null))
        assertEquals(listOf(AccountCapability.MAIL), AccountSettingsRules.inUse(entry))
        assertEquals(
            listOf(AccountCapability.CALENDAR to linked("alice@cloud.example")),
            AccountSettingsRules.linked(entry),
        )
    }

    @Test
    fun removing_names_the_accounts_that_lose_their_link() {
        assertNull(AccountSettingsRules.unlinked(mailboxOnly()) { it.address })
        val entry = mailboxOnly(linkedFrom = listOf(linked("a@example.org"), linked("b@example.org")))
        assertEquals("a@example.org, b@example.org", AccountSettingsRules.unlinked(entry) { it.address })
    }
}
