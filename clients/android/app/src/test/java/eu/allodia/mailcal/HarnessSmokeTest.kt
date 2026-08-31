package eu.allodia.mailcal

import androidx.compose.material3.Text
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.SwipeActionKind

// Proves the three legs of the harness stand up on the JVM: Robolectric gives us a Context and the
// generated string catalog, Compose's test rule renders and queries a composable, and the generated
// UniFFI data types are usable without loading the cdylib.
@RunWith(RobolectricTestRunner::class)
class HarnessSmokeTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun robolectric_resolves_the_generated_l10n_catalog() {
        assertEquals("Undo", L10n.action_undo(RuntimeEnvironment.getApplication()))
    }

    @Test
    fun compose_rule_renders_and_queries() {
        compose.setContent { Text("hello") }
        compose.onNodeWithText("hello").assertIsDisplayed()
    }

    @Test
    fun generated_uniffi_types_load_without_the_cdylib() {
        assertEquals(3, SwipeActionKind.entries.size)
    }
}
