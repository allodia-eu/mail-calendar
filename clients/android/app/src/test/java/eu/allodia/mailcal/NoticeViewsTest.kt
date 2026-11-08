// What the notice card, the Issues sheet, an account's mark and its page put on screen
// (docs/notices.md), through the catalog's own words.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeAction
import uniffi.mailcal_bindings.NoticeKind
import uniffi.mailcal_bindings.NoticeScreens
import uniffi.mailcal_bindings.NoticeSeverity
import uniffi.mailcal_bindings.NoticeText

@RunWith(RobolectricTestRunner::class)
class NoticeViewsTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()
    private val run = mutableListOf<String>()
    private val dismissed = mutableListOf<String>()

    private val ada = mapOf("address" to "ada@example.com")

    private val mailReauth = Notice(
        id = "mail",
        kind = NoticeKind.MailReauth,
        title = NoticeText("notice_mail_reauth", ada),
        detail = null,
        severity = NoticeSeverity.BLOCKED,
        screens = NoticeScreens.MAIL,
        account = "acct-a",
        mark = NoticeText("notice_mark_mail_reauth", emptyMap()),
        action = NoticeAction.SIGN_IN_AGAIN,
        actionLabel = NoticeText("notice_action_sign_in_again", emptyMap()),
        dismissible = false,
        remedyFailed = false,
    )
    private val calendarReauth = mailReauth.copy(
        id = "calendar",
        kind = NoticeKind.CalendarReauth,
        title = NoticeText("notice_calendar_reauth", ada),
        severity = NoticeSeverity.DEGRADED,
        screens = NoticeScreens.CALENDAR,
        mark = NoticeText("notice_mark_calendar_reauth", emptyMap()),
    )
    private val skipped = Notice(
        id = "skipped",
        kind = NoticeKind.AccountsSkipped(1u),
        title = NoticeText("notice_accounts_skipped", emptyMap()),
        detail = null,
        severity = NoticeSeverity.DEGRADED,
        screens = NoticeScreens.MAIL,
        account = null,
        mark = null,
        action = null,
        actionLabel = null,
        dismissible = true,
        remedyFailed = false,
    )

    private fun words(text: NoticeText) = noticeWords(ctx, text)

    private fun area(notices: List<Notice>, screen: NoticeScreens = NoticeScreens.MAIL, running: String? = null) {
        val handlers = NoticeHandlers(running, onRun = { notice, _ -> run.add(notice.id) }, onDismiss = { dismissed.add(it.id) })
        compose.setContent { NoticeArea(notices, screen, handlers) }
        compose.waitForIdle()
    }

    @Test
    fun `one card holds the screen's first notice and the rest stand behind Issues`() {
        area(listOf(mailReauth, calendarReauth))

        compose.onNodeWithText(words(mailReauth.title)).assertIsDisplayed()
        compose.onAllNodesWithText(words(calendarReauth.title)).assertCountEquals(0)

        compose.onNodeWithText(L10n.notices_more(ctx, 1)).performClick()
        compose.waitForIdle()

        compose.onNodeWithText(L10n.notices_issues(ctx)).assertIsDisplayed()
        compose.onNodeWithText(words(calendarReauth.title)).assertIsDisplayed()
        val label = L10n.notice_action_sign_in_again(ctx)
        compose.onNodeWithContentDescription("$label: ${words(calendarReauth.title)}").performClick()
        assertEquals(listOf("calendar"), run)
    }

    @Test
    fun `the calendar holds its own notice, with no Issues entry when it is the only other`() {
        area(listOf(mailReauth, calendarReauth), NoticeScreens.CALENDAR)

        compose.onNodeWithText(words(calendarReauth.title)).assertIsDisplayed()
        compose.onNodeWithText(L10n.notices_more(ctx, 1)).assertIsDisplayed()
    }

    @Test
    fun `a screen nothing belongs to draws no card`() {
        area(listOf(calendarReauth), NoticeScreens.CONTACTS)

        compose.onAllNodesWithText(words(calendarReauth.title)).assertCountEquals(0)
    }

    @Test
    fun `a standing notice has no Close and its action runs it`() {
        area(listOf(mailReauth))

        compose.onAllNodesWithContentDescription(L10n.notices_dismiss(ctx)).assertCountEquals(0)
        compose.onAllNodesWithText(L10n.notices_more(ctx, 1)).assertCountEquals(0)
        compose.onNodeWithText(L10n.notice_action_sign_in_again(ctx)).performClick()
        assertEquals(listOf("mail"), run)
    }

    @Test
    fun `an event can be closed`() {
        area(listOf(skipped))

        compose.onNodeWithContentDescription(L10n.notices_dismiss(ctx)).performClick()
        assertEquals(listOf("skipped"), dismissed)
    }

    @Test
    fun `a running remedy holds its button with a spinner`() {
        area(listOf(mailReauth), running = "mail")

        compose.onNodeWithTag("notice-action-mail").assertIsNotEnabled()
        compose.onNodeWithTag("notice-action-mail-running", useUnmergedTree = true).assertIsDisplayed()
    }

    @Test
    fun `a failed remedy says so under the title`() {
        area(listOf(mailReauth.copy(remedyFailed = true)))

        compose.onNodeWithText(L10n.notices_remedy_failed(ctx)).assertIsDisplayed()
    }

    @Test
    fun `an account's mark is one line, and a screen reader hears every sentence`() {
        val mark = NoticeRules.marks(listOf(mailReauth, calendarReauth), ::words) {
            L10n.notices_account_count(ctx, it)
        }.getValue("acct-a")
        compose.setContent { AccountMarkLine(mark) }

        val node = compose.onNodeWithContentDescription(mark.detail).fetchSemanticsNode()
        assertEquals(L10n.notices_account_count(ctx, 2), mark.label)
        assertEquals(listOf(mark.detail), node.config[SemanticsProperties.ContentDescription])
    }

    @Test
    fun `an account's page opens with its notices`() {
        compose.setContent { AccountNoticesGroup(NoticeRules.about(listOf(mailReauth, skipped), "acct-a"), NoticeHandlers()) }

        compose.onNodeWithText(words(mailReauth.title)).assertIsDisplayed()
        compose.onAllNodesWithText(words(skipped.title)).assertCountEquals(0)
    }
}
