// How a flow's pages move: parts rising or popping into place as a page arrives, and bars growing
// from nothing. With animations removed in the system settings nothing moves: every value is drawn
// final from the first frame.
package eu.allodia.mailcal

import android.animation.ValueAnimator
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp

internal val RISE = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)
internal val POP = CubicBezierEasing(0.3f, 1.25f, 0.5f, 1f)
internal val SETTLE = CubicBezierEasing(0.32f, 0.72f, 0f, 1f)
internal val COUNT = CubicBezierEasing(0.33f, 1f, 0.68f, 1f)

/** Whether the person removed animations (Accessibility), which sets the animator scale to zero. */
@Composable
internal fun rememberReducedMotion(): Boolean = remember { !ValueAnimator.areAnimatorsEnabled() }

/**
 * Whether a page's pictures are on screen: false for the first frame, so they play in, and true
 * from the start when animations are removed or [still] asks for every value final.
 */
@Composable
internal fun rememberShown(still: Boolean = false): Boolean {
    val reduced = rememberReducedMotion() || still
    var shown by remember { mutableStateOf(reduced) }
    LaunchedEffect(Unit) { shown = true }
    return shown
}

/** How a part of a page arrives: rising into place, or popping in with a slight overshoot. */
internal enum class Entrance { RISE, POP }

/** Draws this hidden until [shown], then brings it in [delayMillis] later. */
@Composable
internal fun Modifier.entrance(shown: Boolean, kind: Entrance, delayMillis: Int): Modifier {
    val rise = kind == Entrance.RISE
    val progress by animateFloatAsState(
        targetValue = if (shown) 1f else 0f,
        animationSpec = tween(if (rise) 500 else 450, delayMillis, if (rise) RISE else POP),
        label = "entrance",
    )
    return graphicsLayer {
        alpha = progress.coerceIn(0f, 1f)
        translationY = (1f - progress) * (if (rise) 8.dp else 6.dp).toPx()
        if (!rise) {
            scaleX = 0.94f + 0.06f * progress
            scaleY = scaleX
        }
    }
}

/**
 * A bar drawn from the leading edge to [fraction] of the width, growing in from nothing once
 * [shown], without stretching its rounded ends.
 */
@Composable
internal fun Modifier.growingBar(shown: Boolean, fraction: Float, color: Color, radius: Dp, delayMillis: Int): Modifier {
    val grown by animateFloatAsState(
        targetValue = if (shown) fraction.coerceIn(0f, 1f) else 0f,
        animationSpec = tween(700, delayMillis, RISE),
        label = "bar",
    )
    return drawBehind {
        val width = size.width * grown
        if (width <= 0f) return@drawBehind
        val corner = minOf(radius.toPx(), width / 2, size.height / 2)
        val x = if (layoutDirection == LayoutDirection.Rtl) size.width - width else 0f
        drawRoundRect(color = color, topLeft = Offset(x, 0f), size = Size(width, size.height), cornerRadius = CornerRadius(corner))
    }
}
