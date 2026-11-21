// The learning flow in the flow host (docs/ai.md rules 11 to 14) as the core's record draws it:
// each stage, exactly the controls the record gives, every press an intent, and system back as Back
// where the stage has one and as Close where it has not. Driven through the real composables.
package eu.allodia.mailcal

import androidx.activity.ComponentDialog
import androidx.activity.OnBackPressedDispatcher
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasProgressBarRangeInfo
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.text.TextLayoutResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode
import org.robolectric.shadows.ShadowDialog
import uniffi.mailcal_bindings.AiFailure
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.StyleLearnRange
import uniffi.mailcal_bindings.StyleLearningDestination
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.StyleLearningLanguage
import uniffi.mailcal_bindings.StyleLearningReport
import uniffi.mailcal_bindings.StyleLearningReview
import uniffi.mailcal_bindings.StyleLearningStage
import uniffi.mailcal_bindings.StyleRangeFetch
import uniffi.mailcal_bindings.StyleSentReach
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleLearning

private val REPORT = StyleLearningReport(
    found = 120u,
    usable = 80u,
    languages = listOf(StyleLearningLanguage("en", 60u), StyleLearningLanguage("nl", 20u)),
    otherLanguages = 3u,
    oldest = 1_700_000_000L,
    newest = 1_760_000_000L,
)

private fun review(
    state: StyleLearningReview = StyleLearningReview.Ready(REPORT),
    sendsTo: StyleLearningDestination? = StyleLearningDestination.Relay,
    cannotSend: AiFailure? = null,
    keywordKept: Boolean = true,
    olderNotFetched: Boolean = false,
) = StyleLearningStage.Review(
    email = "alice@test.local",
    range = StyleLearnRange.Everything,
    reach = StyleSentReach.Since("2025-03-01"),
    olderNotFetched = olderNotFetched,
    state = state,
    sendsTo = sendsTo,
    cannotSend = cannotSend,
    keywordKept = keywordKept,
)

