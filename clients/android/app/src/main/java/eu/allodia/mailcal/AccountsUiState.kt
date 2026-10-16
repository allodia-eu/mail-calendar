// Settings → Accounts' state on this screen: the core's one snapshot of every account, the page
// open, and what the last change came to. Held by the activity rather than the Settings screen,
// because signing an account in again leaves for the browser and comes back through onNewIntent,
// and the page has to still be open when it does.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.MailcalException

/** How a notice reads: a success is said briefly, a warning or failure waits to be dismissed. */
internal enum class NoticeKind { SUCCESS, WARNING, FAILURE }

/** What an account's page or the list says about the last change. */
internal data class AccountNotice(val kind: NoticeKind, val title: String, val message: String)

/** A use being switched, drawn at its new position and held still until the core answers. */
internal data class PendingUse(
    val accountId: String,
    val capability: AccountCapability,
    val on: Boolean,
)

/** A sign-in in the browser for an existing account, waiting for its redirect. */
internal data class PendingConsent(
    val accountId: String,
    val google: Boolean,
    val handle: String,
)

internal class AccountsUiState {
    /** Every account, mail or not, as the core last listed it. */
    var snapshot by mutableStateOf(AccountsSnapshot(emptyList()))

    /** Whether the last snapshot listed an account: the last one going returns to setup. */
    var hadAccounts = false

    /** The account whose page is open in Settings → Accounts, or null for the list. */
    var page by mutableStateOf<String?>(null)

    /** What the last change said, until it is dismissed or the page closes. */
    var notice by mutableStateOf<AccountNotice?>(null)

    /** A use switch the core is still answering. */
    var pendingUse by mutableStateOf<PendingUse?>(null)

    /** The account whose servers are being tried. */
    var savingServers by mutableStateOf<String?>(null)

    /** Counts saved server edits, so the form starts again from what was saved. */
    var serversSaved by mutableStateOf(0)

    /** The account whose pasted secret is being tried. */
    var replacingSecret by mutableStateOf<String?>(null)

    /** The account being signed in again, from the browser hop until its connect answers. */
    var signingIn by mutableStateOf<String?>(null)

    /** The consent sign-in the next Microsoft or Google redirect completes, if any. */
    var consent: PendingConsent? = null

    /**
     * A first sign-in from the setup form claims the redirect a sign-in again was waiting for. That
     * one can no longer finish, so its page stops saying it is under way, or "Sign in again" would
     * stay disabled with nothing left to enable it.
     */
    fun yieldConsent() {
        if (consent == null) return
        consent = null
        signingIn = null
    }

    /** The account [id] names, if it is still listed. */
    fun entry(id: String): AccountEntry? = snapshot.accounts.firstOrNull { it.id == id }

    /**
     * What a person reads account [id] by (docs/accounts.md rule 8): its address from the snapshot,
     * which names every account, mail or not, then [fallback], then the id.
     */
    fun addressOf(id: String, fallback: String? = null): String =
        entry(id)?.address ?: fallback ?: id
}

/**
 * The words a core refusal carries, without the record's own field names: a generated
 * `MailcalException` renders as `v1=...`, which is not something to put in front of anyone.
 */
internal fun coreErrorText(error: Throwable): String? = when (error) {
    is MailcalException.Config -> error.v1
    is MailcalException.Connect -> error.v1
    is MailcalException.CertificateRejected -> error.reason
    is MailcalException.SignInRefused -> error.reason
    // The record carries the account's id, an address; the core's own words carry none.
    is MailcalException.AlreadySetUp -> "that account is already set up"
    is MailcalException.Engine -> error.v1
    is MailcalException.Composer -> error.v1
    else -> error.message
}

/**
 * What to show a person for a core refusal: a refused sign-in in their language, since the
 * server's reply is meant for a developer, and an account already set up likewise; anything else
 * as [coreErrorText] has it.
 */
internal fun coreErrorSaid(error: Throwable, ctx: Context): String? = when (error) {
    is MailcalException.SignInRefused -> L10n.signin_refused(ctx)
    is MailcalException.AlreadySetUp -> L10n.account_already_set_up(ctx)
    else -> coreErrorText(error)
}
