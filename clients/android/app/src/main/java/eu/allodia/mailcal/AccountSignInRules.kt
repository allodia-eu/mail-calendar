// How an account signs in again (docs/accounts.md rule 17): which remedy an expired sign-in takes,
// and whether the "already set up" refusal offers to update the account already there. Which
// controls its page's Sign-in section draws is the core's (AccountPage.signIn). Compose-free and
// catalog-free, so the JVM suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/** What the expired-sign-in banner does for an account. */
internal enum class ExpiredRemedy {
    /** Microsoft or Google: signed in again through their own consent. */
    PROVIDER_CONSENT,

    /** A JMAP account that signed in: its own grant authorised again. */
    JMAP_REAUTH,

    /** An IMAP account that signed in, or one a Nextcloud sign-in made: the account sign-in. */
    ACCOUNT_SIGN_IN,

    /** A password or a pasted secret, or an unknown family: replaced on the account's page. */
    SETTINGS,
}

internal object AccountSignInRules {
    /** Whether an account of [kind] can take the credential of another setup of it. */
    fun switchable(kind: CoreAccountKind?): Boolean =
        kind == CoreAccountKind.IMAP || kind == CoreAccountKind.JMAP

    /**
     * The redirect a sign-in of an account of [kind] is started with: the one setup uses for that
     * kind. An account that already signs in through its provider keeps its grant's own redirect,
     * which is one of these two, so both hosts are routed to the account sign-in while it waits.
     */
    fun redirectFor(kind: CoreAccountKind?): String? = when (kind) {
        CoreAccountKind.IMAP -> ImapOAuthConfig.REDIRECT_URI
        CoreAccountKind.JMAP -> JmapOAuthConfig.REDIRECT_URI
        else -> null
    }

    /**
     * The banner's remedy for an account whose sign-in [provider] expired; [nextcloud] for one a
     * Nextcloud sign-in made, whatever its provider says.
     */
    fun expiredRemedy(provider: AccountProvider?, nextcloud: Boolean = false): ExpiredRemedy = when {
        nextcloud -> ExpiredRemedy.ACCOUNT_SIGN_IN
        else -> expiredRemedyFor(provider)
    }

    private fun expiredRemedyFor(provider: AccountProvider?): ExpiredRemedy = when (provider) {
        AccountProvider.MICROSOFT, AccountProvider.GOOGLE -> ExpiredRemedy.PROVIDER_CONSENT
        AccountProvider.JMAP_OAUTH -> ExpiredRemedy.JMAP_REAUTH
        AccountProvider.IMAP_OAUTH -> ExpiredRemedy.ACCOUNT_SIGN_IN
        AccountProvider.PASSWORD, AccountProvider.JMAP, null -> ExpiredRemedy.SETTINGS
    }

    /**
     * The account an add was refused for, when the refusal can offer to put the credential just
     * given on it: an IMAP or JMAP account, whose sign-in Settings can switch. [kindOf] reads the
     * account's kind from the snapshot.
     */
    fun existingAccount(
        error: Throwable,
        configToml: String,
        kindOf: (String) -> CoreAccountKind?,
    ): ExistingAccount? {
        val refusal = error as? MailcalException.AlreadySetUp ?: return null
        if (!switchable(kindOf(refusal.accountId))) return null
        return ExistingAccount(refusal.accountId, configToml)
    }
}

/**
 * The account an add was refused for, the config that add was given, and what the sign-in that
 * config came from will not reach, asked about before the update (docs/accounts.md rule 18).
 */
internal data class ExistingAccount(
    val accountId: String,
    val configToml: String,
    val unreached: List<AccountCapability> = emptyList(),
    /** A Nextcloud sign-in's grant, given in place of a config (docs/nextcloud-sign-in.md). */
    val nextcloudGrant: String? = null,
)
