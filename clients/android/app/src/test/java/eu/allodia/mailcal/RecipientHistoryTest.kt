// A recipient field's history, driven directly: what one step is, and the chords that move it.
package eu.allodia.mailcal

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RecipientHistoryTest {

    /** Types [text] one character at a time, as the input reports it. */
    private fun RecipientHistory.type(text: String) {
        for (letter in text) record(current + letter, typed = true)
    }

    @Test
    fun `one address typed is one step`() {
        val history = RecipientHistory("")
        history.type("bob@example.test")
        assertEquals("", history.undo())
    }

    @Test
    fun `finishing an address is a step of its own`() {
        val history = RecipientHistory("")
        history.type("bob@example.test,")
        history.type(" carol")
        assertEquals("bob@example.test,", history.undo())
        assertEquals("bob@example.test", history.undo())
    }

    @Test
    fun `a removed recipient comes back and goes again`() {
        val history = RecipientHistory("bob@example.test, carol@example.test, ")
        history.record("carol@example.test, ", typed = false)
        assertEquals("bob@example.test, carol@example.test, ", history.undo())
        assertEquals("carol@example.test, ", history.redo())
    }

    @Test
    fun `a change after an undo leaves nothing to redo`() {
        val history = RecipientHistory("")
        history.record("bob@example.test, ", typed = false)
        history.undo()
        history.record("carol@example.test, ", typed = false)
        assertNull(history.redo())
    }

    @Test
    fun `typing after an undo starts a new step`() {
        val history = RecipientHistory("")
        history.type("bo")
        history.undo()
        history.type("al")
        assertEquals("", history.undo())
        assertNull(history.undo())
    }

    @Test
    fun `a value from outside the field is where its history starts`() {
        val history = RecipientHistory("")
        history.record("bob@example.test, ", typed = false)
        history.restart("ada@example.test, ")
        assertNull(history.undo())
        assertEquals("ada@example.test, ", history.current)
    }

    @Test
    fun `the chords are the platform's own, read by key code`() {
        assertEquals(HistoryStep.Undo, historyStep(KeyEvent.KEYCODE_Z, ctrl = true, shift = false, alt = false))
        assertEquals(HistoryStep.Redo, historyStep(KeyEvent.KEYCODE_Z, ctrl = true, shift = true, alt = false))
        assertEquals(HistoryStep.Redo, historyStep(KeyEvent.KEYCODE_Y, ctrl = true, shift = false, alt = false))
        assertNull(historyStep(KeyEvent.KEYCODE_Y, ctrl = true, shift = true, alt = false))
        assertNull(historyStep(KeyEvent.KEYCODE_Z, ctrl = false, shift = false, alt = false))
        assertNull(historyStep(KeyEvent.KEYCODE_Z, ctrl = true, shift = false, alt = true))
        assertNull(historyStep(KeyEvent.KEYCODE_V, ctrl = true, shift = false, alt = false))
    }
}