@RunWith(RobolectricTestRunner::class)
class WritingStyleLearningTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<WritingStyleIntent>()
    private val signIns = mutableListOf<Unit>()
    private var endpointsOpened = 0
    private var record by mutableStateOf<WritingStyleLearning?>(null)

    private fun show(learning: WritingStyleLearning) {
        record = learning
        compose.setContent {
            AppTheme {
                val ui = testWritingStyleUi(stylePage(), record, sent, signIns = signIns)
                WritingStyleFlow(ui, revealing = null, onRevealLeft = {}, onOpenEndpoint = { endpointsOpened++ })
            }
        }
        compose.waitForIdle()
    }

    private fun intents() = learningSent(sent)

    @Test
    fun choose_account_lists_each_address_and_draws_only_the_controls_given() {
        show(learning(chooseAccount(), next = HELD))
        compose.onNodeWithText(L10n.learn_account_title(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(FLOW_BACK_TAG).assertDoesNotExist()
        compose.onNodeWithTag(FLOW_INDICATOR_TAG).assertDoesNotExist()

        compose.onNodeWithTag(learnAccountTag("bob")).performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.SelectAccount("bob")), intents())
    }

    @Test
    fun next_sends_next_once_the_record_frees_it() {
        show(learning(chooseAccount(selected = "alice"), next = FREE))
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsEnabled().performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Next), intents())
    }

    @Test
    fun the_last_twelve_months_come_first_and_start_selected() {
        show(learning(chooseRange(), back = FREE, next = FREE))
        compose.onNodeWithTag(learnRangeTag(StyleLearnRange.LastTwelveMonths))
            .assertIsSelected()
            .assert(hasText(L10n.learn_range_last_twelve_months(ctx()), substring = false))
        val first = compose.onNodeWithTag(learnRangeTag(StyleLearnRange.LastTwelveMonths)).getUnclippedBoundsInRoot()
        val all = compose.onNodeWithTag(learnRangeTag(StyleLearnRange.Everything)).getUnclippedBoundsInRoot()
        assertTrue("the core's first preset is drawn first", first.bottom <= all.top)
    }

    @Test
    fun the_range_says_what_next_fetches_and_nothing_when_it_fetches_nothing() {
        show(learning(chooseRange(fetch = StyleRangeFetch.OLDER), back = FREE, next = FREE))
        compose.onNodeWithText(L10n.learn_range_fetch_older(ctx())).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_range_fetch_all(ctx())).assertDoesNotExist()

        record = learning(chooseRange(StyleLearnRange.Everything, fetch = StyleRangeFetch.WHOLE_HISTORY), back = FREE, next = FREE)
        compose.waitForIdle()
        compose.onNodeWithText(L10n.learn_range_fetch_all(ctx())).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_range_fetch_older(ctx())).assertDoesNotExist()

        record = learning(chooseRange(StyleLearnRange.UntilEndOfLastYear, fetch = StyleRangeFetch.NOTHING), back = FREE, next = FREE)
        compose.waitForIdle()
        compose.onNodeWithText(L10n.learn_range_fetch_all(ctx())).assertDoesNotExist()
        compose.onNodeWithText(L10n.learn_range_fetch_older(ctx())).assertDoesNotExist()
        assertEquals("drawing sends nothing", emptyList<StyleLearningIntent>(), intents())
    }

    @Test
    fun a_range_of_ones_own_may_start_on_any_day_and_ends_by_the_core_s_last() {
        val days = learnRangeDays("2026-11-19")
        assertTrue("a day long before the device's mail", days.isSelectableDate(checkNotNull(dayMillis("1998-02-03"))))
        assertTrue("the last day", days.isSelectableDate(checkNotNull(dayMillis("2026-11-19"))))
        assertFalse("a day after it", days.isSelectableDate(checkNotNull(dayMillis("2026-11-20"))))
    }

    @Test
    fun choose_range_offers_the_presets_and_a_range_of_ones_own_within_the_core_s_days() {
        show(learning(chooseRange(), back = FREE, next = FREE))
        compose.onNodeWithText(L10n.learn_range_all(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_range_until_last_year(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(LEARN_SINCE_TAG).assertDoesNotExist()

        compose.onNodeWithTag(learnRangeTag(StyleLearnRange.Between("", ""))).performClick()
        compose.onNodeWithTag(learnRangeTag(StyleLearnRange.UntilEndOfLastYear)).performClick()
        assertEquals(
            listOf(
                StyleLearningIntent.SelectRange(StyleLearnRange.Between("2025-11-19", "2026-11-19")),
                StyleLearningIntent.SelectRange(StyleLearnRange.UntilEndOfLastYear),
            ),
            intents(),
        )
    }

    @Test
    fun a_range_of_ones_own_shows_its_two_days() {
        show(learning(chooseRange(selected = StyleLearnRange.Between("2025-04-01", "2026-01-31")), next = FREE))
        compose.onNodeWithTag(LEARN_SINCE_TAG).assertExists()
        compose.onNodeWithTag(LEARN_UNTIL_TAG).assertExists()
    }

    @Test
    fun a_preset_the_core_does_not_offer_is_not_drawn() {
        show(learning(chooseRange(presets = listOf(StyleLearnRange.Everything)), next = FREE))
        compose.onNodeWithText(L10n.learn_range_until_last_year(ctx())).assertDoesNotExist()
    }

    @Test
    fun the_review_reads_first_and_sends_nothing() {
        show(learning(review(state = StyleLearningReview.Reading), back = FREE, learn = HELD))
        compose.onNodeWithText(L10n.learn_reading(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsNotEnabled()
    }

    @Test
    fun the_review_fetches_older_mail_first_with_the_one_spinner() {
        show(learning(review(state = StyleLearningReview.Fetching), back = FREE, learn = HELD))
        compose.onNodeWithText(L10n.learn_fetching(ctx())).assertIsDisplayed()
        compose.onAllNodes(hasProgressBarRangeInfo(ProgressBarRangeInfo.Indeterminate)).assertCountEquals(1)
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(FLOW_BACK_TAG).assertIsEnabled()
    }

    @Test
    fun older_mail_that_could_not_be_fetched_is_said() {
        show(learning(review(olderNotFetched = true), back = FREE, learn = FREE))
        compose.onNodeWithText(L10n.learn_older_not_fetched(ctx())).assertExists()
        record = learning(review(), back = FREE, learn = FREE)
        compose.waitForIdle()
        compose.onNodeWithText(L10n.learn_older_not_fetched(ctx())).assertDoesNotExist()
    }

    @Test
    fun messages_that_could_not_be_loaded_are_said_in_either_review() {
        val states = listOf<(StyleLearningReport) -> StyleLearningReview>(
            { StyleLearningReview.Ready(it) },
            { StyleLearningReview.NothingUsable(it) },
        )
        show(learning(review(), back = FREE))
        for (state in states) {
            for (count in listOf(3, 1)) {
                record = learning(review(state = state(REPORT.copy(notLoaded = count.toUInt()))), back = FREE)
                compose.waitForIdle()
                compose.onNodeWithText(L10n.learn_not_loaded(ctx(), count)).performScrollTo().assertIsDisplayed()
            }
            record = learning(review(state = state(REPORT)), back = FREE)
            compose.waitForIdle()
            for (count in listOf(0, 1, 3)) {
                compose.onNodeWithText(L10n.learn_not_loaded(ctx(), count)).assertDoesNotExist()
            }
        }
    }

    @Test
    fun the_review_says_what_it_found_and_where_learn_sends_it() {
        show(learning(review(keywordKept = false), back = FREE, learn = FREE))
        compose.onNodeWithText(L10n.learn_report_found(ctx(), 120)).assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_report_usable(ctx(), 80)).assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_consent_relay(ctx())).assertExists()
        compose.onNodeWithText(L10n.learn_keyword_not_kept(ctx())).assertExists()

        compose.onNodeWithTag(FLOW_PRIMARY_TAG).performClick()
        assertEquals(
            listOf<StyleLearningIntent>(StyleLearningIntent.Learn(L10n.writing_style_default_name(ctx()), "en")),
            intents(),
        )
    }

    @Test
    fun the_review_names_the_own_endpoint_s_host_or_why_it_cannot_send() {
        show(learning(review(sendsTo = StyleLearningDestination.OwnEndpoint("llm.test.local")), learn = FREE))
        compose.onNodeWithText(L10n.learn_consent_own(ctx(), "llm.test.local")).assertExists()

        record = learning(review(sendsTo = null, cannotSend = AiFailure.NowhereToSend), learn = HELD)
        compose.waitForIdle()
        compose.onNodeWithText(L10n.ai_failure_nowhere_to_send(ctx())).assertExists()
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertIsNotEnabled()
    }

    @Test
    fun nothing_usable_says_so_and_names_no_destination() {
        val nothing = review(state = StyleLearningReview.NothingUsable(REPORT), sendsTo = null, cannotSend = null)
        show(learning(nothing, back = FREE, learn = HELD))
        compose.onNodeWithText(L10n.learn_report_nothing(ctx())).assertExists()
        // Nothing will be sent, so the record names no place and no reason, and neither is drawn.
        compose.onNodeWithText(L10n.learn_consent_title(ctx())).assertDoesNotExist()
        compose.onNodeWithText(L10n.ai_failure_nowhere_to_send(ctx())).assertDoesNotExist()
    }

    @Test
    fun learning_counts_once_it_knows_the_total_and_stop_stops_it() {
        show(learning(StyleLearningStage.Learning(0u, 0u), stop = FREE))
        compose.onNodeWithText(L10n.learn_progress_starting(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(FLOW_BACK_TAG).assertDoesNotExist()

        record = learning(StyleLearningStage.Learning(2u, 5u), stop = FREE)
        compose.waitForIdle()
        compose.onNodeWithText(L10n.learn_progress(ctx(), "2", "5")).assertIsDisplayed()
        compose.onNodeWithText(L10n.learn_stop(ctx())).performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Stop), intents())
    }

    @Test
    fun a_failure_offers_its_one_remedy_and_the_core_moves_the_flow() {
        show(learning(StyleLearningStage.Failed(AiFailure.NothingToLearn, offered(AiRemedy.CHOOSE_RANGE)), back = FREE))
        compose.onNodeWithText(L10n.ai_failure_nothing_to_learn(ctx())).assertIsDisplayed()
        compose.onNodeWithText(L10n.ai_remedy_choose_range(ctx())).performClick()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Remedy), intents())
        assertEquals("a step of the flow opens nothing else", 0, endpointsOpened + signIns.size)
    }

    @Test
    fun a_remedy_that_leads_elsewhere_goes_to_the_flow_alone_and_then_opens_there() {
        show(learning(StyleLearningStage.Failed(AiFailure.EndpointKeyRefused, offered(AiRemedy.OPEN_AI_ENDPOINT)), back = FREE))
        compose.onNodeWithText(L10n.ai_remedy_open_endpoint(ctx())).performClick()
        assertEquals("the flow ends on the remedy, never a Close", listOf<StyleLearningIntent>(StyleLearningIntent.Remedy), intents())
        assertEquals(1, endpointsOpened)

        sent.clear()
        record = learning(StyleLearningStage.Failed(AiFailure.RelaySignIn, offered(AiRemedy.SIGN_IN_TO_ALLODIA)), back = FREE)
        compose.waitForIdle()
        compose.onNodeWithText(L10n.ai_remedy_sign_in(ctx())).performClick()
        assertEquals(1, signIns.size)
        assertEquals("the flow waits on its review", listOf<StyleLearningIntent>(StyleLearningIntent.Remedy), intents())
    }

    @Test
    fun a_failure_without_a_remedy_draws_no_primary() {
        show(learning(StyleLearningStage.Failed(AiFailure.OutOfCredits, null), back = FREE))
        compose.onNodeWithTag(FLOW_PRIMARY_TAG).assertDoesNotExist()
        compose.onNodeWithTag(FLOW_BACK_TAG).assertIsDisplayed()
    }

    @Test
    fun close_and_back_send_their_intents() {
        show(learning(chooseRange(), back = FREE, next = FREE))
        compose.onNodeWithTag(FLOW_BACK_TAG).performClick()
        compose.onNodeWithTag(FLOW_CLOSE_TAG).performClick()
        assertEquals(listOf(StyleLearningIntent.Back, StyleLearningIntent.Close), intents())
    }

    @Test
    fun system_back_is_back_where_the_stage_has_one_and_close_where_it_has_not() {
        record = learning(chooseRange(), back = FREE, next = FREE)
        compose.setContent {
            AppTheme {
                WritingStyleFlow(testWritingStyleUi(stylePage(), record, sent), null, {}, {})
            }
        }
        compose.waitForIdle()
        pressBackInHost()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Back), intents())

        sent.clear()
        record = learning(chooseAccount(), next = HELD)
        compose.waitForIdle()
        pressBackInHost()
        assertEquals(listOf<StyleLearningIntent>(StyleLearningIntent.Close), intents())
    }

    // Presses back inside the dialog: its window has a dispatcher of its own.
    private fun pressBackInHost() {
        val owner = checkNotNull(hostDispatcher()) { "the flow host has no back dispatcher" }
        compose.runOnIdle { owner.onBackPressed() }
        compose.waitForIdle()
    }

    private fun hostDispatcher(): OnBackPressedDispatcher? {
        val dialog = ShadowDialog.getLatestDialog() as? ComponentDialog
        return dialog?.onBackPressedDispatcher
    }

    @Test
    @Config(qualifiers = "w320dp")
    @GraphicsMode(GraphicsMode.Mode.NATIVE)
    fun at_a_large_text_size_the_foot_stacks_rather_than_breaking_a_word() {
        // The dialog's window answers its own density, so the scale is the configuration's.
        RuntimeEnvironment.setFontScale(2f)
        show(learning(StyleLearningStage.Failed(AiFailure.NothingToLearn, offered(AiRemedy.CHOOSE_RANGE)), back = FREE))
        val primary = compose.onNodeWithTag(FLOW_PRIMARY_TAG).getUnclippedBoundsInRoot()
        val back = compose.onNodeWithTag(FLOW_BACK_TAG).getUnclippedBoundsInRoot()
        assertTrue("the primary sits above Back: $primary over $back", primary.bottom <= back.top)
        assertEquals("both span the foot", (primary.right - primary.left).value, (back.right - back.left).value, 1f)
    }

    @Test
    @Config(qualifiers = "w360dp")
    @GraphicsMode(GraphicsMode.Mode.NATIVE)
    fun at_a_large_text_size_a_day_s_label_keeps_its_words_whole() {
        RuntimeEnvironment.setFontScale(2f)
        show(learning(chooseRange(selected = StyleLearnRange.Between("2025-08-20", "2026-11-19")), next = FREE))
        for (label in listOf(L10n.learn_range_from(ctx()), L10n.learn_range_until_label(ctx()))) {
            val layouts = mutableListOf<TextLayoutResult>()
            compose.onNodeWithText(label).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
            val layout = layouts.single()
            val words = label.split(' ').size
            assertTrue("\"$label\" wraps only between words: ${layout.lineCount} lines", layout.lineCount <= words)
            for (line in 0 until layout.lineCount) {
                val end = layout.getLineEnd(line)
                assertTrue("\"$label\" breaks inside a word at $end", end == label.length || label[end - 1] == ' ')
            }
        }
    }

    @Test
    fun at_the_usual_text_size_the_foot_is_one_row() {
        show(learning(review(), back = FREE, learn = FREE))
        val primary = compose.onNodeWithTag(FLOW_PRIMARY_TAG).getUnclippedBoundsInRoot()
        val back = compose.onNodeWithTag(FLOW_BACK_TAG).getUnclippedBoundsInRoot()
        assertTrue("side by side", primary.left > back.right)
    }

    @Test
    fun learned_turns_into_its_reveal_in_the_same_host() {
        record = learning(StyleLearningStage.Learned("style-1"))
        compose.setContent {
            AppTheme {
                val ui = testWritingStyleUi(stylePage(), record, sent, reveal = { styleReveal(it) })
                WritingStyleFlow(ui, null, {}, {})
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText(L10n.reveal_title(ctx())).assertIsDisplayed()
        compose.onAllNodesWithTag(FLOW_CLOSE_TAG).assertCountEquals(1)
        compose.onNodeWithTag(FLOW_CLOSE_TAG).performClick()
        assertEquals(
            "one Close over a reveal, the reveal's own, after learning as from a row",
            listOf<WritingStyleIntent>(WritingStyleIntent.CloseReveal("style-1")),
            sent,
        )
    }

    @Test
    fun no_flow_and_no_reveal_draws_no_host() {
        compose.setContent { AppTheme { WritingStyleFlow(testWritingStyleUi(stylePage()), null, {}, {}) } }
        compose.onNodeWithTag(FLOW_HOST_TAG).assertDoesNotExist()
    }
}
