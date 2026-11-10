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
import uniffi.mailcal_bindings.AccountPage
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.UseProblem

/** How a notice reads: a success is said briefly, a warning or failure waits to be dismissed. */
internal enum class NoticeKind { SUCCESS, WARNING, FAILURE }

/** What an account's page or the list says about the last change. */
internal data class AccountNotice(val kind: NoticeKind, val title: String, val message: String)

internal class AccountsUiState {
    /** Every account, mail or not, as the core last listed it. */
    var snapshot by mutableStateOf(AccountsSnapshot(emptyList()))

    /** Whether the last snapshot listed an account: the last one going returns to setup. */
    var hadAccounts = false

    /** The account whose page is open in Settings → Accounts, or null for the list. */
    var page by mutableStateOf<String?>(null)

    /** What the last change said, until it is dismissed or the page closes. */
    var notice by mutableStateOf<AccountNotice?>(null)

    /** The open account's page as the core decided it: its uses, links and removal. */
    var accountPage by mutableStateOf<AccountPage?>(null)

    /** The last of the core's change notices said, so each is said once. */
    var coreNotice: ULong = 0u

    /** The notice a change on the page left, which the core withdraws when the next one starts. */
    var changeNotice: AccountNotice? = null

    /** A calendar or address book being given a sign-in of its own, from its account's page. */
    val separate = SeparateSignIn()

    /** A calendar or address book server found on another site, being asked about. */
    val serverChange = ServerChangePrompt()

    /** The account [id] names, if it is still listed. */
    fun entry(id: String): AccountEntry? = snapshot.accounts.firstOrNull { it.id == id }

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
        UseProblem.NONE_ON_SERVER -> if (contacts) {
            L10n.settings_account_use_none_on_server_contacts(ctx)
        } else {
            L10n.settings_account_use_none_on_server_calendar(ctx)
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
        UseProblem.NONE_ON_SERVER -> "its server has none for this account"
    }
    return "$use did not connect: $why"
}
