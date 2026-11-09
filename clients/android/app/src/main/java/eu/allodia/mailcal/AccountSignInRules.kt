// Whether the "already set up" refusal offers to update the account already there
// (docs/accounts.md rule 17). Which controls an account's Sign-in section draws, and every step of
// its browser sign-in, is the core's (AccountPage.signIn). Compose-free and catalog-free, so the JVM
// suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

internal object AccountSignInRules {
    /** Whether an account of [kind] can take the credential of another setup of it. */
    fun switchable(kind: CoreAccountKind?): Boolean =
        kind == CoreAccountKind.IMAP || kind == CoreAccountKind.JMAP

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
