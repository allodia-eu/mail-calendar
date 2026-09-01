package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import org.junit.Assert.assertEquals
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
    fun a_flat_phone_row_starts_with_the_sender_avatar_and_no_unread_gutter() {
        compose.setContent {
            AppTheme {
                Box {
                    SwipeableFlatMessageRow(
                        message = flatRow(),
                        activeZoneId = "Europe/Amsterdam",
                        inJunkFolder = false,
                        swipe = SwipeSettings(
                            left = SwipeActionKind.ARCHIVE,
                            right = SwipeActionKind.DELETE,
                        ),
                        onSwipe = { _, _, _ -> },
                        accounts = emptyList(),
                        onOpen = {},
                        onSetRead = { _, _, _ -> },
                        onSetFlagged = { _, _, _ -> },
                        onDelete = { _, _ -> },
                        onPermanentlyDelete = { _, _ -> },
                        onMarkAsSpam = { _, _ -> },
                        onMarkAsNotSpam = { _, _ -> },
                        onReply = { _, _, _, _, _, _, _ -> true },
                        onForward = { _, _, _, _, _, _, _ -> true },
                        replyRecipients = { _, _, _ -> null },
                    )
                }
            }
        }

        val left = compose.onNodeWithTag("mail-avatar", useUnmergedTree = true)
            .assertExists()
            .fetchSemanticsNode()
            .boundsInRoot
            .left
        val density = RuntimeEnvironment.getApplication().resources.displayMetrics.density
        assertEquals(16f * density, left, 1f)
    }

    @Test
    fun a_thread_row_draws_the_latest_senders_avatar() {
        compose.setContent {
            AppTheme {
                ThreadConversationRow(
                    thread = threadRow(),
                    activeZoneId = "Europe/Amsterdam",
                    onOpenThread = {},
                    onArchiveThread = {},
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
                    onOpen = {},
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
                ReadingIdentityHeader(message = message, reading = reading.value, onBack = {})
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
        hasAttachment = false,
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
