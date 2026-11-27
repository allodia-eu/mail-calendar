// The reading header (ReadingHeader.kt, docs/reading-actions.md, docs/timestamps.md).
//
// The names and addresses come from the snapshot's structured people, never from splitting the
// joined strings, so the fixtures give the joined strings values no person has: a header that read
// them would show words these tests never look for.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode
import uniffi.mailcal_bindings.MailPerson
import uniffi.mailcal_bindings.ReadingSnapshot

@RunWith(RobolectricTestRunner::class)
class ReadingHeaderTest {
    @get:Rule val rule = createComposeRule()

    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private val message = OpenedMessage(
        account = "acct-1",
        key = "m1",
        subject = "Quarterly report",
        from = "Row sender",
        avatar = stubAvatar(),
        date = "2026-07-10T11:34:00Z",
    )

    private fun snapshot(
        key: String = "m1",
        fromName: String = "Ada Lovelace",
        fromAddress: String = "ada@example.test",
        to: List<MailPerson> = listOf(MailPerson("Bob Babbage", "bob@example.test"), MailPerson("", "carol@example.test")),
        cc: List<MailPerson> = listOf(MailPerson("Dora", "dora@example.test")),
        pending: Boolean = false,
    ) = ReadingSnapshot(
        key = key,
        from = "joined-from",
        avatar = stubAvatar(),
        to = "joined-to",
        cc = "joined-cc",
        bcc = "",
        html = null,
        plain = if (pending) null else "Body",
        hasRemoteImages = false,
        loadError = false,
        attachments = emptyList(),
        invitation = null,
        pending = pending,
        fromName = fromName,
        fromAddress = fromAddress,
        toPeople = to,
        ccPeople = cc,
    )

    private fun show(reading: ReadingSnapshot?, opened: OpenedMessage = message) {
        rule.setContent {
            AppTheme {
                Column {
                    ReadingSubject(opened, reading)
                    ReadingSender(opened, reading, "Europe/Amsterdam")
                }
            }
        }
    }

    @Test
    fun theRecipientsAreNamedByNameAndByAddressWhereTheyHaveNone() {
        val to = L10n.compose_to(ctx)
        val cc = L10n.compose_cc(ctx)
        assertEquals(
            "$to: Bob Babbage, carol@example.test   $cc: Dora",
            recipientSummary(ctx, snapshot()),
        )
    }

    @Test
    fun aGroupWithNobodyInItIsLeftOut() {
        assertEquals("${L10n.compose_cc(ctx)}: Dora", recipientSummary(ctx, snapshot(to = emptyList())))
    }

    @Test
    fun theSenderIsNamedByTheHeaderOnceItArrivesAndByTheRowUntilThen() {
        assertEquals("Row sender", readingSenderName(message, null))
        assertEquals("Ada Lovelace", readingSenderName(message, snapshot()))
        // No display name: the address is the name line, and is not repeated under it.
        assertEquals("ada@example.test", readingSenderName(message, snapshot(fromName = "")))
        assertNull(readingSenderAddress(snapshot(fromName = "")))
    }

    @Test
    fun aPendingOrStaleSnapshotIsNotTheHeader() {
        assertNull(loadedHeader(message, snapshot(pending = true)))
        assertNull(loadedHeader(message, snapshot(key = "m0")))
    }

    @Test
    fun theSubjectIsShownInFullAndTheSendersAddressAlways() {
        show(snapshot())
        rule.onNodeWithText("Quarterly report").assertIsDisplayed()
        rule.onNodeWithText("Ada Lovelace").assertIsDisplayed()
        // What tells a real sender from one borrowing the name, so it is never folded away.
        rule.onNodeWithText("ada@example.test").assertIsDisplayed()
    }

    @Test
    // Real text measurement: the legacy graphics measure every glyph as nothing, so nothing wraps.
    @GraphicsMode(GraphicsMode.Mode.NATIVE)
    fun aLongSubjectTakesAsManyLinesAsItNeedsAndIsThePagesHeading() {
        val long = "Minutes of the quarterly planning meeting, with the budget for every team, " +
            "the hiring plan, the office move and the three questions nobody answered last time"
        show(snapshot(), message.copy(subject = long))
        val title = rule.onNodeWithTag("reading-subject").fetchSemanticsNode()
        val layouts = mutableListOf<TextLayoutResult>()
        title.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts)
        val layout = layouts.single()
        assertTrue(layout.lineCount > 2)
        assertFalse(layout.hasVisualOverflow)
        assertEquals(Unit, title.config.getOrNull(SemanticsProperties.Heading))
    }

    @Test
    fun theHeaderShowsTheListsDateAndTheDetailsTheFullOne() {
        show(snapshot())
        val relative = relativeDate(message.date, "Europe/Amsterdam", true)
        val absolute = localDateTime(message.date, "Europe/Amsterdam", true)
        rule.onNodeWithText(relative).assertIsDisplayed()
        rule.onNodeWithText(L10n.quote_sent(ctx)).assertDoesNotExist()

        rule.onNodeWithTag(READING_DETAILS_TOGGLE_TAG).performClick()
        // Last in a box that scrolls once it outgrows its cap, as it does on this small screen.
        rule.onNodeWithText(L10n.quote_sent(ctx)).performScrollTo().assertIsDisplayed()
        rule.onNodeWithText(absolute).assertIsDisplayed()
    }

    @Test
    fun theDetailsNameEveryPersonWithTheirAddress() {
        show(snapshot())
        rule.onNodeWithTag(READING_DETAILS_TAG).assertDoesNotExist()
        rule.onNodeWithTag(READING_DETAILS_TOGGLE_TAG).performClick()
        for (text in listOf("Bob Babbage", "bob@example.test", "carol@example.test", "Dora", "dora@example.test")) {
            rule.onNodeWithText(text).assertIsDisplayed()
        }
        rule.onNodeWithText("joined-to", substring = true).assertDoesNotExist()

        rule.onNodeWithTag(READING_DETAILS_TOGGLE_TAG).performClick()
        rule.waitForIdle()
        rule.onNodeWithTag(READING_DETAILS_TAG).assertDoesNotExist()
    }

    @Test
    fun theToggleReadsAsTheRecipientsAndSaysWhatAPressDoes() {
        show(snapshot())
        val toggle = rule.onNodeWithTag(READING_DETAILS_TOGGLE_TAG)
        fun node() = toggle.fetchSemanticsNode().config
        assertEquals(listOf(recipientSummary(ctx, snapshot())), node().getOrNull(SemanticsProperties.ContentDescription))
        assertEquals(L10n.a11y_show_message_details(ctx), node()[SemanticsActions.OnClick].label)

        toggle.performClick()
        assertEquals(L10n.a11y_hide_message_details(ctx), node()[SemanticsActions.OnClick].label)
    }

    @Test
    fun noToggleUntilTheHeaderArrives() {
        show(null)
        rule.onNodeWithText("Row sender").assertIsDisplayed()
        rule.onNodeWithTag(READING_DETAILS_TOGGLE_TAG).assertDoesNotExist()
    }
}
