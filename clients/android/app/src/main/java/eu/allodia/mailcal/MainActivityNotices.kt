// The core's notices (docs/notices.md): pulled after Surface.NOTICES and once at launch, drawn by
// each screen's card, each account's row and its page in Settings, and each action run through a
// flow this client already has, or by the core for a Retry. Which notice is drawn where is
// NoticeRules'; the views are NoticeViews.kt.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeAction
import uniffi.mailcal_bindings.NoticeIntent

/** The notices in force and what the activity derived from them. */
internal class NoticesState {
    /** Every notice in force, in the core's order. */
    var notices by mutableStateOf<List<Notice>>(emptyList())

    /** Each account's mark, by account id. */
    var marks by mutableStateOf<Map<String, AccountMark>>(emptyMap())

    /** The accounts a pasted JMAP password or token was refused for, which a new one replaces. */
    var pastedSecret by mutableStateOf<Set<String>>(emptySet())

    /** What the last change raised, for assistive technology; empty when it raised nothing. */
    var announcement by mutableStateOf("")

    /** The remedy a notice started, until it ends. */
    var remedy by mutableStateOf<RunningRemedy?>(null)

    val announcements = NoticeAnnouncements()

    /** The accounts whose sign-in the server stopped accepting. */
    val expired: Set<String> get() = NoticeRules.expired(notices)

    /** Whether the Allodia account's sign-in was refused. */
    val allodiaSignInEnded: Boolean get() = NoticeRules.allodiaSignInEnded(notices)
}

/** Pulls the notices in force; a pull that changed nothing redraws nothing. */
internal fun MainActivity.pullNotices() {
    val instance = app ?: return
    val pulled = instance.notices().notices
    if (pulled == notices.notices) return
    notices.notices = pulled
    logUiInfo("notices: ${pulled.size} in force")
    notices.marks = NoticeRules.marks(pulled, { noticeWords(this, it) }) { L10n.notices_account_count(this, it) }
    notices.pastedSecret = notices.expired.filter { instance.accountProvider(it) == AccountProvider.JMAP }.toSet()
    notices.announcement = notices.announcements.arrived(pulled).joinToString("\n") { notice ->
        listOfNotNull(noticeWords(this, notice.title), notice.detail?.let { noticeWords(this, it) })
            .joinToString(" ")
    }
}

/** What the notice views run, and the notice whose remedy is under way. */
internal fun MainActivity.noticeHandlers() = NoticeHandlers(
    running = noticeRunning(),
    onRun = { notice, action -> runNotice(notice, action) },
    onDismiss = { dismissNotice(it) },
)

/** Closes a notice the core offered to close. */
internal fun MainActivity.dismissNotice(notice: Notice) {
    if (notice.dismissible) app?.dispatch(Intent.Notices(NoticeIntent.Dismiss(notice.id)))
}

/**
 * Runs [action], one of [notice]'s answers: a Retry through the core, which judges it, a question's
 * answer as the intent it always sent (rule 8), and every other through the flow this client
 * already has (rule 7).
 */
internal fun MainActivity.runNotice(notice: Notice, action: NoticeAction) {
    val instance = app ?: return
    NoticeRules.coreRemedy(notice, action)?.let {
        instance.dispatch(Intent.Notices(it))
        return
    }
    NoticeRules.answer(notice, action) { response, summary -> invitationReplySubject(this, response, summary) }?.let {
        instance.dispatch(it)
        return
    }
    val account = notice.account
    when (action) {
        NoticeAction.SIGN_IN_AGAIN -> account?.let { signInAgainFromNotice(notice, it) }
        NoticeAction.SIGN_IN_TO_ALLODIA -> if (!signingInAllodia) {
            notices.remedy = RunningRemedy(NoticeRemedy.ALLODIA, notice.id, null)
            signInToAllodia()
        }
        NoticeAction.OPEN_ACCOUNT_SETTINGS -> if (account != null) {
            openAccountPage(account)
        } else {
            settingsCategory = SettingsCategory.ACCOUNTS
            showingSettings = true
        }
        NoticeAction.REVIEW -> account?.let { reviewServerChange(it) }
        // A Retry went to the core and an answer was sent as its intent, both above.
        NoticeAction.RETRY,
        NoticeAction.FILE_SENT_COPY,
        NoticeAction.LEAVE_SENT_COPY,
        NoticeAction.SEND_REPLY,
        NoticeAction.DONT_SEND_REPLY,
        NoticeAction.USE_DEVICE_TIME_ZONE,
        NoticeAction.KEEP_TIME_ZONE,
        -> Unit
    }
}

/**
 * The account's sign-in again, in place: it keeps its id, mail and settings. A browser sign-in is
 * one at a time, so a press while one runs is held, as its button is.
 */
private fun MainActivity.signInAgainFromNotice(notice: Notice, account: String) {
    val nextcloud = accountSettings.entry(account)?.nextcloud == true
    fun start(remedy: NoticeRemedy) {
        notices.remedy = RunningRemedy(remedy, notice.id, account)
    }
    when (NoticeRules.signInFlow(app?.accountProvider(account), nextcloud)) {
        ExpiredRemedy.PROVIDER_CONSENT -> if (accountSettings.signingIn == null) {
            start(NoticeRemedy.CONSENT)
            signInAgain(account, emptyList())
        }
        // Addressed to the account id: the core re-authorises this account's own stored grant.
        ExpiredRemedy.JMAP_REAUTH -> if (!signingInJmap) {
            start(NoticeRemedy.JMAP)
            reconnectJmap(accountId = account)
        }
        ExpiredRemedy.ACCOUNT_SIGN_IN -> if (accountSettings.signIn.running == null) {
            start(NoticeRemedy.ACCOUNT_SIGN_IN)
            startAccountSignIn(account, switching = false)
        }
        // No browser flow to run: the password or token is entered again on the account's page.
        ExpiredRemedy.SETTINGS -> openAccountPage(account)
    }
}

/**
 * A flow a notice started ended; [failed] when it ended refused (rule 10). A browser the person
 * closed, or a wait that ran out, is not a failure. A flow nobody started from a notice, or one
 * for another account, is not this notice's to report.
 */
internal fun MainActivity.noticeRemedyEnded(remedy: NoticeRemedy, account: String?, failed: Boolean) {
    val running = notices.remedy ?: return
    if (running.remedy != remedy || running.account != account) return
    notices.remedy = null
    if (failed) {
        logUiInfo("notices: a notice's remedy did not complete")
        app?.dispatch(Intent.Notices(NoticeIntent.RemedyFailed(running.noticeId)))
    }
}

/** The notice whose remedy runs now, read from the flow's own state so a closed browser stops it. */
internal fun MainActivity.noticeRunning(): String? {
    val running = notices.remedy ?: return null
    val busy = when (running.remedy) {
        NoticeRemedy.CONSENT -> accountSettings.signingIn == running.account
        NoticeRemedy.JMAP -> signingInJmap || completingJmap
        NoticeRemedy.ACCOUNT_SIGN_IN -> accountSettings.signIn.running?.accountId == running.account
        NoticeRemedy.ALLODIA -> signingInAllodia
    }
    return running.noticeId.takeIf { busy }
}
