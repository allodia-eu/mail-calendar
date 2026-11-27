// The WebView the reading page draws a message in, sharing its vertical drags with the page
// around it (ReadingPage.kt), so the header and the message scroll as one.
//
// A WebView scrolls itself and tells no parent: it implements none of Android's nested scrolling,
// so inside Compose the header above it would never move. This one offers every one-finger
// vertical drag to its parent first, which Compose's AndroidView hands to the page's nested scroll,
// and offers what is left once it is at its top. A pinch, a second finger down, is the WebView's
// alone (docs/reading-zoom.md).
//
// The distance is read on the screen (`rawY`), not in the view: the page moves the view under the
// finger as the header goes, which would otherwise read as the finger moving less than it did.
// The view moving with the header is also what keeps the WebView from scrolling the same distance
// twice: the finger stays where it was on the view.
package eu.allodia.mailcal

import android.annotation.SuppressLint
import android.content.Context
import android.view.MotionEvent
import android.webkit.WebView
import androidx.core.view.NestedScrollingChild3
import androidx.core.view.NestedScrollingChildHelper
import androidx.core.view.ViewCompat
import kotlin.math.roundToInt

internal class ReadingWebView(context: Context) : WebView(context), NestedScrollingChild3 {
    private val nested = NestedScrollingChildHelper(this)
    private var lastRawY = 0f
    private val consumed = IntArray(2)
    private val offsetInWindow = IntArray(2)

    init {
        // Both flags: the dispatches below read the helper's, and Compose's AndroidView forwards a
        // child's nested scroll only while the View's own says it is on.
        nested.isNestedScrollingEnabled = true
        isNestedScrollingEnabled = true
    }

    @SuppressLint("ClickableViewAccessibility") // A drag is shared, never handled here: a tap still reaches the WebView.
    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                lastRawY = event.rawY
                startNestedScroll(ViewCompat.SCROLL_AXIS_VERTICAL, ViewCompat.TYPE_TOUCH)
            }
            MotionEvent.ACTION_MOVE -> if (event.pointerCount == 1) shareDrag(event.rawY)
            // A second finger is a pinch: the page leaves it alone until the gesture ends.
            MotionEvent.ACTION_POINTER_DOWN,
            MotionEvent.ACTION_UP,
            MotionEvent.ACTION_CANCEL,
            -> stopNestedScroll(ViewCompat.TYPE_TOUCH)
        }
        return super.onTouchEvent(event)
    }

    private fun shareDrag(rawY: Float) {
        // Positive when the finger moves up, the View sign.
        val dy = (lastRawY - rawY).roundToInt()
        if (dy == 0) return
        lastRawY = rawY
        if (!hasNestedScrollingParent(ViewCompat.TYPE_TOUCH)) return
        consumed.fill(0)
        dispatchNestedPreScroll(0, dy, consumed, offsetInWindow, ViewCompat.TYPE_TOUCH)
        val left = dy - consumed[1]
        // A drag down with the message already at its top is the page's: it brings the header back.
        if (left < 0 && !canScrollVertically(-1)) {
            consumed.fill(0)
            dispatchNestedScroll(0, 0, 0, left, offsetInWindow, ViewCompat.TYPE_TOUCH, consumed)
        }
    }

    override fun startNestedScroll(axes: Int): Boolean = nested.startNestedScroll(axes)

    override fun startNestedScroll(axes: Int, type: Int): Boolean = nested.startNestedScroll(axes, type)

    override fun stopNestedScroll() = nested.stopNestedScroll()

    override fun stopNestedScroll(type: Int) = nested.stopNestedScroll(type)

    override fun hasNestedScrollingParent(): Boolean = nested.hasNestedScrollingParent()

    override fun hasNestedScrollingParent(type: Int): Boolean = nested.hasNestedScrollingParent(type)

    override fun dispatchNestedScroll(
        dxConsumed: Int,
        dyConsumed: Int,
        dxUnconsumed: Int,
        dyUnconsumed: Int,
        offsetInWindow: IntArray?,
    ): Boolean = nested.dispatchNestedScroll(dxConsumed, dyConsumed, dxUnconsumed, dyUnconsumed, offsetInWindow)

    override fun dispatchNestedScroll(
        dxConsumed: Int,
        dyConsumed: Int,
        dxUnconsumed: Int,
        dyUnconsumed: Int,
        offsetInWindow: IntArray?,
        type: Int,
    ): Boolean = nested.dispatchNestedScroll(dxConsumed, dyConsumed, dxUnconsumed, dyUnconsumed, offsetInWindow, type)

    override fun dispatchNestedScroll(
        dxConsumed: Int,
        dyConsumed: Int,
        dxUnconsumed: Int,
        dyUnconsumed: Int,
        offsetInWindow: IntArray?,
        type: Int,
        consumed: IntArray,
    ) = nested.dispatchNestedScroll(dxConsumed, dyConsumed, dxUnconsumed, dyUnconsumed, offsetInWindow, type, consumed)

    override fun dispatchNestedPreScroll(dx: Int, dy: Int, consumed: IntArray?, offsetInWindow: IntArray?): Boolean =
        nested.dispatchNestedPreScroll(dx, dy, consumed, offsetInWindow)

    override fun dispatchNestedPreScroll(
        dx: Int,
        dy: Int,
        consumed: IntArray?,
        offsetInWindow: IntArray?,
        type: Int,
    ): Boolean = nested.dispatchNestedPreScroll(dx, dy, consumed, offsetInWindow, type)

    override fun dispatchNestedFling(velocityX: Float, velocityY: Float, consumed: Boolean): Boolean =
        nested.dispatchNestedFling(velocityX, velocityY, consumed)

    override fun dispatchNestedPreFling(velocityX: Float, velocityY: Float): Boolean =
        nested.dispatchNestedPreFling(velocityX, velocityY)
}
