// The reading page: one scroll for everything between the top app bar and the answers, which stay
// put (docs/reading-actions.md). The subject, the rest of the conversation, who sent it, its
// banners, files and invitation, and the message itself are one column, so a drag or a fling moves
// all of it at once and runs from the message's end to the subject without stopping.
//
// That holds because the message never scrolls by itself: its WebView is laid out at the full
// height of what it draws (ReadingScreenBody.kt), and the page is what moves it.
package eu.allodia.mailcal

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color

@Composable
internal fun ReadingPage(
    scroll: ScrollState,
    // The colour the message is drawn on, which runs to the foot of the page.
    page: Color,
    header: @Composable () -> Unit,
    body: @Composable () -> Unit,
    modifier: Modifier = Modifier,
) {
    BoxWithConstraints(modifier = modifier) {
        val viewport = maxHeight
        Column(modifier = Modifier.fillMaxSize().verticalScroll(scroll)) {
            header()
            // At least a screen tall, so a short message leaves no strip of another colour under
            // it and the header can still be scrolled away above it.
            Box(modifier = Modifier.fillMaxWidth().heightIn(min = viewport).background(page)) { body() }
        }
    }
}
