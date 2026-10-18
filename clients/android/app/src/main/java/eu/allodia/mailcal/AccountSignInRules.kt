// How an IMAP or JMAP account signs in, and what changes it (docs/accounts.md rule 17): which
// controls its page's Sign-in section draws, which remedy an expired sign-in takes, and whether
// the "already set up" refusal offers to update the account already there. Compose-free and
// catalog-free, so the JVM suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.AccountSignInOptions
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.SignInMethod
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/**
 * What an account's Sign-in section draws under its status line. Every control is false until the
 * server has answered, and when it offered nothing.
 */
internal data class SignInSection(
    val method: SignInMethod,
    /** Signs in through its provider, and can do so again. */
    val signInAgain: Boolean,
    /** Signs in through its provider, and the server takes a password. */
    val toPassword: Boolean,
    /** Signs in with a password, and the server offers signing in through the provider. */
    val toProvider: Boolean,
    /** The provider's name for [toProvider]'s button, when the build knows one. */
    val providerLabel: String?,
) {
    /** Whether the section offers another way in, which the explanatory line goes under. */
    val offersSwitch: Boolean get() = toPassword || toProvider
}

/** What the expired-sign-in banner does for an account. */
internal enum class ExpiredRemedy {
    /** Microsoft or Google: signed in again through their own consent. */
    PROVIDER_CONSENT,

    /** A JMAP account that signed in: its own grant authorised again. */
    JMAP_REAUTH,

    /** An IMAP account that signed in: signed in again through the account sign-in. */
    ACCOUNT_SIGN_IN,

    /** A password or a pasted secret, or an unknown family: replaced on the account's page. */
    SETTINGS,
}

internal object AccountSignInRules {
    /** Whether an account of [kind] has a Sign-in section that can switch how it signs in. */
    fun switchable(kind: CoreAccountKind?): Boolean =
        kind == CoreAccountKind.IMAP || kind == CoreAccountKind.JMAP

    /** The section for [entry], given what its server answered ([options], null until then). */
    fun section(entry: AccountEntry, options: AccountSignInOptions?): SignInSection {
        val provider = entry.signIn == SignInMethod.PROVIDER
        return SignInSection(
            method = entry.signIn,
            signInAgain = provider && options?.signIn == true,
            toPassword = provider && options?.password == true,
            toProvider = !provider && options?.signIn == true,
            providerLabel = options?.providerLabel?.takeIf { !provider && it.isNotBlank() },
        )
    }

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

    /** The banner's remedy for an account whose sign-in [provider] expired. */
    fun expiredRemedy(provider: AccountProvider?): ExpiredRemedy = when (provider) {
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
)
