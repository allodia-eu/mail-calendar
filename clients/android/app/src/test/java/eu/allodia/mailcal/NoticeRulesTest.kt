// Which notice the card on each screen holds, what an account's row says, and which flow an
// action runs (docs/notices.md). The words are the key itself here: the rules never read them.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.AccountProvider
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeAction
import uniffi.mailcal_bindings.NoticeIntent
import uniffi.mailcal_bindings.NoticeKind
import uniffi.mailcal_bindings.NoticeScreens
import uniffi.mailcal_bindings.NoticeSeverity
import uniffi.mailcal_bindings.NoticeText
import uniffi.mailcal_bindings.UseProblem

internal fun text(key: String, args: Map<String, String> = emptyMap()) = NoticeText(key, args)

internal fun notice(
    id: String,
    screens: NoticeScreens = NoticeScreens.MAIL,
    severity: NoticeSeverity = NoticeSeverity.DEGRADED,
    account: String? = null,
    kind: NoticeKind = NoticeKind.Unreachable,
    action: NoticeAction? = null,
    mark: String? = null,
    detail: String? = null,
    dismissible: Boolean = false,
    remedyFailed: Boolean = false,
) = Notice(
    id = id,
    kind = kind,
    title = text("title_$id"),
    detail = detail?.let { text(it) },
    severity = severity,
    screens = screens,
    account = account,
    mark = mark?.let { text(it) },
    action = action,
    actionLabel = action?.let { text("label_$id") },
    dismissible = dismissible,
    remedyFailed = remedyFailed,
)

class NoticeRulesTest {
    private val words: (NoticeText) -> String = { it.key }

    @Test
    fun `each screen shows its first notice and counts the rest behind Issues`() {
        val notices = listOf(
            notice("calendar", screens = NoticeScreens.CALENDAR),
            notice("offline", screens = NoticeScreens.EVERYWHERE),
            notice("mail"),
        )

        assertEquals("offline", NoticeRules.prominent(notices, NoticeScreens.MAIL)?.id)
        assertEquals("calendar", NoticeRules.prominent(notices, NoticeScreens.CALENDAR)?.id)
        assertEquals(2, NoticeRules.more(notices, NoticeScreens.MAIL))
    }

    @Test
    fun `a screen nothing belongs to has no card and no Issues entry`() {
        val notices = listOf(notice("calendar", screens = NoticeScreens.CALENDAR))

        assertNull(NoticeRules.prominent(notices, NoticeScreens.CONTACTS))
        assertEquals(0, NoticeRules.more(notices, NoticeScreens.CONTACTS))
    }

    @Test
    fun `each tab is its own screen`() {
        assertEquals(NoticeScreens.MAIL, NoticeRules.screenOf(AppDestination.MAIL))
        assertEquals(NoticeScreens.CALENDAR, NoticeRules.screenOf(AppDestination.CALENDAR))
        assertEquals(NoticeScreens.CONTACTS, NoticeRules.screenOf(AppDestination.CONTACTS))
    }

    @Test
    fun `an account row carries its first notice's mark, or how many when there are several`() {
        val notices = listOf(
            notice("a1", severity = NoticeSeverity.BLOCKED, account = "a", mark = "mark_a1"),
            notice("b1", account = "b", mark = "mark_b1"),
            notice("a2", account = "a", mark = "mark_a2"),
            notice("app"),
        )

        val marks = NoticeRules.marks(notices, words) { "$it issues" }

        assertEquals(setOf("a", "b"), marks.keys)
        assertEquals(AccountMark(NoticeSeverity.BLOCKED, "2 issues", "title_a1\ntitle_a2"), marks["a"])
        assertEquals(AccountMark(NoticeSeverity.DEGRADED, "mark_b1", "title_b1"), marks["b"])
    }

    @Test
    fun `a notice without a mark marks its account with its title`() {
        val marks = NoticeRules.marks(listOf(notice("a1", account = "a")), words) { "$it" }

        assertEquals("title_a1", marks["a"]?.label)
    }

    @Test
    fun `each severity has a glyph of its own shape`() {
        val glyphs = NoticeSeverity.entries.map(NoticeRules::glyph)

        assertEquals(glyphs.size, glyphs.toSet().size)
        assertEquals(R.drawable.ic_error, NoticeRules.glyph(NoticeSeverity.BLOCKED))
    }

    @Test
    fun `a failed remedy is said in place of the detail`() {
        val failed = notice("n", detail = "detail_n", remedyFailed = true)

        assertEquals("failed", NoticeRules.beneath(failed, words, "failed"))
        assertEquals("detail_n", NoticeRules.beneath(notice("n", detail = "detail_n"), words, "failed"))
        assertNull(NoticeRules.beneath(notice("n"), words, "failed"))
    }

    @Test
    fun `only a Retry goes to the core, and only a notice with a label offers its action`() {
        val retry = notice("r", action = NoticeAction.RETRY)

        assertEquals(NoticeIntent.Retry("r"), NoticeRules.coreRemedy(retry, NoticeAction.RETRY))
        assertNull(NoticeRules.coreRemedy(retry, NoticeAction.SIGN_IN_AGAIN))
        assertTrue(NoticeRules.offers(retry))
        assertEquals(false, NoticeRules.offers(notice("n")))
    }

