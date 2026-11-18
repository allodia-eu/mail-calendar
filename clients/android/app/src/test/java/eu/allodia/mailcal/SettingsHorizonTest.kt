// Settings → Calendar's horizon picker draws the stops the core lists, in its order, and selects
// the horizon the grid shows, including one a pinch left between two stops. Nothing loads the
// cdylib: the display settings are a plain record built here. The rows are emitted into the
// caller's layout, so each test holds them in a column as Settings does, or they stack.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.filter
import androidx.compose.ui.test.isSelectable
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.junit4.v2.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarLayout
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.WeekStart

@RunWith(RobolectricTestRunner::class)
class SettingsHorizonTest {
    @get:Rule val compose = createComposeRule()

    private fun ctx() = RuntimeEnvironment.getApplication()

    private fun display(hours: Int, choices: List<Int>) = DisplaySettings(
        WeekStart.MONDAY,
        TimeFormat.TWENTY_FOUR_HOUR,
        Appearance.SYSTEM,
        true,
        hours.toUByte(),
        ByteArray(choices.size) { choices[it].toByte() },
        CalendarLayout.WEEK,
    )

    private fun label(hours: Int) = L10n.settings_horizon_hours(ctx(), hours.toString())

    @Test
    fun the_picker_offers_the_cores_stops_in_its_order() {
        val choices = listOf(4, 9, 24)
        compose.setContent { Column { CalendarHorizonSettingsRows(display(9, choices)) {} } }

        choices.forEach { compose.onNodeWithText(label(it)).assertExists() }
        compose.onAllNodesWithText(label(12)).assertCountEquals(0)
        assertEquals(choices, horizonChoices(display(9, choices)))
    }

    @Test
    fun a_horizon_a_pinch_left_between_two_stops_is_the_one_selected() {
        val choices = listOf(6, 8, 10, 12, 16, 24)
        compose.setContent { Column { CalendarHorizonSettingsRows(display(10, choices)) {} } }

        val radios = compose.onAllNodes(isSelectable())
        radios.assertCountEquals(choices.size)
        radios[choices.indexOf(10)].assertIsSelected()
        radios.filter(isSelected()).assertCountEquals(1)
    }

    @Test
    fun a_pick_sends_the_hours_it_names() {
        var picked: Int? = null
        compose.setContent { Column { CalendarHorizonSettingsRows(display(12, listOf(6, 8, 12, 16, 24))) { picked = it } } }

        compose.onNodeWithText(label(16)).performClick()

        assertEquals(16, picked)
    }
}
