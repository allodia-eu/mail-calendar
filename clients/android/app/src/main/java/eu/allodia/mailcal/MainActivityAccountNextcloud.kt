// Signing a calendar and contacts account in with Nextcloud, from its page or from the banner of a
// refused sign-in: again, for one a Nextcloud sign-in made, or instead of its typed password
// (docs/nextcloud-sign-in.md rules 2 and 7). It runs through the account sign-in's own state, so the
// page draws it as it draws a provider sign-in. The browser half is MainActivityNextcloud.kt's.
package eu.allodia.mailcal

import kotlin.concurrent.thread
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/**
 * Whether [accountId] signs in through a browser with Nextcloud: a calendar and contacts account,
 * whose page offers no other browser sign-in.
 */
internal fun MainActivity.signsInWithNextcloud(accountId: String): Boolean =
    accountSettings.entry(accountId)?.kind == CoreAccountKind.DAV

/**
 * Signs [accountId] in with Nextcloud; [switching] from a typed password. A grant kept from the
 * last try goes straight to the core.
 */
internal fun MainActivity.startAccountNextcloudSignIn(accountId: String, switching: Boolean) {
    val signIn = accountSettings.signIn
    if (!signIn.begin(accountId, switching)) return
    signIn.nextcloudFlight?.stop()
    signIn.nextcloudFlight = null
    signIn.nextcloudGrant?.takeIf { it.first == accountId }?.let { (_, grant) ->
        signIn.completingNextcloud(accountId, switching)
        return completeAccountNextcloud(accountId, grant, switching)
    }
    signIn.leaveNextcloud()
    logUiInfo("sign-in: signing a calendar and contacts account in with Nextcloud")
    var mine: NextcloudFlight? = null
    mine = flyNextcloud(
        "mailcal-account-nextcloud",
        begin = { it.beginAccountNextcloudSignIn(accountId) },
        opening = {
            val ours = signIn.nextcloudFlight === mine
            if (ours) signIn.polling()
            ours
        },
    ) { end ->
        val ours = mine != null && signIn.nextcloudFlight === mine
        if (ours) signIn.nextcloudFlight = null
        when {
            end is NextcloudEnd.Granted && ours && signIn.completingNextcloud(accountId, switching) ->
                completeAccountNextcloud(accountId, end.grant, switching)
            end is NextcloudEnd.Granted -> abandonNextcloud(end.grant)
            !ours -> Unit
            else -> {
                signIn.finished()
                // A wait that ran out is a browser left open, not a refusal (docs/notices.md rule 10).
                noticeRemedyEnded(NoticeRemedy.ACCOUNT_SIGN_IN, accountId, failed = end is NextcloudEnd.Failed)
                nextcloudSaid(end, this)?.let {
                    say(AccountNotice(NoticeKind.FAILURE, nextcloudFailedTitle(switching), it))
                }
            }
        }
    }
    if (mine == null) signIn.finished() else signIn.nextcloudFlight = mine
}

private fun MainActivity.nextcloudFailedTitle(switching: Boolean): String =
    if (switching) L10n.settings_account_switch_failed(this) else L10n.settings_account_signin_failed(this)

/** Puts [grant] on [accountId] off the main thread, keeping it until the core has. */
private fun MainActivity.completeAccountNextcloud(accountId: String, grant: String, switching: Boolean) {
    val instance = app ?: return
    val signIn = accountSettings.signIn
    signIn.nextcloudGrant = accountId to grant
    thread(name = "mailcal-account-nextcloud-complete") {
        val answer = runCatching { instance.completeAccountNextcloudSignIn(accountId, grant) }
        mainHandler.post {
            // Used, or withdrawn by the core as somebody else's: nothing to try again.
            if (answer.isSuccess || answer.exceptionOrNull() is MailcalException.DifferentAccount) {
                signIn.nextcloudGrant = null
            }
            signIn.finished()
            refreshAccountsSnapshot()
            pullNotices()
            noticeRemedyEnded(NoticeRemedy.ACCOUNT_SIGN_IN, accountId, failed = answer.isFailure)
            answer.onSuccess {
                logUiInfo("sign-in: a calendar and contacts account signed in with Nextcloud")
                val said = if (switching) {
                    L10n.settings_account_switched_to_provider(this)
                } else {
                    L10n.settings_account_signed_in(this)
                }
                say(AccountNotice(NoticeKind.SUCCESS, said, ""))
            }
            answer.onFailure {
                logUiWarn("sign-in: the Nextcloud grant did not go on the account: ${coreErrorText(it)}")
                val detail = if (switching) coreErrorSaid(it, this) ?: L10n.error_unknown(this) else signInAgainSaid(it, this)
                say(AccountNotice(NoticeKind.FAILURE, nextcloudFailedTitle(switching), detail))
            }
        }
    }
}
