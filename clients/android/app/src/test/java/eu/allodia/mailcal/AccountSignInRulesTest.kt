// When the "already set up" refusal offers to update the account already there (docs/accounts.md
// rule 17).
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class AccountSignInRulesTest {
    @Test
    fun only_imap_and_jmap_accounts_switch_here() {
        assertTrue(AccountSignInRules.switchable(CoreAccountKind.IMAP))
        assertTrue(AccountSignInRules.switchable(CoreAccountKind.JMAP))
        assertFalse(AccountSignInRules.switchable(CoreAccountKind.MICROSOFT))
        assertFalse(AccountSignInRules.switchable(CoreAccountKind.GOOGLE))
        assertFalse(AccountSignInRules.switchable(CoreAccountKind.DAV))
        assertFalse(AccountSignInRules.switchable(null))
    }

    @Test
    fun the_refusal_keeps_the_account_already_there_and_the_config_it_was_given() {
        val refusal = MailcalException.AlreadySetUp("alice@example.org@imap.example.org")
        val kinds = mapOf("alice@example.org@imap.example.org" to CoreAccountKind.IMAP)

        val existing = AccountSignInRules.existingAccount(refusal, "toml") { kinds[it] }

        assertEquals(ExistingAccount("alice@example.org@imap.example.org", "toml"), existing)
    }

    @Test
    fun the_refusal_offers_no_update_settings_could_not_make() {
        val refusal = MailcalException.AlreadySetUp("acct")

        assertNull(AccountSignInRules.existingAccount(refusal, "toml") { CoreAccountKind.DAV })
        assertNull(AccountSignInRules.existingAccount(refusal, "toml") { CoreAccountKind.MICROSOFT })
        // Not listed: nothing to say what it is.
        assertNull(AccountSignInRules.existingAccount(refusal, "toml") { null })
        // Any other failure is said as a failure, with nothing to offer.
        assertNull(
            AccountSignInRules.existingAccount(MailcalException.Connect("down"), "toml") {
                CoreAccountKind.IMAP
            },
        )
    }
}
