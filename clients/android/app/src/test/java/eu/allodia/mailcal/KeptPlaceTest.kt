// The reader's place across a scheme change (ReadingScreenBody.kt, docs/reading-dark-mode.md
// rule 9). The WebView reports the callbacks in an order this cannot drive, so the order is
// stated here: for an in-memory document `onPageFinished` comes first, then the new page's first
// frame scrolls it to the top, then `onPageCommitVisible`. Only the last can put the place back.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class KeptPlaceTest {
    @Test
    fun theSameMessageInAnotherSchemeGetsItsPlaceBackWhenThePageCommitsAndOnlyThen() {
        val kept = KeptPlace()
        kept.loading(currentScrollY = 3579, sameMessage = true)
        assertEquals(3579, kept.committed())
        assertNull("a place is put back once, not on every later commit", kept.committed())
    }

    @Test
    fun anotherMessageOpensAtItsTop() {
        val kept = KeptPlace()
        kept.loading(currentScrollY = 3579, sameMessage = false)
        assertNull(kept.committed())
    }

    @Test
    fun aLoadAfterAnUnspentPlaceDoesNotInheritIt() {
        val kept = KeptPlace()
        kept.loading(currentScrollY = 3579, sameMessage = true)
        kept.loading(currentScrollY = 0, sameMessage = false)
        assertNull(kept.committed())
    }
}
