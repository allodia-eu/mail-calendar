// The WebView the reading page draws a message in (ReadingPage.kt). It is laid out at the full
// height of the message, so it has nothing to scroll vertically: the page moves it, and a vertical
// drag that starts on it is the page's, which takes it over as a ViewGroup intercepts a child's.
// A tap, a link and a sideways drag across a zoomed message stay the WebView's.
//
// A pinch is the WebView's alone (docs/reading-zoom.md): with a second finger down it asks the page
// to keep out until the gesture ends, or a pinch that also moves vertically would scroll the page
// and cancel the zoom.
package eu.allodia.mailcal

import android.annotation.SuppressLint
import android.content.Context
import android.view.MotionEvent
import android.webkit.WebView

internal class ReadingWebView(context: Context) : WebView(context) {
    init {
        // Nothing scrolls inside it, so a scroll bar of its own would never move.
        isVerticalScrollBarEnabled = false
    }

    @SuppressLint("ClickableViewAccessibility") // Only asks the page to keep out; the WebView handles the touch.
    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (event.actionMasked == MotionEvent.ACTION_POINTER_DOWN) {
            parent?.requestDisallowInterceptTouchEvent(true)
        }
        return super.onTouchEvent(event)
    }
}
