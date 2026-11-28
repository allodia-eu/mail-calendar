package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.ThreadRow

@RunWith(RobolectricTestRunner::class)
class AvatarSurfacesTest {
    @get:Rule val compose = createComposeRule()

    @Test
    fun an_unread_flat_row_leads_with_the_dot_and_keeps_the_avatar_where_it_was() {
        setFlatRow(flatRow())

        val density = RuntimeEnvironment.getApplication().resources.displayMetrics.density
        val dot = compose.onNodeWithTag("unread-dot", useUnmergedTree = true)
            .assertExists()
            .fetchSemanticsNode()
            .boundsInRoot
        val avatar = compose.onNodeWithTag("mail-avatar", useUnmergedTree = true)
            .fetchSemanticsNode()
            .boundsInRoot
        // In the row's start padding, so the text keeps its width, and clear of the avatar.
        assertTrue(avatar.left - dot.right >= 3f * density)
        assertEquals(16f * density, avatar.left, 1f)
    }

    @Test
    fun a_read_flat_row_draws_no_dot() {
        setFlatRow(flatRow().copy(unread = false))

        compose.onNodeWithTag("unread-dot", useUnmergedTree = true).assertDoesNotExist()
    }

    @Test
    fun a_conversation_with_an_unread_message_leads_with_the_dot() {
        compose.setContent {
            AppTheme {
                ThreadConversationRow(
                    thread = threadRow(),
                    activeZoneId = "Europe/Amsterdam",
                    selected = false,
                    selecting = false,
                    onToggleSelect = {},
                    onOpenThread = {},
                )
            }
        }

        compose.onNodeWithTag("unread-dot", useUnmergedTree = true).assertExists()
    }

    private fun setFlatRow(message: FlatRow) {
        compose.setContent {
            AppTheme {
                Box {
                    SwipeableFlatMessageRow(
                        message = message,
                        activeZoneId = "Europe/Amsterdam",
                        swipe = SwipeSettings(
                            left = SwipeActionKind.ARCHIVE,
                            right = SwipeActionKind.DELETE,
                        ),
                        onSwipe = { _, _, _ -> },
                        selected = false,
                        selecting = false,
                        onToggleSelect = {},
                        onOpen = { _, _ -> },
                    )
                }
            }
        }
    }

    @Test
    fun a_thread_row_draws_the_latest_senders_avatar() {
        compose.setContent {
            AppTheme {
                ThreadConversationRow(
                    thread = threadRow(),
                    activeZoneId = "Europe/Amsterdam",
                    selected = false,
                    selecting = false,
                    onToggleSelect = {},
                    onOpenThread = {},
                )
            }
        }

        compose.onNodeWithTag("thread-avatar", useUnmergedTree = true).assertExists()
    }

    @Test
    fun a_collapsed_thread_message_draws_its_own_senders_avatar() {
        val older = threadMessage("older", "OT")
        val focused = threadMessage("focused", "FT")
        compose.setContent {
            AppTheme {
                ConversationStrip(
                    conversation = listOf(focused, older),
                    focusedKey = focused.key,
                    subject = "A conversation",
                    activeZoneId = "Europe/Amsterdam",
                    onOpen = { _, _ -> },
                )
            }
        }

        compose.onNodeWithTag("thread-message-avatar", useUnmergedTree = true).assertExists()
    }

    @Test
    fun the_reading_header_uses_the_row_avatar_until_its_matching_snapshot_arrives() {
        val message = openedMessage("ROW")
        val reading = mutableStateOf<ReadingSnapshot?>(null)
        compose.setContent {
            AppTheme {
                ReadingSender(message = message, reading = reading.value, activeZoneId = "Europe/Amsterdam")
            }
        }
        compose.onNodeWithText("ROW", useUnmergedTree = true).assertExists()

        compose.runOnIdle {
            reading.value = readingSnapshot(avatarInitials = "BODY")
        }
        compose.onNodeWithText("BODY", useUnmergedTree = true).assertExists()
    }

    private fun flatRow() = FlatRow(
        account = "acct-1",
        key = "m1",
        subject = "Quarterly report",
        from = "Ada Lovelace",
        avatar = stubAvatar("AL"),
        date = "2026-07-10T11:34:41Z",
        unread = true,
        flagged = false,
        hasAttachment = false,
        importance = uniffi.mailcal_bindings.MessageImportance.NORMAL,
        preview = "Numbers and notes",
    )

    private fun threadMessage(key: String, initials: String) = ThreadMessage(
        account = "acct-1",
        key = key,
        from = "Ada Lovelace",
        avatar = stubAvatar(initials),
        date = "2026-07-10T11:34:41Z",
        preview = "Numbers and notes",
        unread = true,
        outgoing = false,
        draft = false,
        hasAttachment = false,
        importance = uniffi.mailcal_bindings.MessageImportance.NORMAL,
    )

    private fun threadRow() = ThreadRow(
        account = "acct-1",
        threadId = "thread-1",
        latestKey = "m1",
        subject = "Quarterly report",
        latestFrom = "Ada Lovelace",
        avatar = stubAvatar("AL"),
        latestDate = "2026-07-10T11:34:41Z",
        messageCount = 2u,
        unreadCount = 1u,
        hasAttachment = false,
        importance = uniffi.mailcal_bindings.MessageImportance.NORMAL,
        preview = "Numbers and notes",
        messages = listOf(threadMessage("m1", "AL"), threadMessage("m0", "GH")),
    )

    private fun openedMessage(initials: String) = OpenedMessage(
        account = "acct-1",
        key = "m1",
        subject = "Quarterly report",
        from = "Ada Lovelace",
        avatar = stubAvatar(initials),
        date = "10 July 2026 at 13:34",
    )

    private fun readingSnapshot(avatarInitials: String) = ReadingSnapshot(
        key = "m1",
        from = "Ada Lovelace <ada@example.test>",
        avatar = stubAvatar(avatarInitials),
        to = "me@example.test",
        cc = "",
        bcc = "",
        html = null,
        plain = "Body",
        hasRemoteImages = false,
        loadError = false,
        attachments = emptyList(),
        invitation = null,
        pending = false,
    )
}
