// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). Each change may touch the
// Keystore, delete local data or dial a server, so it runs off the main thread and answers with
// what the page should say.
package eu.allodia.mailcal

import android.widget.Toast
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind
import uniffi.mailcal_bindings.LinkSlot

/**
 * Reads the snapshot again. The last account gone returns the app to first-run setup, which is
 * decided here rather than from the mailbox: an account without mail is in none of its rows
 * (docs/accounts.md rule 11).
 */
internal fun MainActivity.refreshAccountsSnapshot() {
    val instance = app ?: return
    val snapshot = instance.accountsSnapshot()
    accountSettings.snapshot = snapshot
    val any = snapshot.accounts.isNotEmpty()
    if (!any && accountSettings.hadAccounts) {
        logUiInfo("accounts: the last account is gone; returning to first-run setup")
        showingSettings = false
        settingsCategory = null
        addingAccount = false
        accountSettings.page = null
        needsSetup = true
    }
    accountSettings.hadAccounts = any
    if (accountSettings.page?.let { accountSettings.entry(it) } == null) {
        accountSettings.page = null
    }
}

/** Opens Settings on account [id]'s page: where a password is replaced and a use asked for. */
internal fun MainActivity.openAccountPage(id: String) {
    accountSettings.notice = null
    accountSettings.page = id
    settingsCategory = SettingsCategory.ACCOUNTS
    showingSettings = true
}

/**
 * Says [notice] where the person is: on the account's page while Settings is open, and in passing
 * otherwise, since a sign-in started from a banner ends on the mailbox and a notice kept for
 * Settings would greet them there later about something long over.
 */
private fun MainActivity.say(notice: AccountNotice) {
    if (showingSettings) {
        accountSettings.notice = notice
    } else {
        val text = listOf(notice.title, notice.message).filter { it.isNotEmpty() }.joinToString(": ")
        Toast.makeText(this, text, Toast.LENGTH_LONG).show()
    }
}

private fun MainActivity.changeFailed(detail: String?) = AccountNotice(
    NoticeKind.FAILURE,
    L10n.settings_account_change_failed_title(this),
    detail ?: L10n.error_unknown(this),
)

/** Switches one of an account's uses on or off, and asks the provider when the core says to. */
internal fun MainActivity.setAccountUse(accountId: String, capability: AccountCapability, on: Boolean) {
    val instance = app ?: return
    if (accountSettings.pendingUse != null) return
    accountSettings.pendingUse = PendingUse(accountId, capability, on)
    thread(name = "mailcal-account-use") {
        // Blocking: switching a use off deletes what the device holds of it.
        val answer = runCatching { instance.setAccountCapability(accountId, capability, on) }
        mainHandler.post {
            accountSettings.pendingUse = null
            answer.onFailure {
                logUiWarn("accounts: switching a use failed: ${coreErrorText(it)}")
                accountSettings.notice = changeFailed(coreErrorText(it))
            }
            answer.onSuccess { change ->
                when (AccountSettingsRules.outcome(change)) {
                    // The provider has not granted it: the person switching it on is the person
                    // who would be asked, so they are asked now.
                    UseChangeOutcome.ASK_PROVIDER -> signInAgain(accountId, listOf(capability))
                    UseChangeOutcome.NEEDS_ENDPOINT -> accountSettings.notice =
                        changeFailed(L10n.settings_account_use_needs_endpoint(this))
                    UseChangeOutcome.APPLIED -> Unit
                }
            }
            refreshAccountsSnapshot()
        }
    }
}

/** Links an account to another for one slot, or clears that link when [target] is null. */
internal fun MainActivity.setAccountLink(accountId: String, slot: LinkSlot, target: String?) {
    val instance = app ?: return
    thread(name = "mailcal-account-link") {
        // Stored through the Keystore, which may block.
        val answer = runCatching { instance.setAccountLink(accountId, slot, target) }
        mainHandler.post {
            answer.onFailure {
                logUiWarn("accounts: linking an account failed: ${coreErrorText(it)}")
                accountSettings.notice = changeFailed(coreErrorText(it))
            }
            refreshAccountsSnapshot()
        }
    }
}

