// How a swipe turns the page.
//
// Two numbers decide whether a calendar feels "unbreakable", and only one of them is the one people
// reach for. Measured off screen recordings of Samsung Calendar and ours on the same phone, driven
// by the same hand:
//
//                       page turn takes      peak speed
//     ours (before)     0.32 – 0.50 s        ~5 pages/s
//     Samsung           0.02 – 0.15 s        20 – 28 pages/s
//
// The threshold, how far you must drag before it commits, was NOT the difference. Ours was already
// a sixth of the screen, and at 20+ pages a second Samsung is committing on **velocity** anyway, so
// the finger's travel is irrelevant to it. The difference was the **settle**: theirs is over in a
// tenth of a second, ours took most of half a second. That is what makes rapid swipes feel like they
// are fighting the app, each new flick lands on a grid that is still gliding from the last one,
// while Samsung's has long since arrived.
package eu.allodia.mailcal

import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.gestures.TargetedFlingBehavior
import androidx.compose.foundation.pager.PagerDefaults
import androidx.compose.foundation.pager.PagerState
import androidx.compose.runtime.Composable

/**
 * How the page settles once it has committed.
 *
 * **This is the one that makes it feel unbreakable**, and it is the one that was wrong. Compose's
 * default is a soft spring (`StiffnessMediumLow`) that takes the best part of half a second to
 * arrive. Samsung's page is *there* in a tenth of one.
 *
 * Tuned by measurement, not by taste. `StiffnessMedium` (1500) got a page turn down to 0.23 s,
 * which was half of what it had been and still twice Samsung. This sits between `StiffnessMedium`
 * and `StiffnessHigh`, and lands the page in about a tenth of a second, their number.
 *
 * Critically damped (**no bounce**) is deliberate, and not a matter of taste either: a week that
 * springs past its own column and comes back is a grid whose day columns visibly overshoot the
 * headings above them. On a dense week that reads as a glitch, not as polish.
 */
private const val SNAP_STIFFNESS = 4000f

internal fun <T> snapSpec() = spring<T>(
    dampingRatio = Spring.DampingRatioNoBouncy,
    stiffness = SNAP_STIFFNESS,
)

/**
 * The fling behaviour the **month** pager uses.
 *
 * The time grid no longer has a pager to give this to: it owns its whole pointer stream and turns its
 * own weeks (see CalendarSurfaceGesture). It still turns them by exactly these two numbers:
 * [PAGE_TURN_THRESHOLD] and [snapSpec], so a month and a week page with the same hand.
 */
@Composable
internal fun calendarFlingBehavior(state: PagerState): TargetedFlingBehavior =
    PagerDefaults.flingBehavior(
        state = state,
        snapPositionalThreshold = PAGE_TURN_THRESHOLD,
        snapAnimationSpec = snapSpec(),
    )
