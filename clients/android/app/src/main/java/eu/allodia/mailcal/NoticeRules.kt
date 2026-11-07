// Which of the core's notices this client draws where (docs/notices.md): the one prominent on the
// screen on show, how many more stand behind it, the mark each account's row carries, and which
// flow a notice's action runs. The core has already ranked them, so every rule here keeps its
// order and decides nothing about rank. Compose-free and catalog-free: the words arrive as a
// parameter, so the JVM suite reaches every rule.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeAction
import uniffi.mailcal_bindings.NoticeIntent
import uniffi.mailcal_bindings.NoticeScreens
import uniffi.mailcal_bindings.NoticeSeverity
import uniffi.mailcal_bindings.NoticeText

/**
 * What an account's row says about its notices (rule 6): its first notice's [severity], one short
 * [label] (that notice's mark, or how many there are) and every sentence, for a screen reader.
 */
internal data class AccountMark(val severity: NoticeSeverity, val label: String, val detail: String)

/** A flow a notice's action runs that ends later, and can end refused (rule 10). */
internal enum class NoticeRemedy {
    /** Microsoft or Google, signed in again through their own consent. */
    CONSENT,

    /** A JMAP account's own grant authorised again. */
    JMAP,

    /** An IMAP or JMAP account's sign-in, or a Nextcloud one. */
    ACCOUNT_SIGN_IN,

    /** The Allodia account's sign-in. */
    ALLODIA,
}

/** A remedy running for notice [noticeId], about [account] (null for the Allodia account). */
internal data class RunningRemedy(val remedy: NoticeRemedy, val noticeId: String, val account: String?)

internal object NoticeRules {
    /** The screens a notice can name, for the destination on show. */
    fun screenOf(destination: AppDestination): NoticeScreens = when (destination) {
        AppDestination.MAIL -> NoticeScreens.MAIL
        AppDestination.CALENDAR -> NoticeScreens.CALENDAR
        AppDestination.CONTACTS -> NoticeScreens.CONTACTS
    }

    /** Whether [notice] is drawn on [screen]. */
    fun belongs(notice: Notice, screen: NoticeScreens): Boolean =
        notice.screens == NoticeScreens.EVERYWHERE || notice.screens == screen

    /** The notice the card on [screen] holds: the first, in the core's order, that belongs there. */
    fun prominent(notices: List<Notice>, screen: NoticeScreens): Notice? =
        notices.firstOrNull { belongs(it, screen) }

    /**
     * How many notices besides the card's are in force, which its Issues entry counts. None while
     * the screen has no card: an account's notices are on its row then.
     */
    fun more(notices: List<Notice>, screen: NoticeScreens): Int =
        if (prominent(notices, screen) == null) 0 else notices.size - 1

    /** The line under a notice's title: the failed remedy's sentence ([failed]), else its detail. */
    fun beneath(notice: Notice, words: (NoticeText) -> String, failed: String): String? =
        if (notice.remedyFailed) failed else notice.detail?.let(words)

    /** The notices about [account], in the core's order, which its page in Settings opens with. */
    fun about(notices: List<Notice>, account: String): List<Notice> = notices.filter { it.account == account }

    /** The mark each account with a notice carries; [count] words "N issues". */
    fun marks(
        notices: List<Notice>,
        words: (NoticeText) -> String,
        count: (Int) -> String,
    ): Map<String, AccountMark> = notices
        .filter { it.account != null }
        .groupBy { it.account!! }
        .mapValues { (_, all) ->
            val first = all.first()
            AccountMark(
                severity = first.severity,
                label = if (all.size > 1) count(all.size) else words(first.mark ?: first.title),
                detail = all.joinToString("\n") { words(it.title) },
            )
        }

    /** The drawable a severity is drawn with: a shape of its own, so colour is never the only sign. */
    fun glyph(severity: NoticeSeverity): Int = when (severity) {
        NoticeSeverity.BLOCKED -> R.drawable.ic_error
        NoticeSeverity.DEGRADED -> R.drawable.ic_warning
        NoticeSeverity.INFORMATIONAL -> R.drawable.ic_info
    }

    /** Whether a notice has something to run: the core sends an action and its label together. */
    fun offers(notice: Notice): Boolean = notice.action != null && notice.actionLabel != null

    /**
     * What the core runs for a notice's action, or null for a flow this client runs itself. A
     * Retry is the core's: only it can tell when the dial it starts has ended, so the client never
     * reports one as failed.
     */
    fun coreRemedy(notice: Notice): NoticeIntent? =
        if (notice.action == NoticeAction.RETRY) NoticeIntent.Retry(notice.id) else null

    /**
     * The flow Sign in again runs for an account of [provider]; [nextcloud] for one a Nextcloud
     * sign-in made. The same answer a dead sign-in takes: a narrowed Microsoft or Google grant is
     * widened through that provider's consent, which is what the dead sign-in runs too.
     */
    fun signInFlow(provider: AccountProvider?, nextcloud: Boolean): ExpiredRemedy =
        AccountSignInRules.expiredRemedy(provider, nextcloud)
}

/**
 * Which notices are new since the last snapshot, so each is announced to assistive technology
 * once, when its id first appears (rule 12), and never again on a redraw.
 */
internal class NoticeAnnouncements {
    private var inForce = emptySet<String>()

    /** The notices in [notices] that were not in force last time; one that went and came back is new. */
    fun arrived(notices: List<Notice>): List<Notice> {
        val arrived = notices.filter { it.id !in inForce }
        inForce = notices.map { it.id }.toSet()
        return arrived
    }
}
