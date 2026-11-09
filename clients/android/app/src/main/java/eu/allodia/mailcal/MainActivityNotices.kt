// The core's notices (docs/notices.md): pulled after Surface.NOTICES and once at launch, drawn by
// each screen's card, each account's row and its page in Settings, and each action run through a
// flow this client already has, or by the core for a Retry. Which notice is drawn where is
// NoticeRules'; the views are NoticeViews.kt.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
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

    /** What the last change raised, for assistive technology; empty when it raised nothing. */
    var announcement by mutableStateOf("")

    /**
     * The notice whose Allodia sign-in this client runs, until it ends. An account's sign-in from a
     * notice is the core's, which draws that notice running itself.
     */
    var allodiaRemedy by mutableStateOf<String?>(null)

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
    notices.announcement = notices.announcements.arrived(pulled).joinToString("\n") { notice ->
        listOfNotNull(noticeWords(this, notice.title), notice.detail?.let { noticeWords(this, it) })
            .joinToString(" ")
    }
}

/** What the notice views run, and the notice whose Allodia sign-in is under way. */
internal fun MainActivity.noticeHandlers() = NoticeHandlers(
    running = notices.allodiaRemedy?.takeIf { signingInAllodia },
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
        // The core runs it, draws this notice running while it is out, and marks it failed when
        // it fails (MainActivityBrowserSignIn.kt).
        NoticeAction.SIGN_IN_AGAIN -> account?.let { signInInBrowser(it, emptyList()) }
        NoticeAction.SIGN_IN_TO_ALLODIA -> if (!signingInAllodia) {
            notices.allodiaRemedy = notice.id
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
 * The Allodia sign-in a notice started ended; [failed] when it ended refused (rule 10). A browser
 * the person closed is not a failure. One nobody started from a notice is not a notice's to report.
 */
internal fun MainActivity.allodiaRemedyEnded(failed: Boolean) {
    val noticeId = notices.allodiaRemedy ?: return
    notices.allodiaRemedy = null
    if (failed) {
        logUiInfo("notices: a notice's remedy did not complete")
        app?.dispatch(Intent.Notices(NoticeIntent.RemedyFailed(noticeId)))
    }
}
