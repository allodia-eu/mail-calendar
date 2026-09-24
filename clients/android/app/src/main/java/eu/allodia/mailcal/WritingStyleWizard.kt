// The frame the two Writing style sheets share (docs/ai.md, "Learning"): full screen, as this
// platform's settings flows are, one page at a time in a pager, and Back, the page dots and Next in
// a bar at the foot. With animations removed in the system settings nothing slides, grows or
// counts: every value is drawn final.
package eu.allodia.mailcal

import android.animation.ValueAnimator
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.pager.HorizontalPager
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties

// What stands where Next does.
internal sealed interface WizardPrimary {
    // Next, which the frame performs.
    data class Next(val enabled: Boolean) : WizardPrimary

    // The caller's own action in its place: Save, Learn, Stop, Close.
    class Action(
        val title: String,
        val enabled: Boolean = true,
        val prominent: Boolean = true,
        val run: () -> Unit,
    ) : WizardPrimary

    // Nothing yet.
    data object None : WizardPrimary
}

// Whether the person has removed animations (Settings → Accessibility), which sets the animator
// duration scale to zero.
@Composable
internal fun rememberReducedMotion(): Boolean = remember { !ValueAnimator.areAnimatorsEnabled() }

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun WizardSheet(
    pager: WizardPager,
    onClose: () -> Unit,
    showsBack: Boolean,
    primary: WizardPrimary,
    // Whether a swipe or a dot moves to any page; only where every page can be reached in any order.
    allowsJump: Boolean = false,
    // Drawn in the top bar beside Close: the reveal's language control.
    topBar: @Composable () -> Unit = {},
    // Questions asked over the sheet, drawn inside its window so back reaches them first.
    dialogs: @Composable () -> Unit = {},
    page: @Composable (index: Int) -> Unit,
) {
    val ctx = LocalContext.current
    val reduced = rememberReducedMotion()
    val state = rememberPagerState { pager.count }
    LaunchedEffect(pager.index) {
        if (state.currentPage == pager.index) return@LaunchedEffect
        if (reduced) {
            state.scrollToPage(pager.index)
        } else {
            state.animateScrollToPage(pager.index, animationSpec = spring(0.9f, Spring.StiffnessMediumLow))
        }
    }
    LaunchedEffect(state) { snapshotFlow { state.currentPage }.collect(pager::go) }
    val back = { pager.go(pager.index - 1) }

    Dialog(
        // The system back steps back a page while Back is on screen, as the button does.
        onDismissRequest = { if (showsBack) back() else onClose() },
        properties = DialogProperties(usePlatformDefaultWidth = false, decorFitsSystemWindows = false),
    ) {
        SystemBarsMatchTheme()
        Surface(modifier = Modifier.fillMaxSize()) {
            Scaffold(
                modifier = Modifier.imePadding(),
                topBar = {
                    TopAppBar(
                        title = topBar,
                        navigationIcon = {
                            IconButton(onClick = onClose) {
                                Icon(
                                    painter = painterResource(R.drawable.ic_close),
                                    contentDescription = L10n.action_cancel(ctx),
                                )
                            }
                        },
                    )
                },
                bottomBar = {
                    Column(modifier = Modifier.navigationBarsPadding()) {
                        HorizontalDivider()
                        WizardFooter(pager, showsBack, primary, allowsJump, back)
                    }
                },
            ) { padding ->
                HorizontalPager(
                    state = state,
                    modifier = Modifier.fillMaxSize().padding(padding),
                    userScrollEnabled = allowsJump,
                ) { index -> page(index) }
            }
            dialogs()
        }
    }
}

