// What a calendar picker offers out of the core's groups (`calendar_groups`): the writable calendars,
// in the core's order, under each account that has one. The grouping and its order are the core's
// and tested there; this is the filter that lives on this side.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.CalendarColor
import uniffi.mailcal_bindings.CalendarGroup
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.Swatch

private fun row(account: String, id: String, canWrite: Boolean) = CalendarRow(
    account = account,
    accountAddress = "$account@example.org",
    id = id,
    name = id,
    color = CalendarColor(
        hex = "#2f6fa8",
        light = Swatch("#2f6fa8", "#ffffff", "#245782"),
        dark = Swatch("#23537e", "#ffffff", "#2f6fa8"),
    ),
    visible = true,
    canWrite = canWrite,
    canInvite = false,
    isDefault = false,
)

private fun group(account: String, vararg calendars: CalendarRow) = CalendarGroup(
    account = account,
    address = "$account@example.org",
    expanded = true,
    calendars = calendars.toList(),
)

class CalendarGroupsTest {
    @Test
    fun a_picker_keeps_the_cores_order_and_only_the_writable_calendars() {
        // Deliberately not alphabetical: the order is the core's account order, never re-sorted.
        val groups = listOf(
            group("zed", row("zed", "work", canWrite = true), row("zed", "holidays", canWrite = false)),
            group("ada", row("ada", "home", canWrite = true)),
        )
        val offered = writableGroups(groups)
        assertEquals(listOf("zed", "ada"), offered.map { it.account })
        assertEquals(listOf("work"), offered.first().calendars.map { it.id })
    }

    @Test
    fun an_account_with_nothing_writable_gets_no_heading() {
        val groups = listOf(
            group("ada", row("ada", "holidays", canWrite = false)),
            group("zed", row("zed", "work", canWrite = true)),
        )
        assertEquals(listOf("zed"), writableGroups(groups).map { it.account })
    }
}
