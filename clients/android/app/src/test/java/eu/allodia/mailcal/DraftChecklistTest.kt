// A drafted reply's card (docs/ai.md, "Summary and checklist" and "Where they show"): the items in
// the core's order, a fill-in item that follows the reply and nothing else, the others ticked by the
// person, and Send asking once per composer while anything is open.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.DraftTask
import uniffi.mailcal_bindings.DraftTaskKind

internal val DRAFT_TASKS = listOf(
    DraftTask(DraftTaskKind.FILL_IN, "[date]"),
    DraftTask(DraftTaskKind.FILL_IN, "[time]"),
    DraftTask(DraftTaskKind.ATTACH, "Attach the agenda"),
    DraftTask(DraftTaskKind.DO, "Book the room"),
)

@RunWith(RobolectricTestRunner::class)
class DraftChecklistTest {
    private fun ctx() = RuntimeEnvironment.getApplication()

    private fun drafted(tasks: List<DraftTask> = DRAFT_TASKS, attachments: Int = 0) =
        DraftChecklist().apply { show("Bob asks when you can meet.", tasks, attachments) }

    @Test
    fun items_keep_the_core_s_order_and_a_later_draft_replaces_them() {
        val checklist = drafted()
        assertEquals(listOf("[date]", "[time]", "Attach the agenda", "Book the room"), checklist.items.map { it.text })
        assertEquals(L10n.composer_task_fill_in(ctx(), placeholder = "[date]"), checklist.items.first().title(ctx()))
        assertEquals("Book the room", checklist.items.last().title(ctx()))
        checklist.toggle(3)
        val first = checklist.draft
        checklist.show("", listOf(DraftTask(DraftTaskKind.DO, "Call Bob")), 0)
        assertEquals(listOf("Call Bob"), checklist.items.map { it.text })
        assertEquals(1, checklist.openCount)
        assertTrue(checklist.summary.isEmpty())
        assertNotEquals(first, checklist.draft)
    }

    @Test
    fun a_draft_with_nothing_to_say_shows_no_card() {
        assertTrue(DraftChecklist().isEmpty)
        val checklist = DraftChecklist()
        checklist.show("", emptyList(), 0)
        assertTrue(checklist.isEmpty)
        checklist.show("Bob asks for the slides.", emptyList(), 0)
        assertFalse(checklist.isEmpty)
    }

    /** Ticked exactly while the placeholder is gone, so an undo opens the item again. */
    @Test
    fun a_fill_in_item_follows_what_is_left_in_the_reply() {
        val checklist = drafted()
        assertEquals(listOf("[date]", "[time]"), checklist.placeholders)
        assertTrue(checklist.awaitsPlaceholders)
        checklist.placeholdersLeft(listOf("[time]"))
        assertEquals(listOf(true, false, false, false), checklist.items.map { it.ticked })
        assertEquals(checklist.draft, checklist.following)
        checklist.placeholdersLeft(emptyList())
        assertFalse(checklist.awaitsPlaceholders)
        assertNull(checklist.following)
        checklist.placeholdersLeft(listOf("[date]"))
        assertEquals(listOf(false, true, false, false), checklist.items.map { it.ticked })
        assertEquals("a reopened item is followed again", checklist.draft, checklist.following)
    }

    @Test
    fun only_the_items_the_editor_cannot_see_are_ticked_by_hand() {
        val checklist = drafted()
        checklist.toggle(0)
        assertFalse(checklist.items[0].ticked)
        checklist.toggle(2)
        checklist.toggle(3)
        assertTrue(checklist.items[2].ticked && checklist.items[3].ticked)
        checklist.toggle(3)
        assertFalse(checklist.items[3].ticked)
        checklist.placeholdersLeft(emptyList())
        assertTrue("the reply leaves an attach item alone", checklist.items[2].ticked)
    }

    /**
     * Which file answers which item cannot be told, so an attach item ticks only once there is a
     * new file for every one of them.
     */
    @Test
    fun attach_items_tick_once_each_has_a_new_file() {
        val tasks = listOf(
            DraftTask(DraftTaskKind.ATTACH, "Attach the agenda"),
            DraftTask(DraftTaskKind.ATTACH, "Attach the minutes"),
            DraftTask(DraftTaskKind.DO, "Book the room"),
        )
        val checklist = drafted(tasks, attachments = 1)
        checklist.attachmentsChanged(2)
        assertEquals(listOf(false, false, false), checklist.items.map { it.ticked })
        checklist.attachmentsChanged(3)
        assertEquals(listOf(true, true, false), checklist.items.map { it.ticked })

        val none = drafted(listOf(DraftTask(DraftTaskKind.DO, "Book the room")))
        none.attachmentsChanged(4)
        assertFalse(none.items[0].ticked)
    }

    @Test
    fun the_open_count_is_every_unticked_item() {
        val checklist = drafted()
        assertEquals(4, checklist.openCount)
        checklist.placeholdersLeft(listOf("[time]"))
        checklist.toggle(2)
        assertEquals(2, checklist.openCount)
    }

    /**
     * Either answer ends the asking for the composer, a later draft included; and with nothing
     * open it never asks at all.
     */
    @Test
    fun send_asks_once_and_only_while_something_is_open() {
        val checklist = drafted()
        assertTrue(checklist.asksBeforeSend)
        checklist.sendAsked()
        assertFalse(checklist.asksBeforeSend)
        checklist.show("", DRAFT_TASKS, 0)
        assertTrue(checklist.hasAsked && !checklist.asksBeforeSend)

        val done = drafted(listOf(DraftTask(DraftTaskKind.DO, "Book the room")))
        done.toggle(0)
        assertFalse(done.asksBeforeSend)
        assertFalse(DraftChecklist().asksBeforeSend)
    }

    @Test
    fun a_phone_collapses_a_long_list_until_the_person_chooses() {
        val checklist = drafted()
        assertTrue(checklist.isCollapsed(onPhone = true))
        assertFalse(checklist.isCollapsed(onPhone = false))
        assertFalse(drafted(DRAFT_TASKS.take(3)).isCollapsed(onPhone = true))
        checklist.collapsedChoice = false
        checklist.show("", DRAFT_TASKS, 0)
        assertFalse("the choice outlives a later draft", checklist.isCollapsed(onPhone = true))
    }
}
