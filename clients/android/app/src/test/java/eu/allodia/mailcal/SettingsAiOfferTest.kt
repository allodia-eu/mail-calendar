// Settings holds the writing assistant exactly while the core offers it (docs/ai.md rule 4): not
// offered, the hub has no Writing style and Advanced no endpoint form; withdrawn while Settings is
// open, an open Writing style is left for the hub with its flow, reveal or detail, the form leaves
// Advanced, and neither comes back open with the offer. Driven through SettingsScreen, the records
// moved as the activity moves them on a signal.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AboutInfo
import uniffi.mailcal_bindings.AiEndpointForm
import uniffi.mailcal_bindings.AiEndpointGaps
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.NewEventCalendars
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.SendAccountSetting
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.VideoconferencingPage
import uniffi.mailcal_bindings.ViewMode
import uniffi.mailcal_bindings.WeekStart
import uniffi.mailcal_bindings.WritingStyleLearning
import uniffi.mailcal_bindings.WritingStyleOpens
import uniffi.mailcal_bindings.WritingStylePage

@RunWith(RobolectricTestRunner::class)
class SettingsAiOfferTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    // The three records the activity reads on each signal; all null while the feature is not offered.
    private var page by mutableStateOf<WritingStylePage?>(stylePage())
    private var learning by mutableStateOf<WritingStyleLearning?>(null)
    private var form by mutableStateOf<AiEndpointForm?>(endpointForm())

    private fun withdraw() {
        page = null
        learning = null
        form = null
        compose.waitForIdle()
    }

    private fun show(category: SettingsCategory? = null) {
        compose.setContent {
            AppTheme {
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
                    sendAccount = SendAccountSetting.NoAccount,
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
                    advanced = ADVANCED_FREE,
                    onReset = {},
                    onBack = {},
                    initialCategory = category,
                    // Built where the records are read, as the activity builds them, so a change redraws.
                    writingStyle = testWritingStyleUi(
                        page,
                        learning,
                        reveal = { styleReveal(it) },
                        detail = { styleDetail(it) },
                    ),
                    aiEndpoint = AiEndpointUi(form, { _, _, _ -> AiEndpointGaps(null, false, null, null) }) {},
                )
            }
        }
        compose.waitForIdle()
    }

    private val writingStyleTitle get() = L10n.settings_category_writing_style(ctx())
    private val endpointTitle get() = L10n.ai_endpoint_title(ctx())

    private fun assertOnTheHub() {
        compose.onNodeWithText(L10n.settings_title(ctx())).assertIsDisplayed()
    }

    @Test
    fun not_offered_the_hub_has_no_writing_style() {
        withdraw()
        show()
        assertOnTheHub()
        compose.onNodeWithText(L10n.settings_category_signatures(ctx())).assertExists()
        compose.onNodeWithText(writingStyleTitle).assertDoesNotExist()
    }

    @Test
    fun not_offered_advanced_has_no_endpoint_form() {
        withdraw()
        show(SettingsCategory.ADVANCED)
        compose.onNodeWithText(L10n.settings_advanced_reset_description(ctx())).assertExists()
        compose.onNodeWithText(endpointTitle).assertDoesNotExist()
        compose.onNodeWithTag(AI_ENDPOINT_ADDRESS_TAG).assertDoesNotExist()
    }

    @Test
    fun not_offered_writing_style_asked_for_by_name_opens_the_hub() {
        withdraw()
        show(SettingsCategory.WRITING_STYLE)
        assertOnTheHub()
        compose.onNodeWithText(writingStyleTitle).assertDoesNotExist()
    }

    @Test
    fun an_open_writing_style_is_left_for_the_hub_and_stays_left() {
        show(SettingsCategory.WRITING_STYLE)
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).assertIsDisplayed()

        withdraw()
        assertOnTheHub()
        compose.onNodeWithText(writingStyleTitle).assertDoesNotExist()
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).assertDoesNotExist()

        page = stylePage()
        compose.waitForIdle()
        assertOnTheHub()
        compose.onNodeWithText(writingStyleTitle).assertExists()
        compose.onNodeWithTag(WRITING_STYLE_LEARN_TAG).assertDoesNotExist()
    }

    @Test
    fun an_open_learning_flow_closes_with_the_offer() {
        learning = learning(chooseAccount())
        show(SettingsCategory.WRITING_STYLE)
        compose.onNodeWithTag(LEARN_STAGE_TAG).assertIsDisplayed()

        withdraw()
        compose.onNodeWithTag(LEARN_STAGE_TAG).assertDoesNotExist()
        assertOnTheHub()
    }

    @Test
    fun an_open_reveal_closes_with_the_offer() {
        page = stylePage(styles = listOf(styleEntry("style-1", "My style", opens = WritingStyleOpens.REVEAL)))
        show(SettingsCategory.WRITING_STYLE)
        compose.onNodeWithTag(writingStyleRowTag("style-1")).performClick()
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()

        withdraw()
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertDoesNotExist()
        assertOnTheHub()
    }

    @Test
    fun an_open_detail_closes_with_the_offer() {
        page = stylePage(styles = listOf(styleEntry("style-1", "My style")))
        show(SettingsCategory.WRITING_STYLE)
        compose.onNodeWithTag(writingStyleRowTag("style-1")).performClick()
        compose.onNodeWithTag(DETAIL_NAME_TAG).assertExists()

        withdraw()
        compose.onNodeWithTag(DETAIL_NAME_TAG).assertDoesNotExist()
        assertOnTheHub()
    }

    @Test
    fun the_endpoint_form_leaves_advanced_with_the_offer() {
        show(SettingsCategory.ADVANCED)
        compose.onNodeWithTag(AI_ENDPOINT_ADDRESS_TAG).performScrollTo().assertIsDisplayed()

        withdraw()
        compose.onNodeWithTag(AI_ENDPOINT_ADDRESS_TAG).assertDoesNotExist()
        compose.onNodeWithText(endpointTitle).assertDoesNotExist()
        compose.onNodeWithText(L10n.settings_advanced_reset_description(ctx())).assertExists()
        compose.onNodeWithText(L10n.settings_category_advanced(ctx())).assertIsDisplayed()
    }
}