    @Test
    fun `each answer to a question sends the intent it always sent`() {
        val reply = notice("reply", kind = NoticeKind.ReplyNotDelivered("Planning", "org@example.com", InvitationResponse.ACCEPT))
        val subject = { response: InvitationResponse, summary: String -> "$response: $summary" }

        assertEquals(Intent.RetryUnfiledCopy, NoticeRules.answer(reply, NoticeAction.FILE_SENT_COPY, subject))
        assertEquals(Intent.DismissUnfiledCopy, NoticeRules.answer(reply, NoticeAction.LEAVE_SENT_COPY, subject))
        assertEquals(
            Intent.AnswerReplyPrompt(send = true, remember = false, replySubject = "ACCEPT: Planning"),
            NoticeRules.answer(reply, NoticeAction.SEND_REPLY, subject),
        )
        // Not sending composes no subject, and neither answer is remembered: that is the account's
        // page's to choose.
        assertEquals(
            Intent.AnswerReplyPrompt(send = false, remember = false, replySubject = null),
            NoticeRules.answer(reply, NoticeAction.DONT_SEND_REPLY, subject),
        )
        assertEquals(Intent.AcceptTimeZoneChange, NoticeRules.answer(reply, NoticeAction.USE_DEVICE_TIME_ZONE, subject))
        assertEquals(Intent.DismissTimeZoneChange, NoticeRules.answer(reply, NoticeAction.KEEP_TIME_ZONE, subject))
        assertNull(NoticeRules.answer(reply, NoticeAction.SIGN_IN_AGAIN, subject))
        assertNull(NoticeRules.answer(notice("other"), NoticeAction.SEND_REPLY, subject))
    }

    @Test
    fun `a question offers its other answer, and both are held while one runs`() {
        val question = notice("q", action = NoticeAction.FILE_SENT_COPY).copy(
            secondAction = NoticeAction.LEAVE_SENT_COPY,
            secondActionLabel = text("second_q"),
        )

        assertTrue(NoticeRules.offersSecond(question))
        assertFalse(NoticeRules.offersSecond(notice("n", action = NoticeAction.RETRY)))
        assertFalse(NoticeRules.held(question, null))
        assertTrue(NoticeRules.held(question.copy(running = true), null))
        assertTrue(NoticeRules.held(question, "q"))
        assertFalse(NoticeRules.held(question, "other"))
    }

    @Test
    fun `sign in again runs the flow the account signs in by`() {
        assertEquals(ExpiredRemedy.PROVIDER_CONSENT, NoticeRules.signInFlow(AccountProvider.MICROSOFT, false))
        assertEquals(ExpiredRemedy.PROVIDER_CONSENT, NoticeRules.signInFlow(AccountProvider.GOOGLE, false))
        assertEquals(ExpiredRemedy.JMAP_REAUTH, NoticeRules.signInFlow(AccountProvider.JMAP_OAUTH, false))
        assertEquals(ExpiredRemedy.ACCOUNT_SIGN_IN, NoticeRules.signInFlow(AccountProvider.IMAP_OAUTH, false))
        assertEquals(ExpiredRemedy.ACCOUNT_SIGN_IN, NoticeRules.signInFlow(AccountProvider.PASSWORD, true))
        assertEquals(ExpiredRemedy.SETTINGS, NoticeRules.signInFlow(AccountProvider.PASSWORD, false))
        assertEquals(ExpiredRemedy.SETTINGS, NoticeRules.signInFlow(null, false))
    }

    @Test
    fun `an account's page holds its own notices, in order`() {
        val notices = listOf(
            notice("a1", account = "a"),
            notice("b1", account = "b", kind = NoticeKind.CalendarOutage(UseProblem.UNREACHABLE)),
            notice("a2", account = "a"),
        )

        assertEquals(listOf("a1", "a2"), NoticeRules.about(notices, "a").map { it.id })
    }

    @Test
    fun `an expired sign-in is the account's own notice`() {
        val notices = listOf(
            notice("a-expired", account = "a", kind = NoticeKind.SignInExpired),
            notice("b-mail", account = "b", kind = NoticeKind.MailReauth),
        )

        // A narrower grant is not a dead sign-in.
        assertEquals(setOf("a"), NoticeRules.expired(notices))
    }

    @Test
    fun `the Allodia card follows its notice`() {
        val offline = notice("offline", screens = NoticeScreens.EVERYWHERE, kind = NoticeKind.Offline)
        val ended = notice("allodia", screens = NoticeScreens.EVERYWHERE, kind = NoticeKind.AllodiaSignInEnded("anna@example.org"))

        assertTrue(NoticeRules.allodiaSignInEnded(listOf(offline, ended)))
        assertFalse(NoticeRules.allodiaSignInEnded(listOf(offline)))
    }

    @Test
    fun `a notice is announced when it first appears, and again only after it went`() {
        val announcements = NoticeAnnouncements()
        val one = notice("one")
        val two = notice("two")

        assertEquals(listOf(one), announcements.arrived(listOf(one)))
        assertEquals(listOf(two), announcements.arrived(listOf(one, two)))
        assertEquals(emptyList<Notice>(), announcements.arrived(listOf(one, two)))
        announcements.arrived(listOf(two))
        assertEquals(listOf(one), announcements.arrived(listOf(one, two)))
    }
}