/**
 * Tries an account's edited servers and sign-in, and keeps them only if they connect. A refusal
 * leaves the form as the person typed it, password included, so one field is corrected rather
 * than all of them retyped.
 */
internal fun MainActivity.saveAccountEndpoints(accountId: String, endpoints: AccountEndpoints) {
    val instance = app ?: return
    accountSettings.savingServers = accountId
    thread(name = "mailcal-account-servers") {
        val answer = runCatching { instance.updateAccountEndpoints(accountId, endpoints) }
        mainHandler.post {
            accountSettings.savingServers = null
            answer.onSuccess {
                // The form starts again from what was saved, the password field empty.
                accountSettings.serversSaved += 1
                accountSettings.notice = AccountNotice(
                    NoticeKind.SUCCESS,
                    L10n.settings_account_servers_saved(this),
                    "",
                )
                refreshConnectivity()
            }
            answer.onFailure {
                // The reason, never the certificate: a refusal's message carries the whole of it
                // (docs/certificate-exceptions.md rule 9).
                logUiWarn("accounts: editing an account's servers failed")
                accountSettings.notice = AccountNotice(
                    NoticeKind.FAILURE,
                    L10n.settings_account_servers_failed(this),
                    coreErrorText(it) ?: L10n.error_unknown(this),
                )
            }
            refreshAccountsSnapshot()
        }
    }
}

/** Replaces a JMAP account's pasted password or API token, after proving it connects. */
internal fun MainActivity.replaceAccountSecret(accountId: String, secret: String) {
    val instance = app ?: return
    accountSettings.replacingSecret = accountId
    thread(name = "mailcal-account-secret") {
        val answer = runCatching { instance.replaceAccountSecret(accountId, secret) }
        mainHandler.post {
            accountSettings.replacingSecret = null
            answer.onSuccess {
                accountSettings.notice = AccountNotice(
                    NoticeKind.SUCCESS,
                    L10n.settings_account_signed_in(this),
                    "",
                )
                refreshConnectivity()
            }
            answer.onFailure {
                logUiWarn("accounts: replacing an account's secret failed: ${coreErrorText(it)}")
                accountSettings.notice = AccountNotice(
                    NoticeKind.FAILURE,
                    L10n.settings_account_signin_failed(this),
                    coreErrorText(it) ?: L10n.error_unknown(this),
                )
            }
        }
    }
}

/**
 * Removes account [id] off the main thread: the core lets go of it (its sync stops, it leaves
 * every surface, the links other accounts hold to it are cleared) and deletes its stored
 * credential. What can still fail is that erase, which would bring the account back at the next
 * launch, so it is said wherever the person is left rather than left to surprise them.
 */
internal fun MainActivity.removeAccount(id: String) {
    val instance = app ?: return
    if (accountSettings.page == id) accountSettings.page = null
    thread(name = "mailcal-remove-account") {
        val answer = runCatching { instance.removeAccount(id) }
        mainHandler.post {
            answer.onFailure {
                logUiWarn("remove account: the credential could not be erased: ${coreErrorText(it)}")
                val said = L10n.remove_account_failed(this, coreErrorText(it) ?: L10n.error_unknown(this))
                say(AccountNotice(NoticeKind.FAILURE, "", said))
            }
            refreshAccountsSnapshot()
        }
    }
}

/**
 * Signs an existing Microsoft or Google account in again at its provider, asking for what it is
 * used for plus [adding]. The account keeps its id, settings, links and downloaded mail
 * (docs/provider-oauth.md rule 11), where a first sign-in would add it again and download it
 * afresh. The redirect comes back through onNewIntent to [completeAccountConsent].
 */
