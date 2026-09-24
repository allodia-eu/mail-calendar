// The reading screen's overflow menu (ReadingScreenOverflow.kt, docs/reading-actions.md).
//
// What is worth asserting here is that the menu is a menu: the export is reachable but not
// standing on the toolbar, where it would crowd out the actions people use on every message.
// The export itself is not driven from here, because it reaches the cdylib and this suite loads
// none.
package eu.allodia.mailcal

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.core.app.ApplicationProvider
import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.mailcal_bindings.ReadingSnapshot

@RunWith(RobolectricTestRunner::class)
class ReadingOverflowTest {
    @get:Rule val rule = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private fun show(onPrint: (() -> Unit)? = {}) {
        rule.setContent {
            ReadingOverflowMenu(
                subject = "Quarterly report",
                account = "acct-1",
                key = "m1",
                onExportMessage = { _, _, _ -> true },
                onPrint = onPrint,
            )
        }
    }

    private fun snapshot(pending: Boolean = false, loadError: Boolean = false) = ReadingSnapshot(
        key = "m1",
        from = "Ada Lovelace <ada@example.test>",
        avatar = stubAvatar(),
        to = "me@example.test",
        cc = "",
        bcc = "",
        html = if (pending) null else "<p>Body</p>",
        plain = null,
        hasRemoteImages = false,
        loadError = loadError,
        attachments = emptyList(),
        invitation = null,
        pending = pending,
    )

    @Test
    fun openingTheMenuOffersPrint() {
        var printed = false
        show(onPrint = { printed = true })
        rule.onNodeWithTag(READING_OVERFLOW_TAG).performClick()
        rule.onNodeWithText(L10n.action_print(ctx)).assertIsEnabled().performClick()
        assertTrue(printed)
    }

    @Test
    fun printWaitsForTheBody() {
        // Shown so the menu does not change shape as the message opens, but not pressable: a
        // printout of a message whose body has not arrived is a header and a blank page.
        show(onPrint = null)
        rule.onNodeWithTag(READING_OVERFLOW_TAG).performClick()
        rule.onNodeWithText(L10n.action_print(ctx)).assertIsNotEnabled()
    }

    @Test
    fun onlyAFetchedBodyIsPrintable() {
        assertFalse(isPrintable(null))
        assertFalse(isPrintable(snapshot(pending = true)))
        assertFalse(isPrintable(snapshot(loadError = true)))
        assertTrue(isPrintable(snapshot()))
    }

    @Test
    fun thePrintedHeaderSaysWhatTheScreenSays() {
        val message = OpenedMessage(
            account = "acct-1",
            key = "m1",
            subject = "Quarterly report",
            from = "Ada Lovelace",
            avatar = stubAvatar(),
            date = "10 July 2026 at 13:34",
        )
        val lines = messagePrintLines(ctx, message, snapshot())
        assertEquals(
            listOf(
                L10n.compose_from(ctx) to "Ada Lovelace <ada@example.test>",
                L10n.compose_to(ctx) to "me@example.test",
                L10n.compose_cc(ctx) to "",
                L10n.compose_bcc(ctx) to "",
                L10n.quote_sent(ctx) to "10 July 2026 at 13:34",
            ),
            lines.map { it.label to it.value },
        )
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
