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
import uniffi.mailcal_bindings.UseProblem

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

    /** An IMAP or JMAP account's sign-in, switched or renewed, and what each server takes. */
    val signIn = AccountSignInState()

    /** A calendar or address book being given a sign-in of its own, from its account's page. */
    val separate = SeparateSignIn()

    /** A calendar or address book server found on another site, being asked about. */
    val serverChange = ServerChangePrompt()

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

    /**
     * What a prompt asking account [id] to sign in again names it by: [accountName], apart from an
     * account that shares its address, while the snapshot lists it; else [fallback], then the id.
     */
    fun nameOf(ctx: Context, id: String, fallback: String? = null): String =
        entry(id)?.let { accountName(ctx, it) } ?: fallback ?: id
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
    is MailcalException.DifferentAccount -> "that sign-in is for a different account"
    is MailcalException.StaleSignIn -> "the browser returned from another sign-in"
    is MailcalException.UseNotConnected -> useNotConnectedText(error.capability, error.problem)
    is MailcalException.Engine -> error.v1
    is MailcalException.Composer -> error.v1
    else -> error.message
}

/**
 * What to show a person for a core refusal: a refused sign-in in their language, since the
 * server's reply is meant for a developer, and an account already set up, a sign-in as another
 * account, a browser returning from another sign-in and a calendar or address book that did not
 * connect likewise; anything else as [coreErrorText] has it.
 */
internal fun coreErrorSaid(error: Throwable, ctx: Context): String? = when (error) {
    is MailcalException.SignInRefused -> L10n.signin_refused(ctx)
    is MailcalException.AlreadySetUp -> L10n.account_already_set_up(ctx)
    is MailcalException.DifferentAccount -> L10n.signin_different_account(ctx)
    is MailcalException.StaleSignIn -> L10n.signin_stale(ctx)
    is MailcalException.UseNotConnected -> useNotConnectedSaid(error.capability, error.problem, ctx)
    else -> coreErrorText(error)
}

/**
 * What a sign-in completion whose failures read as [fallback] says: the browser returning from
 * another sign-in has a remedy of its own (docs/provider-oauth.md rule 3), so it is said as that.
 */
internal fun signInFailedSaid(error: Throwable, ctx: Context, fallback: String): String = when (error) {
    is MailcalException.StaleSignIn -> L10n.signin_stale(ctx)
    else -> fallback
}

/**
 * What signing an existing account in again says when it did not complete, from a banner or its
 * page: a browser returning from another sign-in, a sign-in as another account and a refusal each
 * have words of their own; anything else is the plain retry line, the cause being for the log.
 */
internal fun signInAgainSaid(error: Throwable, ctx: Context): String = when (error) {
    is MailcalException.StaleSignIn -> L10n.signin_stale(ctx)
    is MailcalException.DifferentAccount -> L10n.signin_different_account(ctx)
    is MailcalException.SignInRefused -> L10n.signin_refused(ctx)
    else -> L10n.signin_expired_failed(ctx)
}

/**
 * What a calendar or address book that did not connect on a server of its own says
 * (docs/accounts.md rule 18). The core never names mail here; it reads as the calendar if it does.
 */
internal fun useNotConnectedSaid(capability: AccountCapability, problem: UseProblem, ctx: Context): String {
    val contacts = capability == AccountCapability.CONTACTS
    return when (problem) {
        UseProblem.UNREACHABLE -> if (contacts) {
            L10n.separate_server_unreachable_contacts(ctx)
        } else {
            L10n.separate_server_unreachable_calendar(ctx)
        }
        UseProblem.NEEDS_OWN_SIGN_IN -> if (contacts) {
            L10n.settings_account_use_own_sign_in_contacts(ctx)
        } else {
            L10n.settings_account_use_own_sign_in_calendar(ctx)
        }
        UseProblem.SIGN_IN_REFUSED -> L10n.signin_refused(ctx)
        UseProblem.SERVER_CHANGED -> if (contacts) {
            L10n.settings_account_use_server_changed_contacts(ctx)
        } else {
            L10n.settings_account_use_server_changed_calendar(ctx)
        }
    }
}

private fun useNotConnectedText(capability: AccountCapability, problem: UseProblem): String {
    val use = if (capability == AccountCapability.CONTACTS) "the address book" else "the calendar"
    val why = when (problem) {
        UseProblem.UNREACHABLE -> "its server could not be reached"
        UseProblem.NEEDS_OWN_SIGN_IN -> "its server needs a sign-in of its own"
        UseProblem.SIGN_IN_REFUSED -> "its server refused the sign-in"
        UseProblem.SERVER_CHANGED -> "its server was found on another site and waits to be allowed"
    }
    return "$use did not connect: $why"
}
