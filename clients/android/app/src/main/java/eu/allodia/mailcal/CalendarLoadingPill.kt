package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.unit.dp

/** The tag a test finds the pill by. */
internal const val CALENDAR_LOADING_PILL = "calendar-loading-pill"

/**
 * The pill a calendar grid shows over itself while its period is loading.
 *
 * A product decision, the same on every client (`docs/calendar.md`, "Showing a period that is
 * loading"): a period the core has not materialized keeps its grid, and a spinner beside "loading
 * this period" floats over it, pinned to what is on screen. The caller places it; it never moves
 * with the scrolled day.
 *
 * Deliberately not a `Surface`: a Material surface consumes the touches that land on it, and a press
 * on the grid under the pill has to reach the grid. [spoken] is whether the pill is what a screen
 * reader hears. The week's own semantic layer already says the period is loading, so its pill stays
 * silent rather than say it twice; the month has no such node.
 */
@Composable
internal fun CalendarLoadingPill(text: String, spoken: Boolean, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    val shape = RoundedCornerShape(percent = 50)
    Row(
        modifier = modifier
            .testTag(CALENDAR_LOADING_PILL)
            .shadow(elevation = 3.dp, shape = shape)
            .clip(shape)
            .background(colors.surfaceContainerHigh)
            .clearAndSetSemantics { if (spoken) contentDescription = text }
            .padding(horizontal = 14.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
        Text(text, style = MaterialTheme.typography.labelMedium, color = colors.onSurface)
    }
}
