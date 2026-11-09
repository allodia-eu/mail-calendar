// Settings → Accounts' traffic with the core: the one snapshot every account is drawn from, and the
// changes an account's page makes (docs/accounts.md rules 11 to 14). A use, a link and a removal
// are the core's to run (docs/waiting.md): dispatched, they answer at once and the page says they
// run. Every other change may touch the Keystore or dial a server, so it runs off the main thread
// and answers with what the page should say.
package eu.allodia.mailcal

import android.widget.Toast
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountChangeNoticeRaised
import uniffi.mailcal_bindings.AccountEndpoints
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.LinkSlot
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

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
    refreshAccountPage()
    // A suggestion the core found after the link step was drawn reaches it.
    setupFlow.links?.let { setupFlow.refresh(linkStepFor(it.account)) }
}

/** Opens Settings on account [id]'s page: where a password is replaced and a use asked for. */
internal fun MainActivity.openAccountPage(id: String) {
    accountSettings.notice = null
    accountSettings.page = id
    refreshAccountPage()
    settingsCategory = SettingsCategory.ACCOUNTS
    showingSettings = true
}

/**
 * Reads the open account's page again, and says what a change came to, once: on its page, or for a
 * removal, wherever the person is.
 */
internal fun MainActivity.refreshAccountPage() {
    val instance = app ?: return
    val page = accountSettings.page?.let { instance.accountPage(it) }
    accountSettings.accountPage = page
    // The core withdraws what a change came to when the page's next change starts: a save that
    // worked must not stand under the failure before it.
    if (page != null && page.notice == null && accountSettings.notice === accountSettings.changeNotice) {
        accountSettings.notice = null
    }
    page?.notice?.let(::sayAccountNotice)
    instance.accountsNotice()?.let(::sayAccountNotice)
}

private fun MainActivity.sayAccountNotice(raised: AccountChangeNoticeRaised) {
    if (raised.id <= accountSettings.coreNotice) return
    accountSettings.coreNotice = raised.id
    logUiWarn("accounts: a change on an account's page did not take")
    val said = changeNotice(this, raised.notice)
    say(said)
    accountSettings.changeNotice = said
}

/** Asks the core for a change on an account's page; it answers at once, and the page says it runs. */
internal fun MainActivity.changeAccount(intent: AccountPageIntent) {
    app?.dispatch(Intent.AccountPage(intent))
}

/** Makes the links setup's link step chose, one after another. */
internal fun MainActivity.linkFromSetup(accountId: String, links: List<Pair<LinkSlot, String>>) {
    val instance = app ?: return
    thread(name = "mailcal-setup-links") {
        // Stored through the Keystore, which may block.
        links.forEach { (slot, target) ->
            runCatching { instance.setAccountLink(accountId, slot, target) }
                .onFailure { logUiWarn("setup: linking an account failed: ${coreErrorText(it)}") }
        }
        mainHandler.post { refreshAccountsSnapshot() }
    }
}

/**
 * Says [notice] where the person is: on the account's page while Settings is open, and in passing
 * otherwise, since a sign-in started from a banner ends on the mailbox and a notice kept for
 * Settings would greet them there later about something long over.
 */
internal fun MainActivity.say(notice: AccountNotice) {
    if (showingSettings) {
        accountSettings.notice = notice
    } else {
        val text = listOf(notice.title, notice.message).filter { it.isNotEmpty() }.joinToString(": ")
        Toast.makeText(this, text, Toast.LENGTH_LONG).show()
    }
}

internal fun MainActivity.changeFailed(detail: String?) = AccountNotice(
    NoticeKind.FAILURE,
    L10n.settings_account_change_failed_title(this),
    detail ?: L10n.error_unknown(this),
)

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
        noticeRemedyEnded(NoticeRemedy.CONSENT, accountId, failed = true)
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
                coreErrorSaid(e, this) ?: L10n.account_signin_browser_failed(this),
            ),
        )
        noticeRemedyEnded(NoticeRemedy.CONSENT, accountId, failed = true)
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
            pullNotices()
            noticeRemedyEnded(NoticeRemedy.CONSENT, consent.accountId, failed = answer.isFailure)
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
                        signInAgainSaid(it, this),
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
    val account = accountSettings.signingIn ?: return
    if (accountSettings.consent != null) {
        logUiInfo("accounts: signing an account in again was cancelled (browser dismissed)")
        accountSettings.signingIn = null
        accountSettings.consent = null
        noticeRemedyEnded(NoticeRemedy.CONSENT, account, failed = false)
    }
}
