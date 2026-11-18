// The notice at the top of Settings (docs/notices.md rule 2): the first that stands everywhere,
// drawn above whichever category is open through the screens' own card, and on an account's page
// without the notices that page lists.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.AboutInfo
import uniffi.mailcal_bindings.AccountsSnapshot
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.NewEventCalendars
import uniffi.mailcal_bindings.Notice
import uniffi.mailcal_bindings.NoticeAction
import uniffi.mailcal_bindings.NoticeKind
import uniffi.mailcal_bindings.NoticeScreens
import uniffi.mailcal_bindings.NoticeSeverity
import uniffi.mailcal_bindings.NoticeText
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.VideoconferencingPage
import uniffi.mailcal_bindings.ViewMode
import uniffi.mailcal_bindings.WeekStart

@RunWith(RobolectricTestRunner::class)
class SettingsNoticeTest {
    @get:Rule val compose = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()
    private val run = mutableListOf<Pair<String, NoticeAction>>()
    private val account = mailboxOnly()

    private val notSaved = Notice(
        id = "settings",
        kind = NoticeKind.SettingsNotSaved,
        title = NoticeText("notice_settings_not_saved", emptyMap()),
        detail = NoticeText("notice_detail_settings_not_saved", emptyMap()),
        severity = NoticeSeverity.DEGRADED,
        screens = NoticeScreens.EVERYWHERE,
        account = null,
        mark = null,
        action = NoticeAction.RETRY,
        actionLabel = NoticeText("notice_action_retry", emptyMap()),
        dismissible = false,
        remedyFailed = false,
    )
    private val signInExpired = notSaved.copy(
        id = "signin",
        kind = NoticeKind.SignInExpired,
        title = NoticeText("notice_signin_expired", mapOf("address" to account.address)),
        detail = null,
        severity = NoticeSeverity.BLOCKED,
        account = account.id,
        mark = NoticeText("notice_mark_signin_expired", emptyMap()),
        action = NoticeAction.OPEN_ACCOUNT_SETTINGS,
        actionLabel = NoticeText("notice_action_open_account_settings", emptyMap()),
    )
    private val offline = notSaved.copy(
        id = "offline",
        kind = NoticeKind.Offline,
        title = NoticeText("notice_offline", emptyMap()),
        detail = null,
        screens = NoticeScreens.MAIL,
        action = null,
        actionLabel = null,
    )

    private var notices by mutableStateOf<List<Notice>>(emptyList())

    private fun words(text: NoticeText) = noticeWords(ctx, text)

    /** Settings opened at [category], with [state]'s accounts, holding the notices in force. */
    private fun show(category: SettingsCategory? = null, state: AccountsUiState = AccountsUiState()) {
        val handlers = NoticeHandlers(onRun = { notice, action -> run += notice.id to action })
        compose.setContent {
            SettingsScreen(
                about = AboutInfo(version = "9.9.9", supportUrl = "https://support.allodia.eu", attributions = emptyList()),
                timeZone = null,
                onSetTimeZone = {},
                display = DisplaySettings(
                    WeekStart.MONDAY,
                    TimeFormat.TWENTY_FOUR_HOUR,
                    Appearance.SYSTEM,
                    true,
                    12u,
                    byteArrayOf(6, 8, 12, 16, 24),
                    CalendarLayout.WEEK,
                ),
                onSetTimeFormat = {},
                onSetAppearance = {},
                onSetWeekStart = {},
                onSetVisibleHours = {},
                newEventCalendars = NewEventCalendars(emptyList(), named = false),
                onSetDefaultCalendar = { _, _ -> },
                videoconferencing = VideoconferencingPage(null, null, emptyList()),
                onSetJitsiServer = { VideoconferencingPage(null, null, emptyList()) },
                onSetDefaultMeeting = { _, _ -> },
                mode = ViewMode.THREADED,
                onSetMode = {},
                onSetDarkMessages = {},
                quoteSettings = QuoteSettings(QuoteStyleKind.INDENTED, perMessage = false),
                onSetQuoteStyle = {},
                onSetQuoteStylePerMessage = {},
                accounts = emptyList(),
                defaultSendAccount = null,
                onSetDefaultSendAccount = {},
                swipe = SwipeSettings(SwipeActionKind.ARCHIVE, SwipeActionKind.DELETE),
                onSetSwipeLeft = {},
                onSetSwipeRight = {},
                signatures = testSignaturesUi(),
                analyticsEnabled = false,
                onSetAnalytics = {},
                analyticsPayloadPreview = { "{}" },
                allodia = AllodiaSettings(),
                onAllodia = {},
                onAllodiaManage = {},
                allodiaSubscription = AllodiaSubscriptionUi(),
                allodiaDevices = null,
                onAllodiaSetUp = {},
                onOpenDiagnostics = {},
                onReset = {},
                onBack = {},
                initialCategory = category,
                // Built where the notices are read, as the activity builds it, so a change redraws.
                accountsUi = AccountsSettingsUi(state, notices = notices, noticeHandlers = handlers),
            )
        }
        compose.waitForIdle()
    }

    @Test
    fun `the hub opens with the first notice that stands everywhere, and Try again runs it`() {
        notices = listOf(offline, notSaved)
        show()

        compose.onNodeWithText(words(notSaved.title)).assertIsDisplayed()
        compose.onAllNodesWithText(words(offline.title)).assertCountEquals(0)
        compose.onNodeWithText(L10n.notice_action_retry(ctx)).performClick()
        assertEquals(listOf("settings" to NoticeAction.RETRY), run)
    }

    @Test
    fun `an open category is under one notice, never a stack`() {
        notices = listOf(notSaved, signInExpired)
        show(SettingsCategory.READING)

        compose.onAllNodesWithTag("notice-card").assertCountEquals(1)
        compose.onNodeWithText(words(notSaved.title)).assertIsDisplayed()
        compose.onAllNodesWithText(words(signInExpired.title)).assertCountEquals(0)
        compose.onAllNodesWithText(L10n.notices_more(ctx, 1)).assertCountEquals(0)
    }

    @Test
    fun `the notice comes and goes with the core's`() {
        show(SettingsCategory.GENERAL)
        compose.onAllNodesWithTag("notice-card").assertCountEquals(0)

        notices = listOf(notSaved)
        compose.waitForIdle()
        compose.onNodeWithText(words(notSaved.title)).assertIsDisplayed()

        notices = emptyList()
        compose.waitForIdle()
        compose.onAllNodesWithTag("notice-card").assertCountEquals(0)
    }

    @Test
    fun `an account's page leaves its own notice to the page and draws the next`() {
        val state = AccountsUiState().apply {
            snapshot = AccountsSnapshot(listOf(account))
            page = account.id
            accountPage = pageOf(account)
        }
        notices = listOf(signInExpired, notSaved)
        show(SettingsCategory.ACCOUNTS, state)

        compose.onAllNodesWithTag("notice-card").assertCountEquals(1)
        compose.onNode(hasText(words(notSaved.title)) and hasAnyAncestor(hasTestTag("notice-card"))).assertIsDisplayed()
        compose.onNode(hasText(words(signInExpired.title)) and hasAnyAncestor(hasTestTag("account-notices")))
            .assertIsDisplayed()
    }
}
