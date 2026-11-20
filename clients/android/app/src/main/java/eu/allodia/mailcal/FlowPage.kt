// One stage or page inside the flow host (FlowHost.kt).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

internal val FLOW_GUTTER = 20.dp

/** The widest a page's column grows. */
internal val FLOW_READING_WIDTH = 640.dp

/** One answer of a single choice, the whole row its target, read as a radio button. */
@Composable
internal fun ChoiceRow(
    label: String,
    selected: Boolean,
    tag: String,
    enabled: Boolean = true,
    note: String? = null,
    onSelect: () -> Unit,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .selectable(selected = selected, enabled = enabled, role = Role.RadioButton, onClick = onSelect)
            .padding(vertical = 4.dp)
            .testTag(tag),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        RadioButton(selected = selected, onClick = null, enabled = enabled)
        Column(modifier = Modifier.padding(start = 12.dp)) {
            Text(label, style = MaterialTheme.typography.bodyLarge)
            note?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/**
 * A page: its heading, then its content, and [bottom] held to the foot when there is room. The
 * whole page scrolls when it does not fit.
 */
@Composable
internal fun FlowPage(
    title: String?,
    modifier: Modifier = Modifier,
    // A page that opens something, the first of a reveal, takes the larger heading.
    large: Boolean = false,
    bottom: (@Composable ColumnScope.() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    // A line of reading width on a tablet or in landscape, centred, rather than the screen's width.
    BoxWithConstraints(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        val viewport = maxHeight
        Column(
            modifier = Modifier
                .widthIn(max = FLOW_READING_WIDTH)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .heightIn(min = viewport)
                .padding(horizontal = FLOW_GUTTER, vertical = 16.dp),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                if (title != null) {
                    Text(
                        title,
                        style = if (large) MaterialTheme.typography.headlineMedium else MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.semantics { heading() },
                    )
                }
                content()
            }
            if (bottom != null) {
                Column(modifier = Modifier.padding(top = 16.dp), content = bottom)
            }
        }
    }
}
