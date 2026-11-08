// The core's three questions drawn as notices (docs/notices.md rule 8): each with both answers, the
// lesser one beside the first, and answered on the card rather than in a dialog of its own. The
// list stands in for the core, which takes a question away once it is answered.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
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
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeAction
import uniffi.mailcal_bindings.NoticeKind
import uniffi.mailcal_bindings.NoticeScreens
import uniffi.mailcal_bindings.NoticeSeverity
import uniffi.mailcal_bindings.NoticeText

@RunWith(RobolectricTestRunner::class)
class QuestionNoticesTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private fun question(
        id: String,
        kind: NoticeKind,
        title: NoticeText,
        screens: NoticeScreens,
        account: String?,
        action: NoticeAction,
        second: NoticeAction,
        labels: Pair<String, String>,
    ) = Notice(
        id = id,
        kind = kind,
        title = title,
        detail = null,
        severity = NoticeSeverity.INFORMATIONAL,
        screens = screens,
        account = account,
        mark = null,
        action = action,
        actionLabel = NoticeText(labels.first, emptyMap()),
        secondAction = second,
        secondActionLabel = NoticeText(labels.second, emptyMap()),
        dismissible = false,
        remedyFailed = false,
    )

    private val sentNotFiled = question(
        "copy",
        NoticeKind.SentNotFiled("Quarterly figures"),
        NoticeText("notice_sent_not_filed", mapOf("subject" to "Quarterly figures")),
        NoticeScreens.MAIL,
        "acct-a",
        NoticeAction.FILE_SENT_COPY,
        NoticeAction.LEAVE_SENT_COPY,
        "notice_action_file_sent_copy" to "notice_action_leave_sent_copy",
    )
    private val reply = question(
        "reply",
        NoticeKind.ReplyNotDelivered("Planning", "org@example.com", InvitationResponse.ACCEPT),
        NoticeText("notice_reply_not_delivered", mapOf("summary" to "Planning", "organizer" to "org@example.com")),
        NoticeScreens.EVERYWHERE,
        "acct-a",
        NoticeAction.SEND_REPLY,
        NoticeAction.DONT_SEND_REPLY,
        "notice_action_send_reply" to "notice_action_dont_send_reply",
    )
    private val timeZone = question(
        "zone",
        NoticeKind.TimeZoneChanged("Europe/Lisbon"),
        NoticeText("notice_time_zone_changed", mapOf("zone" to "Europe/Lisbon")),
        NoticeScreens.EVERYWHERE,
        null,
        NoticeAction.USE_DEVICE_TIME_ZONE,
        NoticeAction.KEEP_TIME_ZONE,
        "notice_action_use_time_zone" to "notice_action_keep_time_zone",
    )

    private fun words(text: NoticeText) = noticeWords(ctx, text)

    @Test
    fun `behind Issues, every question offers both of its answers`() {
        compose.setContent { NoticeArea(listOf(sentNotFiled, reply, timeZone), NoticeScreens.MAIL, NoticeHandlers()) }
        compose.waitForIdle()

        compose.onNodeWithText(L10n.notices_more(ctx, 2)).performClick()
        compose.waitForIdle()
        // The sheet opens half way, so the last of them can sit below its fold.
        for (question in listOf(sentNotFiled, reply, timeZone)) {
            val title = words(question.title)
            compose.onNodeWithContentDescription("${words(question.actionLabel!!)}: $title").assertExists()
            compose.onNodeWithContentDescription("${words(question.secondActionLabel!!)}: $title").assertExists()
        }
    }

    @Test
    fun `each question's lesser answer takes it away`() {
        val notices = mutableStateOf(listOf(sentNotFiled, reply, timeZone))
        val answered = mutableListOf<NoticeAction>()
        val handlers = NoticeHandlers(
            onRun = { notice, action ->
                answered.add(action)
                notices.value = notices.value - notice
            },
        )
        compose.setContent { NoticeArea(notices.value, NoticeScreens.MAIL, handlers) }
        compose.waitForIdle()

        // Each comes to the card in turn as the one before it is answered.
        for ((question, answer) in listOf(
            sentNotFiled to L10n.notice_action_leave_sent_copy(ctx),
            reply to L10n.notice_action_dont_send_reply(ctx),
            timeZone to L10n.notice_action_keep_time_zone(ctx),
        )) {
            compose.onNodeWithText(words(question.title)).assertIsDisplayed()
            compose.onNodeWithText(words(question.actionLabel!!)).assertIsDisplayed()
            compose.onNodeWithText(answer).performClick()
            compose.waitForIdle()
            compose.onAllNodesWithText(words(question.title)).assertCountEquals(0)
        }

        assertEquals(
            listOf(NoticeAction.LEAVE_SENT_COPY, NoticeAction.DONT_SEND_REPLY, NoticeAction.KEEP_TIME_ZONE),
            answered,
        )
    }

    @Test
    fun `an answer being carried out holds both buttons, with the spinner on the first`() {
        compose.setContent { NoticeArea(listOf(sentNotFiled.copy(running = true)), NoticeScreens.MAIL, NoticeHandlers()) }
        compose.waitForIdle()

        compose.onNodeWithTag("notice-action-copy").assertIsNotEnabled()
        compose.onNodeWithTag("notice-second-copy").assertIsNotEnabled()
        compose.onNodeWithTag("notice-action-copy-running", useUnmergedTree = true).assertIsDisplayed()
        compose.onAllNodesWithText(L10n.notice_action_file_sent_copy(ctx)).assertCountEquals(1)
    }
}
