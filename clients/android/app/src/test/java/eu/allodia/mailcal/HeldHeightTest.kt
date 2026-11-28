// The reader's place when a message is drawn again (ReadingScreenBody.kt): in another scheme
// (docs/reading-dark-mode.md rule 9), or in a view built anew after a visit to another tab. The
// page keeps its scroll by itself as long as the message does not shrink under it, so what is kept
// is the message's height, until the new document has been laid out that tall again.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Test

class HeldHeightTest {
    @Test
    fun theSameMessageKeepsItsHeightUntilItIsThatTallAgain() {
        val held = HeldHeight()
        held.loading(currentHeight = 3579, sameMessage = true)
        assertEquals(3579, held.px)
        // A new document starts at no height at all; the page must not follow it down.
        held.laidOut(0)
        assertEquals(3579, held.px)
        held.laidOut(3579)
        assertEquals(0, held.px)
    }

    @Test
    fun anotherMessageStartsFromNothing() {
        val held = HeldHeight()
        held.loading(currentHeight = 3579, sameMessage = false)
        assertEquals(0, held.px)
    }

    @Test
    fun aViewBuiltAnewIsHeldFromItsFirstLayout() {
        // Back from another tab, before any document has loaded: the page is laid out once before
        // the view loads, and would cut the reader's place short at that height.
        assertEquals(3579, HeldHeight(initial = 3579).px)
    }

    @Test
    fun aZoomLetsGo() {
        // A pinch makes a height of its own, which a kept one would only pad with empty page.
        val held = HeldHeight()
        held.loading(currentHeight = 3579, sameMessage = true)
        held.release()
        assertEquals(0, held.px)
    }
}
