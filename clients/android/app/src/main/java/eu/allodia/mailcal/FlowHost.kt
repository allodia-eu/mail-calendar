// The host for a flow of several steps (docs/ai.md, "The flow host"): a full-screen dialog over
// Settings, as the signature editor is, with Close in a top app bar and Back and the primary action
// in a bar at the foot. It draws the controls it is given and holds no step of its own: whichever
// stage is on screen, and whether there is a way back, is the caller's record.
//
// System back and its predictive gesture are Back while there is one, and Close while there is
// not. With a page count the foot draws an indicator between the two; without one it draws none.
package eu.allodia.mailcal

import androidx.activity.compose.PredictiveBackHandler
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.Measurable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlin.coroutines.cancellation.CancellationException
import uniffi.mailcal_bindings.ControlState

internal const val FLOW_HOST_TAG = "flow-host"
internal const val FLOW_CLOSE_TAG = "flow-close"
internal const val FLOW_BACK_TAG = "flow-back"
internal const val FLOW_PRIMARY_TAG = "flow-primary"
internal const val FLOW_INDICATOR_TAG = "flow-indicator"

/** One of the host's controls: its words, its state and what pressing it does. */
internal class FlowAction(
    val label: String,
    val control: ControlState,
    val onClick: () -> Unit,
    // The lesser kind of primary, drawn in outline: Stop rather than Next.
    val outlined: Boolean = false,
)

/** Which of a known number of pages is on screen, for the indicator. */
internal data class FlowPages(val count: Int, val current: Int)

/** What the host draws around a stage. */
internal class FlowChrome(
    val title: String,
    val close: FlowAction,
    val back: FlowAction? = null,
    val primary: FlowAction? = null,
    val pages: FlowPages? = null,
)

/** What system back does: Back while there is one, Close while there is not. */
internal fun FlowChrome.systemBack(): FlowAction = back ?: close

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun FlowHost(chrome: FlowChrome, content: @Composable (PaddingValues) -> Unit) {
    val reduced = rememberReducedMotion()
    val current by rememberUpdatedState(chrome)
    var gesture by remember { mutableFloatStateOf(0f) }
    Dialog(
        // Back is the handler's below, so the dialog's own dismissal is never the system's.
        onDismissRequest = {},
        properties = DialogProperties(
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
            dismissOnBackPress = false,
            dismissOnClickOutside = false,
        ),
    ) {
        SystemBarsMatchTheme()
        PredictiveBackHandler { progress ->
            try {
                progress.collect { event -> if (!reduced) gesture = event.progress }
                gesture = 0f
                val action = current.systemBack()
                if (action.control.enabled) action.onClick()
            } catch (cancelled: CancellationException) {
                gesture = 0f
                throw cancelled
            }
        }
        Surface(
            modifier = Modifier
                .fillMaxSize()
                .testTag(FLOW_HOST_TAG)
                .graphicsLayer {
                    // The page draws back a little under the finger, as Material's predictive
                    // back does for a screen being left.
                    val scale = 1f - 0.08f * gesture
                    scaleX = scale
                    scaleY = scale
                }
                .clip(if (gesture > 0f) MaterialTheme.shapes.extraLarge else RectangleShape),
        ) {
            Scaffold(
                modifier = Modifier.imePadding(),
                topBar = {
                    TopAppBar(
                        title = { Text(chrome.title, maxLines = 1, overflow = TextOverflow.Ellipsis) },
                        navigationIcon = {
                            ControlIconButton(
                                icon = R.drawable.ic_close,
                                label = chrome.close.label,
                                control = chrome.close.control,
                                tag = FLOW_CLOSE_TAG,
                                onClick = chrome.close.onClick,
                            )
                        },
                    )
                },
                bottomBar = {
                    if (chrome.back != null || chrome.primary != null || chrome.pages != null) {
                        Column(modifier = Modifier.navigationBarsPadding()) {
                            HorizontalDivider()
                            FlowFoot(chrome)
                        }
                    }
                },
            ) { padding -> content(padding) }
        }
    }
}

