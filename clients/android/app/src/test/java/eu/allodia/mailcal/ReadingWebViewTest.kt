// The message's WebView on the reading page (ReadingWebView.kt, docs/reading-actions.md): it has
// nothing of its own to scroll, so the page moves it, and a pinch is kept from the page.
//
// The touches are handed to the view directly inside a parent that records what it is asked: a
// Compose test cannot drive a WebView through AndroidView here, and the parent Compose puts around
// one hears the same request a ViewGroup does.
package eu.allodia.mailcal

import android.content.Context
import android.os.SystemClock
import android.view.MotionEvent
import android.widget.FrameLayout
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ReadingWebViewTest {
    private val ctx: Context = ApplicationProvider.getApplicationContext()

    private class Page(ctx: Context) : FrameLayout(ctx) {
        val keepOut = mutableListOf<Boolean>()

        override fun requestDisallowInterceptTouchEvent(disallowIntercept: Boolean) {
            keepOut += disallowIntercept
            super.requestDisallowInterceptTouchEvent(disallowIntercept)
        }
    }

    private fun touch(view: ReadingWebView, action: Int, pointers: Int) {
        val now = SystemClock.uptimeMillis()
        val props = Array(pointers) { MotionEvent.PointerProperties().apply { id = it } }
        val coords = Array(pointers) { MotionEvent.PointerCoords().apply { x = 100f + it * 200; y = 400f } }
        val event = MotionEvent.obtain(now, now, action, pointers, props, coords, 0, 0, 1f, 1f, 0, 0, 0, 0)
        view.onTouchEvent(event)
        event.recycle()
    }

    @Test
    fun aSecondFingerKeepsThePageOutOfThePinch() {
        val page = Page(ctx)
        val web = ReadingWebView(ctx)
        page.addView(web)
        touch(web, MotionEvent.ACTION_DOWN, pointers = 1)
        touch(web, MotionEvent.ACTION_MOVE, pointers = 1)
        assertEquals(emptyList<Boolean>(), page.keepOut)
        touch(web, MotionEvent.ACTION_POINTER_DOWN or (1 shl MotionEvent.ACTION_POINTER_INDEX_SHIFT), pointers = 2)
        assertEquals(listOf(true), page.keepOut)
    }

    @Test
    fun itDrawsNoScrollBarOfItsOwn() {
        // The page scrolls, never the message, so a bar beside it would stand still.
        assertFalse(ReadingWebView(ctx).isVerticalScrollBarEnabled)
    }
}
