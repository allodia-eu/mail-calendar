// The composer's idle save counts an edit only against the editor as its seeds left it
// (docs/drafts.md, "When a save happens"). Measured against anything earlier, an untouched reply
// saves itself; against a later sample, a short reply typed straight after opening never does.
package eu.allodia.mailcal

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class EditorRevisionWatchTest {
    @Test
    fun theSeedsAreTheBaselineNotAnEdit() {
        val watch = EditorRevisionWatch(seeded = 4)
        assertFalse(watch.changed(4))
        assertFalse(watch.changed(4))
    }

    @Test
    fun typingBeforeTheFirstSampleIsAnEdit() {
        val watch = EditorRevisionWatch(seeded = 4)
        assertTrue(watch.changed(7))
        assertFalse(watch.changed(7))
        assertTrue(watch.changed(9))
    }

    @Test
    fun aCountThatCouldNotBeReadChangesNothing() {
        val watch = EditorRevisionWatch(seeded = 4)
        assertFalse(watch.changed(-1))
        assertFalse(watch.changed(4))
        assertTrue(watch.changed(5))
    }

    @Test
    fun aBaselineThatCouldNotBeReadIsTakenFromTheFirstSample() {
        val watch = EditorRevisionWatch(seeded = -1)
        assertFalse(watch.changed(6))
        assertTrue(watch.changed(8))
    }
}
