// Settings → Writing style's flow host (docs/ai.md rules 11 to 15): the learning flow while the
// core draws one, its reveal once it has learned, and the reveal a library row opens. One host for
// all three, so learning turns into its reveal in the same window.
package eu.allodia.mailcal

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.pager.rememberPagerState
import androidx.compose.ui.Modifier
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.StyleLearningStage
import uniffi.mailcal_bindings.WritingStyleChangeKind
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleResult

/**
 * The host, while the core draws a learning flow or [revealing] names a style whose reveal a row
 * opened. [onRevealLeft] is told when that row's reveal is left; [onOpenEndpoint] opens the own
 * endpoint's form, a remedy the client opens.
 */
@Composable
internal fun WritingStyleFlow(
    ui: WritingStyleUi,
    revealing: String?,
    onRevealLeft: () -> Unit,
    onOpenEndpoint: () -> Unit,
) {
    val ctx = LocalContext.current
    val learning = ui.learning
    val learned = (learning?.stage as? StyleLearningStage.Learned)?.style
    val revealId = learned ?: revealing.takeIf { learning == null }
    if (learning == null && revealId == null) return
    val reveal = revealId?.let { id -> remember(id, ui.version) { ui.reveal(id) } }
    if (learning == null && reveal == null) {
        // Forgotten meanwhile: there is nothing left to show.
        LaunchedEffect(revealId) { onRevealLeft() }
        return
    }
    val pager = key(revealId) { rememberPagerState { reveal?.pages?.size ?: 1 } }
    val fields = key(revealId) { remember { RevealFields(reveal?.name.orEmpty(), reveal?.notes.orEmpty()) } }
    val serials = key(revealId) { remember { OutcomeSerials(ui.page?.outcome?.serial) } }
    val scope = rememberCoroutineScope()
    val fromRow = learning == null

    // What leaving this reveal by Save came to: made, or not written, closes it; refused says why.
    val outcome = ui.page?.outcome
    LaunchedEffect(outcome) {
        if (outcome == null || outcome.style != revealId || outcome.kind != WritingStyleChangeKind.SAVE_REVEAL) return@LaunchedEffect
        if (!serials.fresh(outcome.serial)) return@LaunchedEffect
        when (val result = outcome.result) {
            is WritingStyleResult.Refused -> fields.refused = result.reason
            WritingStyleResult.Done, WritingStyleResult.NotWritten -> if (fromRow) onRevealLeft()
        }
    }

    val remedy = { chosen: AiRemedy ->
        when (remedyAction(chosen)) {
            RemedyAction.SIGN_IN -> ui.onSignIn()
            RemedyAction.OPEN_ENDPOINT -> {
                ui.learn(StyleLearningIntent.Close)
                onOpenEndpoint()
            }
            RemedyAction.MOVE_FLOW, RemedyAction.LEARN, RemedyAction.CHOOSE_STYLE -> ui.learn(StyleLearningIntent.Remedy)
        }
    }
    val chrome = if (reveal != null) {
        revealChrome(
            ctx = ctx,
            reveal = reveal,
            current = pager.currentPage,
            name = fields.name,
            nameRefusal = ui.nameRefusal,
            onPage = { target -> scope.launch { pager.animateScrollToPage(target) } },
            onSave = { ui.onChange(WritingStyleIntent.SaveReveal(revealId, fields.name, fields.notes)) },
            onClose = {
                if (fromRow) {
                    ui.onChange(WritingStyleIntent.CloseReveal(revealId))
                    onRevealLeft()
                } else {
                    ui.learn(StyleLearningIntent.Close)
                }
            },
        )
    } else {
        learningChrome(ctx, checkNotNull(learning), ui::learn, remedy)
    }
    // What is on screen, keyed by its kind so a stage's own updates (a run's count) redraw in place,
    // and by how deep into the flow it stands, which says whether it arrived going on or going back.
    val shown = if (reveal != null) {
        FlowScreen("reveal", Int.MAX_VALUE, null)
    } else {
        val current = checkNotNull(learning)
        FlowScreen(current.stage::class.simpleName.orEmpty(), current.trail.size, current.stage)
    }
    val reduced = rememberReducedMotion()
    FlowHost(chrome) { padding ->
        AnimatedContent(
            targetState = shown,
            transitionSpec = { flowTransition(onward = targetState.depth >= initialState.depth, reduced = reduced) },
            contentKey = { it.kind },
            label = "flow",
        ) { screen ->
            val stage = screen.stage
            if (stage == null) {
                reveal?.let { RevealPages(it, pager, fields, ui.nameRefusal, ui.zone, padding) }
            } else {
                Box(modifier = Modifier.fillMaxSize().padding(padding)) {
                    LearningStageView(stage, ui.zone, ui::learn)
                }
            }
        }
    }
}

// A stage or the reveal, as the flow's transition sees it.
private data class FlowScreen(val kind: String, val depth: Int, val stage: StyleLearningStage?)

// Material's shared axis: onward slides in from the end, back from the start; nothing moves under
// reduced motion.
private fun flowTransition(onward: Boolean, reduced: Boolean): ContentTransform {
    if (reduced) return EnterTransition.None togetherWith ExitTransition.None
    val sign = if (onward) 1 else -1
    val enter = slideInHorizontally(tween(300, easing = SETTLE)) { sign * it / 5 } + fadeIn(tween(240, 60))
    val exit = slideOutHorizontally(tween(300, easing = SETTLE)) { -sign * it / 5 } + fadeOut(tween(120))
    return enter togetherWith exit
}
