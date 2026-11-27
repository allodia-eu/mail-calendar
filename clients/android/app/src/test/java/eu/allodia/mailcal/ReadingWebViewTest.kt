// The message's WebView shares its drags with the reading page (ReadingWebView.kt,
// docs/reading-actions.md), so the header and the message scroll as one.
//
// The touches are handed to the view directly, inside a parent that records what it is offered:
// a Compose test cannot drive a WebView through AndroidView here, and the parent Compose puts
// around one asks exactly what this one does.
package eu.allodia.mailcal

import android.content.Context
import android.os.SystemClock
import android.view.MotionEvent
import android.view.View
import android.widget.FrameLayout
import androidx.core.view.NestedScrollingParent3
import androidx.core.view.ViewCompat
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingWebViewTest {
    private val ctx: Context = ApplicationProvider.getApplicationContext()

    /** A parent that takes up to [takes] of every drag up offered before the WebView scrolls. */
    private class Page(ctx: Context, private val takes: Int) : FrameLayout(ctx), NestedScrollingParent3 {
        val offered = mutableListOf<Int>()

        override fun onStartNestedScroll(child: View, target: View, axes: Int, type: Int) =
            axes and ViewCompat.SCROLL_AXIS_VERTICAL != 0

        override fun onNestedScrollAccepted(child: View, target: View, axes: Int, type: Int) = Unit

        override fun onStopNestedScroll(target: View, type: Int) = Unit

        override fun onNestedPreScroll(target: View, dx: Int, dy: Int, consumed: IntArray, type: Int) {
            offered += dy
            // Like the page: only a drag up moves the header out of the way beforehand.
            consumed[1] = if (dy > 0) minOf(takes, dy) else 0
        }

        override fun onNestedScroll(target: View, dxConsumed: Int, dyConsumed: Int, dxUnconsumed: Int, dyUnconsumed: Int, type: Int, consumed: IntArray) {
            offered += dyUnconsumed
            consumed[1] = dyUnconsumed
        }

        override fun onNestedScroll(target: View, dxConsumed: Int, dyConsumed: Int, dxUnconsumed: Int, dyUnconsumed: Int, type: Int) = Unit
    }

    private fun drag(view: View, vararg ys: Float, pointers: Int = 1) {
        val start = SystemClock.uptimeMillis()
        ys.forEachIndexed { index, y ->
            val action = if (index == 0) MotionEvent.ACTION_DOWN else MotionEvent.ACTION_MOVE
            val event = MotionEvent.obtain(start, start + index * 16L, action, 0f, y, 0)
            if (pointers > 1 && index > 0) {
                val props = Array(pointers) { MotionEvent.PointerProperties().apply { id = it } }
                val coords = Array(pointers) { MotionEvent.PointerCoords().apply { this.y = y + it * 100 } }
                val pinch = MotionEvent.obtain(start, start + index * 16L, action, pointers, props, coords, 0, 0, 1f, 1f, 0, 0, 0, 0)
                view.onTouchEvent(pinch)
                pinch.recycle()
            } else {
                view.onTouchEvent(event)
            }
            event.recycle()
        }
    }

    @Test
    fun itSaysItScrollsNestedWhereComposeAsks() {
        // Compose's AndroidView forwards a child's nested scroll only while the View itself says
        // so; with only the helper's flag set, every drag was dropped and the header never moved.
        assertTrue(ReadingWebView(ctx).isNestedScrollingEnabled)
    }

    @Test
    fun aDragUpIsOfferedToThePageFirst() {
        val page = Page(ctx, takes = 1000)
        val web = ReadingWebView(ctx)
        page.addView(web)
        drag(web, 500f, 450f, 400f)
        assertEquals(listOf(50, 50), page.offered)
    }

    @Test
    fun aDragDownAtTheMessagesTopBringsTheHeaderBack() {
        val page = Page(ctx, takes = 0)
        val web = ReadingWebView(ctx)
        page.addView(web)
        drag(web, 400f, 460f)
        // Offered first (the page takes nothing going down), then as what the message left over.
        assertEquals(listOf(-60, -60), page.offered)
    }

    @Test
    fun aPinchIsTheWebViewsAlone() {
        val page = Page(ctx, takes = 1000)
        val web = ReadingWebView(ctx)
        page.addView(web)
        drag(web, 500f, 450f, 400f, pointers = 2)
        assertEquals(emptyList<Int>(), page.offered)
    }
}