@Composable
private fun WizardFooter(
    pager: WizardPager,
    showsBack: Boolean,
    primary: WizardPrimary,
    allowsJump: Boolean,
    back: () -> Unit,
) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(modifier = Modifier.weight(1f)) {
            // Keeps its place while hidden, so Next does not move under the finger.
            TextButton(
                onClick = back,
                enabled = showsBack,
                modifier = if (showsBack) Modifier else Modifier.alpha(0f).clearAndSetSemantics {},
            ) {
                Text(L10n.wizard_back(ctx))
            }
        }
        WizardDots(pager, allowsJump)
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.CenterEnd) {
            when (primary) {
                is WizardPrimary.Next -> Button(onClick = { pager.go(pager.index + 1) }, enabled = primary.enabled) {
                    Text(L10n.wizard_next(ctx))
                }
                is WizardPrimary.Action -> if (primary.prominent) {
                    Button(onClick = primary.run, enabled = primary.enabled) { Text(primary.title) }
                } else {
                    OutlinedButton(onClick = primary.run, enabled = primary.enabled) { Text(primary.title) }
                }
                WizardPrimary.None -> Unit
            }
        }
    }
}

@Composable
private fun WizardDots(pager: WizardPager, allowsJump: Boolean) {
    val ctx = LocalContext.current
    val step = L10n.a11y_wizard_step(ctx, step = (pager.index + 1).toString(), total = pager.count.toString())
    Row(modifier = Modifier.clearAndSetSemantics { contentDescription = step }) {
        repeat(pager.count) { index ->
            val current = index == pager.index
            val width by animateDpAsState(if (current) 18.dp else 6.dp, tween(300, easing = SETTLE), label = "dot")
            Box(
                modifier = Modifier
                    .then(if (allowsJump) Modifier.clickable { pager.go(index) } else Modifier)
                    .padding(horizontal = 3.dp, vertical = 5.dp)
                    .size(width, 6.dp)
                    .clip(CircleShape)
                    .background(
                        if (current) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.38f)
                        },
                    ),
            )
        }
    }
}

// A page: its heading, then its content, and `bottom` held to the foot of the page when there is
// room. The whole page scrolls when it does not fit.
@Composable
internal fun WizardPage(
    title: String,
    bottom: (@Composable ColumnScope.() -> Unit)? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val viewport = maxHeight
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .heightIn(min = viewport)
                .padding(horizontal = WIZARD_GUTTER, vertical = 16.dp),
            verticalArrangement = Arrangement.SpaceBetween,
        ) {
            Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                Text(
                    title,
                    style = MaterialTheme.typography.headlineSmall,
                    modifier = Modifier.semantics { heading() },
                )
                content()
            }
            if (bottom != null) {
                Column(modifier = Modifier.padding(top = 16.dp), content = bottom)
            }
        }
    }
}

internal val WIZARD_GUTTER = 20.dp

// The prototype's curves.
internal val RISE = CubicBezierEasing(0.2f, 0.8f, 0.2f, 1f)
internal val POP = CubicBezierEasing(0.3f, 1.25f, 0.5f, 1f)
internal val SETTLE = CubicBezierEasing(0.32f, 0.72f, 0f, 1f)
internal val COUNT = CubicBezierEasing(0.33f, 1f, 0.68f, 1f)

// Whether a page's pictures are on screen: false for the first frame, so they play in, and true
// from the start when animations are removed.
@Composable
internal fun rememberShown(): Boolean {
    val reduced = rememberReducedMotion()
    var shown by remember { mutableStateOf(reduced) }
    LaunchedEffect(Unit) { shown = true }
    return shown
}

// How a part of a page arrives: rising into place, or popping in with a slight overshoot.
internal enum class WizardEntrance { RISE, POP }

// Draws this hidden until `shown`, then brings it in `delayMillis` later.
@Composable
internal fun Modifier.wizardEntrance(shown: Boolean, kind: WizardEntrance, delayMillis: Int): Modifier {
    val rise = kind == WizardEntrance.RISE
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

// A bar drawn from the leading edge to `fraction` of the width, growing in from nothing once
// `shown`, without stretching its rounded ends.
@Composable
internal fun Modifier.growingBar(
    shown: Boolean,
    fraction: Float,
    color: Color,
    radius: Dp,
    delayMillis: Int,
): Modifier {
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
        drawRoundRect(
            color = color,
            topLeft = Offset(x, 0f),
            size = Size(width, size.height),
            cornerRadius = CornerRadius(corner),
        )
    }
}
