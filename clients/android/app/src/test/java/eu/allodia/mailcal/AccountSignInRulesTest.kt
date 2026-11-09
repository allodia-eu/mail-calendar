// Which remedy an expired sign-in takes, and when the "already set up" refusal offers to update the
// account already there (docs/accounts.md rules 15 and 17).
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountProvider
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
    fun a_sign_in_starts_with_the_redirect_setup_uses_for_its_kind() {
        assertEquals(ImapOAuthConfig.REDIRECT_URI, AccountSignInRules.redirectFor(CoreAccountKind.IMAP))
        assertEquals(JmapOAuthConfig.REDIRECT_URI, AccountSignInRules.redirectFor(CoreAccountKind.JMAP))
        assertNull(AccountSignInRules.redirectFor(CoreAccountKind.DAV))
    }

    @Test
    fun every_family_has_its_remedy() {
        val remedies = mapOf(
            AccountProvider.MICROSOFT to ExpiredRemedy.PROVIDER_CONSENT,
            AccountProvider.GOOGLE to ExpiredRemedy.PROVIDER_CONSENT,
            AccountProvider.JMAP_OAUTH to ExpiredRemedy.JMAP_REAUTH,
            // An IMAP account that signed in holds no password for Settings to ask for.
            AccountProvider.IMAP_OAUTH to ExpiredRemedy.ACCOUNT_SIGN_IN,
            AccountProvider.PASSWORD to ExpiredRemedy.SETTINGS,
            AccountProvider.JMAP to ExpiredRemedy.SETTINGS,
        )
        assertEquals(AccountProvider.entries.toSet(), remedies.keys)
        remedies.forEach { (provider, remedy) ->
            assertEquals(provider.name, remedy, AccountSignInRules.expiredRemedy(provider))
        }
        assertEquals(ExpiredRemedy.SETTINGS, AccountSignInRules.expiredRemedy(null))
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
