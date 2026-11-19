// Settings → Advanced's own endpoint form as the core draws it (docs/ai.md rules 6 and 7): each gap
// the core names said under its field and Save held meanwhile, no key sent for an empty key field,
// the kept key never drawn back, every field held while a change runs, and what a change came to
// said once. Driven through the real composable.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.ui.Modifier
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.AiEndpointAddressGap
import uniffi.mailcal_bindings.AiEndpointChangeKind
import uniffi.mailcal_bindings.AiEndpointDeclarationGap
import uniffi.mailcal_bindings.AiEndpointForm
import uniffi.mailcal_bindings.AiEndpointGaps
import uniffi.mailcal_bindings.AiEndpointIntent
import uniffi.mailcal_bindings.AiEndpointOutcome
import uniffi.mailcal_bindings.AiEndpointResult
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.JurisdictionMode

// The core's rule, as far as these tests need it: an address on this device may be plain HTTP.
private fun gaps(address: String, model: String, declared: JurisdictionClass?) = AiEndpointGaps(
    address = when {
        address.isBlank() -> AiEndpointAddressGap.MISSING
        address.startsWith("http://") && !address.startsWith("http://localhost") -> AiEndpointAddressGap.NEEDS_HTTPS
        else -> null
    },
    modelMissing = model.isBlank(),
    declaration = when (declared) {
        null -> AiEndpointDeclarationGap.Missing
        JurisdictionClass.NON_EU -> AiEndpointDeclarationGap.Refused(JurisdictionMode.EU_NATIVE, declared)
        else -> null
    },
)

@RunWith(RobolectricTestRunner::class)
class SettingsAiEndpointTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private val sent = mutableListOf<AiEndpointIntent>()
    private var form by mutableStateOf<AiEndpointForm?>(endpointForm())

    private fun show() {
        compose.setContent {
            AppTheme {
                Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                    AiEndpointCard(AiEndpointUi(form, ::gaps) { sent += it })
                }
            }
        }
        compose.waitForIdle()
    }

    private fun save() = compose.onNodeWithTag(AI_ENDPOINT_SAVE_TAG).performScrollTo()

    @Test
    fun nothing_is_drawn_while_the_feature_is_not_offered() {
        form = null
        show()
        compose.onNodeWithText(L10n.ai_endpoint_title(ctx())).assertDoesNotExist()
    }

    @Test
    fun each_gap_is_said_under_its_field_and_holds_save() {
        show()
        save().assertIsNotEnabled()
        compose.onNodeWithTag(AI_ENDPOINT_ADDRESS_TAG).performTextInput("http://llm.test.local/v1")
        compose.onNodeWithText(L10n.ai_endpoint_gap_needs_https(ctx())).assertIsDisplayed()

        compose.onNodeWithTag(aiEndpointWhereTag(JurisdictionClass.NON_EU)).performScrollTo().performClick()
        compose.onNodeWithText(L10n.ai_endpoint_gap_refused_eu_native(ctx())).performScrollTo().assertIsDisplayed()
        save().assertIsNotEnabled()
    }

    @Test
    fun save_sends_the_fields_as_typed_and_no_key_for_an_empty_key_field() {
        form = endpointForm(keyKept = true)
        show()
        compose.onNodeWithText(L10n.ai_endpoint_key_stored(ctx())).assertIsDisplayed()
        compose.onNodeWithTag(AI_ENDPOINT_ADDRESS_TAG).performTextInput("http://localhost:11434/v1")
        compose.onNodeWithTag(AI_ENDPOINT_MODEL_TAG).performTextInput("mistral")
        compose.onNodeWithTag(aiEndpointWhereTag(JurisdictionClass.EU_NATIVE)).performScrollTo().performClick()
        save().assertIsEnabled().performClick()
        assertEquals(
            listOf<AiEndpointIntent>(AiEndpointIntent.Save("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE, null)),
            sent,
        )
    }

    @Test
    fun a_typed_key_is_sent_and_the_kept_one_is_never_drawn_back() {
        form = endpointForm("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE, keyKept = true, remove = FREE)
        show()
        compose.onNodeWithTag(AI_ENDPOINT_KEY_TAG).assert(hasText(""))
        compose.onNodeWithTag(AI_ENDPOINT_KEY_TAG).performTextInput("sk-test")
        save().performClick()
        assertEquals(
            listOf<AiEndpointIntent>(AiEndpointIntent.Save("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE, "sk-test")),
            sent,
        )
    }

    @Test
    fun remove_is_drawn_from_the_form_and_sends_remove() {
        form = endpointForm("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE, remove = FREE)
        show()
        compose.onNodeWithTag(AI_ENDPOINT_REMOVE_TAG).performScrollTo().assertIsEnabled().performClick()
        assertEquals(listOf<AiEndpointIntent>(AiEndpointIntent.Remove), sent)
    }

    @Test
    fun a_running_save_holds_every_field_and_both_buttons() {
        form = endpointForm("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE, save = STYLE_RUNNING, remove = HELD)
        show()
        compose.onNodeWithTag("$AI_ENDPOINT_SAVE_TAG-running").assertExists()
        compose.onNodeWithTag(AI_ENDPOINT_ADDRESS_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(AI_ENDPOINT_KEY_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(AI_ENDPOINT_MODEL_TAG).assertIsNotEnabled()
        compose.onNodeWithTag(aiEndpointWhereTag(JurisdictionClass.EU_HOSTED)).assertIsNotEnabled()
        compose.onNodeWithTag(AI_ENDPOINT_REMOVE_TAG).assertIsNotEnabled()
    }

    @Test
    fun a_key_the_keystore_refused_is_said_once_its_outcome_is_new() {
        val refused = AiEndpointOutcome(1uL, AiEndpointChangeKind.SAVE, AiEndpointResult.KeyNotStored)
        form = endpointForm().copy(outcome = refused)
        show()
        compose.onNodeWithText(L10n.ai_endpoint_key_not_stored(ctx())).assertDoesNotExist()

        form = endpointForm().copy(outcome = refused.copy(serial = 2uL))
        compose.waitForIdle()
        compose.onNodeWithText(L10n.ai_endpoint_key_not_stored(ctx())).assertIsDisplayed()
    }

    @Test
    fun a_redraw_keeps_what_was_typed() {
        form = endpointForm("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE)
        show()
        compose.onNodeWithTag(AI_ENDPOINT_MODEL_TAG).performTextReplacement("mistral-large")
        form = endpointForm("http://localhost:11434/v1", "mistral", JurisdictionClass.EU_NATIVE, keyKept = true)
        compose.waitForIdle()
        compose.onNodeWithTag(AI_ENDPOINT_MODEL_TAG).assert(hasText("mistral-large"))
    }
}
