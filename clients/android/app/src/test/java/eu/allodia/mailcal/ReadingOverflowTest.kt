// The reading screen's overflow menu (ReadingScreenOverflow.kt, docs/reading-actions.md).
//
// What is worth asserting here is that the menu is a menu: the export is reachable but not
// standing on the toolbar, where it would crowd out the actions people use on every message.
// The export itself is not driven from here, because it reaches the cdylib and this suite loads
// none.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import android.content.Context
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingOverflowTest {
    @get:Rule val rule = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private fun show() {
        rule.setContent {
            ReadingOverflowMenu(
                subject = "Quarterly report",
                account = "acct-1",
                key = "m1",
                onExportMessage = { _, _, _ -> true },
            )
        }
    }

    @Test
    fun theExportIsBehindTheMenuRatherThanOnTheToolbar() {
        show()
        rule.onNodeWithText(L10n.action_save_as_eml(ctx)).assertDoesNotExist()
    }

    @Test
    fun openingTheMenuOffersTheExport() {
        show()
        rule.onNodeWithTag(READING_OVERFLOW_TAG).performClick()
        rule.onNodeWithText(L10n.action_save_as_eml(ctx)).assertIsDisplayed()
    }

    @Test
    fun theButtonIsNamedForAScreenReader() {
        // An icon with no label is announced as "button" and nothing else; this menu has no
        // visible text of its own, so the content description is its only name.
        show()
        rule.onNodeWithContentDescription(L10n.a11y_more_actions(ctx)).assertIsDisplayed()
    }
}
