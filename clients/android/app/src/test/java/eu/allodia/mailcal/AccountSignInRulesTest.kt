// Which way in an IMAP or JMAP account's Sign-in section offers, which remedy an expired sign-in
// takes, and when the "already set up" refusal offers to update the account already there
// (docs/accounts.md rules 15 and 17).
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.AccountSignInOptions
import uniffi.mailcal_bindings.CapabilityState
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

class AccountSignInRulesTest {
    private fun entry(signIn: SignInMethod, kind: CoreAccountKind = CoreAccountKind.IMAP) =
        accountEntry(kind, AccountCapability.MAIL to CapabilityState.ON, signIn = signIn)

    private fun options(signIn: Boolean, password: Boolean, label: String? = null) =
        AccountSignInOptions(signIn = signIn, providerLabel = label, password = password)

    @Test
    fun nothing_is_offered_before_the_server_answers() {
        val section = AccountSignInRules.section(entry(SignInMethod.PROVIDER), null)

        assertEquals(SignInMethod.PROVIDER, section.method)
        assertFalse(section.signInAgain || section.toPassword || section.toProvider)
        assertFalse(section.offersSwitch)
    }

    @Test
    fun a_signed_in_account_signs_in_again_and_takes_a_password_where_the_server_does() {
        val both = AccountSignInRules.section(entry(SignInMethod.PROVIDER), options(true, true, "Fastmail"))
        assertTrue(both.signInAgain)
        assertTrue(both.toPassword)
        assertFalse(both.toProvider)
        assertTrue(both.offersSwitch)
        // The label names the switch to the provider, which this account does not need.
        assertNull(both.providerLabel)

        val noPassword = AccountSignInRules.section(entry(SignInMethod.PROVIDER), options(true, false))
        assertTrue(noPassword.signInAgain)
        assertFalse(noPassword.toPassword)
        // Signing in again is not a switch, so the explanatory line stays away.
        assertFalse(noPassword.offersSwitch)
    }

    @Test
    fun a_password_account_switches_to_its_provider_where_setup_would_offer_it() {
        val named = AccountSignInRules.section(entry(SignInMethod.PASSWORD), options(true, true, "Fastmail"))
        assertTrue(named.toProvider)
        assertEquals("Fastmail", named.providerLabel)
        assertFalse(named.signInAgain)
        assertFalse(named.toPassword)

        val unnamed = AccountSignInRules.section(entry(SignInMethod.PASSWORD), options(true, true))
        assertNull(unnamed.providerLabel)

        val none = AccountSignInRules.section(entry(SignInMethod.PASSWORD), options(false, true))
        assertFalse(none.offersSwitch)
    }

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
