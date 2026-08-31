// The welcome screen's consent contract. Every assertion here is a legal condition from
// docs/analytics.md, not a styling preference, which is why they are pinned rather than left to
// a code review to notice:
//
//   * The switch is OFF until the user moves it. A pre-ticked box is not consent (GDPR Art. 4(11),
//     Recital 32; CJEU Planet49 C-673/17), and under ePrivacy Art. 5(3) the *act* of writing the
//     install id is itself what needs consent, so a default-on switch would be the violation, not
//     merely a dark pattern.
//   * "Get started" always works and always records an answer. Leaving the switch alone is a
//     *decline*, and recording it is what stops us asking again; dropping it on the floor would
//     mean re-asking every launch, which is nagging-into-consent.
//   * The payload panel shows the literal bytes, and only when asked for.
//
// Driven through the real composable with Compose's test rule. Nothing here loads the cdylib: the
// screen takes plain values and lambdas, which is the reason it was written that way.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

private const val PAYLOAD = """{"schema":1,"platform":"android","account_count":"1"}"""

@RunWith(RobolectricTestRunner::class)
class WelcomeScreenTest {
    @get:Rule val compose = createComposeRule()

    /** What the screen reported when the user left it. Null until they press "Get started". */
    private var decision: Boolean? = null
    private var previewsPulled = 0

    private fun show() {
        compose.setContent {
            WelcomeScreen(
                payloadPreview = { previewsPulled++; PAYLOAD },
                onGetStarted = { decision = it },
            )
        }
    }

    private fun ctx() = RuntimeEnvironment.getApplication()

    @Test
    fun the_switch_is_off_until_the_user_moves_it() {
        show()
        compose.onNodeWithText(L10n.welcome_analytics_toggle(ctx())).assertIsOff()
    }

    @Test
    fun getting_started_without_touching_the_switch_records_a_decline() {
        show()
        compose.onNodeWithText(L10n.welcome_get_started(ctx())).performClick()

        // Not `null`: an unanswered question would be re-asked on the next launch. A decline is an
        // answer, and it is the one we must remember.
        assertEquals(false, decision)
    }

    @Test
    fun opting_in_takes_a_deliberate_tap_and_then_a_confirmation() {
        show()
        compose.onNodeWithText(L10n.welcome_analytics_toggle(ctx())).performClick()
        compose.onNodeWithText(L10n.welcome_analytics_toggle(ctx())).assertIsOn()

        // Still nothing recorded, moving the switch alone does not write to the device.
        assertEquals(null, decision)

        compose.onNodeWithText(L10n.welcome_get_started(ctx())).performClick()
        assertEquals(true, decision)
    }

    @Test
    fun refusing_costs_nothing_the_way_forward_is_open_either_way() {
        show()
        compose.onNodeWithText(L10n.welcome_get_started(ctx())).assertIsEnabled()
        compose.onNodeWithText(L10n.welcome_analytics_toggle(ctx())).performClick()
        compose.onNodeWithText(L10n.welcome_get_started(ctx())).assertIsEnabled()
    }

    @Test
    fun the_payload_is_shown_verbatim_and_only_when_asked_for() {
        show()
        assertEquals("an unopened panel costs nothing", 0, previewsPulled)

        compose.onNodeWithText(L10n.welcome_analytics_preview(ctx())).performClick()

        // Scrolled to, not just asserted present: on a short screen the panel opens below the fold,
        // and a payload the user cannot reach is not the disclosure we are claiming to make.
        compose.onNodeWithText(PAYLOAD).performScrollTo().assertIsDisplayed()
        assertTrue("the panel pulled the payload when opened", previewsPulled >= 1)
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_screen_is_translated() {
        show()
        // The Dutch frame around the app's own name, not the whole heading: the name is injected
        // (docs/branding.md), so a literal here would assert which checkout the test ran in
        // rather than that the screen came out in Dutch.
        compose.onNodeWithText("Welkom bij", substring = true).assertIsDisplayed()
        compose.onNodeWithText("Gebruiksstatistieken delen").assertIsOff()
        compose.onNodeWithText("Aan de slag").assertIsEnabled()
    }

    /**
     * The consent copy may never call this data "anonymous", it is pseudonymous (there is a stable
     * install id), and saying otherwise is itself a transparency failure under GDPR Art. 5(1)(a).
     * Pinned in both locales because it is exactly the word a well-meaning edit reaches for.
     */
    @Test
    fun the_copy_never_claims_the_data_is_anonymous() {
        for (text in consentCopy(ctx())) {
            assertTrue(
                "consent copy must not claim anonymity: $text",
                !text.lowercase().contains("anonym"),
            )
        }
    }

    @Test
    @Config(qualifiers = "nl")
    fun the_dutch_copy_never_claims_the_data_is_anonymous() {
        for (text in consentCopy(ctx())) {
            assertTrue(
                "consent copy must not claim anonymity: $text",
                !text.lowercase().contains("anoniem") && !text.lowercase().contains("anonym"),
            )
        }
    }

    private fun consentCopy(ctx: android.content.Context) = listOf(
        L10n.welcome_title(ctx),
        L10n.welcome_tagline(ctx),
        L10n.welcome_analytics_toggle(ctx),
        L10n.welcome_analytics_body(ctx),
        L10n.welcome_analytics_preview(ctx),
        L10n.welcome_privacy_policy(ctx),
        L10n.welcome_get_started(ctx),
        L10n.settings_analytics_heading(ctx),
        L10n.settings_analytics_description(ctx),
        L10n.settings_analytics_toggle(ctx),
    )
}