// Back at the start, the indicator in the middle, the primary at the end; when the three do not
// fit side by side at the person's text size, they stack, the primary on top and full width, so a
// label wraps between words rather than inside one.
@Composable
private fun FlowFoot(chrome: FlowChrome) {
    val back = chrome.back
    val primary = chrome.primary
    Layout(
        contents = listOf(
            {
                if (back != null) {
                    ControlTextButton(label = back.label, control = back.control, tag = FLOW_BACK_TAG, onClick = back.onClick)
                }
            },
            { chrome.pages?.let { FlowIndicator(it) } },
            {
                if (primary != null) {
                    ControlButton(
                        label = primary.label,
                        control = primary.control,
                        tag = FLOW_PRIMARY_TAG,
                        outlined = primary.outlined,
                        onClick = primary.onClick,
                    )
                }
            },
        ),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
    ) { (backSlot, indicatorSlot, primarySlot), constraints ->
        val gap = 12.dp.roundToPx()
        val width = constraints.maxWidth
        val natural = { slot: List<Measurable> ->
            slot.sumOf { it.maxIntrinsicWidth(Constraints.Infinity) }
        }
        val back = natural(backSlot)
        val indicator = natural(indicatorSlot)
        val primary = natural(primarySlot)
        // The indicator is centred on the foot, so with one each button must fit in the half of
        // the row beside it, not merely in what the other two leave.
        val fits = if (indicatorSlot.isEmpty()) {
            back + primary + 2 * gap <= width
        } else {
            2 * maxOf(back, primary) + indicator + 2 * gap <= width
        }
        val loose = constraints.copy(minWidth = 0, minHeight = 0)
        if (fits) {
            val placedBack = backSlot.map { it.measure(loose) }
            val placedIndicator = indicatorSlot.map { it.measure(loose) }
            val placedPrimary = primarySlot.map { it.measure(loose) }
            val height = (placedBack + placedIndicator + placedPrimary).maxOfOrNull { it.height } ?: 0
            layout(width, height) {
                placedBack.forEach { it.placeRelative(0, (height - it.height) / 2) }
                placedIndicator.forEach { it.placeRelative((width - it.width) / 2, (height - it.height) / 2) }
                placedPrimary.forEach { it.placeRelative(width - it.width, (height - it.height) / 2) }
            }
        } else {
            val full = Constraints.fixedWidth(width).copy(minHeight = 0, maxHeight = constraints.maxHeight)
            val stacked = primarySlot.map { it.measure(full) to true } +
                indicatorSlot.map { it.measure(loose) to false } +
                backSlot.map { it.measure(full) to true }
            val height = stacked.sumOf { it.first.height } + gap * (stacked.size - 1).coerceAtLeast(0)
            layout(width, height) {
                var y = 0
                stacked.forEach { (placeable, wide) ->
                    placeable.placeRelative(if (wide) 0 else (width - placeable.width) / 2, y)
                    y += placeable.height + gap
                }
            }
        }
    }
}

// Dots, the current one drawn long, read as "Page 2 of 6".
@Composable
private fun FlowIndicator(pages: FlowPages) {
    val ctx = LocalContext.current
    val spoken = L10n.a11y_flow_page(ctx, page = (pages.current + 1).toString(), count = pages.count)
    Row(
        modifier = Modifier
            .padding(horizontal = 8.dp)
            .testTag(FLOW_INDICATOR_TAG)
            .clearAndSetSemantics { contentDescription = spoken },
    ) {
        repeat(pages.count) { index ->
            val on = index == pages.current
            val width by animateDpAsState(if (on) 18.dp else 6.dp, tween(300, easing = SETTLE), label = "dot")
            Box(
                modifier = Modifier
                    .padding(horizontal = 3.dp, vertical = 5.dp)
                    .size(width, 6.dp)
                    .clip(CircleShape)
                    .background(
                        if (on) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.38f)
                        },
                    ),
            )
        }
    }
}