internal fun MainActivity.signInAgain(accountId: String, adding: List<AccountCapability>) {
    val instance = app ?: return
    // One at a time: a second tab would claim the redirect, and the first could no longer finish.
    if (accountSettings.signingIn != null) return
    val google = accountSettings.entry(accountId)?.kind == CoreAccountKind.GOOGLE ||
        instance.accountProvider(accountId) == uniffi.mailcal_bindings.AccountProvider.GOOGLE
    val redirect = if (google) GoogleOAuthConfig.REDIRECT_URI else MicrosoftOAuthConfig.REDIRECT_URI
    if (redirect == null) {
        say(changeFailed(L10n.account_signin_browser_failed(this)))
        return
    }
    try {
        val start = instance.beginAccountConsent(accountId, redirect, adding)
        accountSettings.consent = PendingConsent(accountId, google, start.pending)
        accountSettings.signingIn = accountId
        // A first sign-in from the setup form shares this redirect; this one claims it.
        pendingMicrosoftLogin = null
        pendingGoogleLogin = null
        if (google) {
            openGoogleSignIn(this, start.authorizationUrl)
        } else {
            openMicrosoftSignIn(this, start.authorizationUrl)
        }
    } catch (e: Exception) {
        logUiWarn("accounts: signing an account in again failed to start: ${coreErrorText(e)}")
        accountSettings.signingIn = null
        accountSettings.consent = null
        say(
            AccountNotice(
                NoticeKind.FAILURE,
                L10n.settings_account_signin_failed(this),
                coreErrorText(e) ?: L10n.account_signin_browser_failed(this),
            ),
        )
    }
}

/** Whether the next redirect of this provider completes a sign-in for an existing account. */
internal fun MainActivity.awaitsConsent(google: Boolean): Boolean =
    accountSettings.consent?.google == google

/** Completes [signInAgain] from the redirect: the exchange, connect and catch-up all block. */
internal fun MainActivity.completeAccountConsent(callbackUrl: String) {
    val instance = app ?: return
    val consent = accountSettings.consent ?: return
    accountSettings.consent = null
    thread(name = "mailcal-account-consent") {
        val answer = runCatching { instance.completeAccountConsent(consent.handle, callbackUrl) }
        mainHandler.post {
            accountSettings.signingIn = null
            refreshAccountsSnapshot()
            refreshConnectivity()
            answer.onSuccess {
                logUiInfo("accounts: an account was signed in again in place")
                say(signedInNotice(consent.accountId))
            }
            answer.onFailure {
                logUiWarn("accounts: signing an account in again failed: ${coreErrorText(it)}")
                say(
                    AccountNotice(
                        NoticeKind.FAILURE,
                        L10n.settings_account_signin_failed(this),
                        coreErrorText(it) ?: L10n.signin_expired_failed(this),
                    ),
                )
            }
        }
    }
}

/**
 * What a completed sign-in says: that it worked, or, when the provider still did not allow a use
 * the account is chosen for, which ones, since the sign-in alone does not mean the person got
 * what they asked for.
 */
internal fun MainActivity.signedInNotice(accountId: String): AccountNotice {
    val withheld = accountSettings.entry(accountId)?.let(AccountSettingsRules::withheld).orEmpty()
    return if (withheld.isNotEmpty()) {
        AccountNotice(
            NoticeKind.WARNING,
            L10n.setup_withheld_title(this),
            L10n.setup_withheld_detail(this, withheld.joinToString(", ") { useName(this, it) }),
        )
    } else {
        AccountNotice(NoticeKind.SUCCESS, L10n.settings_account_signed_in(this), "")
    }
}

/** A sign-in for an existing account whose browser was closed without a redirect. */
internal fun MainActivity.cancelAbandonedConsent() {
    if (accountSettings.signingIn != null && accountSettings.consent != null) {
        logUiInfo("accounts: signing an account in again was cancelled (browser dismissed)")
        accountSettings.signingIn = null
        accountSettings.consent = null
    }
}
