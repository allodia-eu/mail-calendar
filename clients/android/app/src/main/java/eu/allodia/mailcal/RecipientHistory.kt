// A recipient field's undo and redo (`docs/undo.md` rule 9).
//
// The field's own, not the text input's. The input holds only the address being typed, and the
// field rewrites it whenever an address is finished, so the input's history has nothing left to
// take back once a pill appears. A step here is the whole field before a change: typing one
// address, finishing it, accepting a suggestion, removing a pill.
//
// A plain class, so the JVM suite drives it without composing a screen.
package eu.allodia.mailcal

import android.view.KeyEvent

/** What an undo chord asks for. */
internal enum class HistoryStep { Undo, Redo }

/**
 * Ctrl+Z is undo, Ctrl+Shift+Z and Ctrl+Y redo: the chords Android's own text fields answer. Read
 * by key code, as theirs are, so a layout with no Latin letter on the key still undoes.
 */
internal fun historyStep(keyCode: Int, ctrl: Boolean, shift: Boolean, alt: Boolean): HistoryStep? {
    if (!ctrl || alt) return null
    return when (keyCode) {
        KeyEvent.KEYCODE_Z -> if (shift) HistoryStep.Redo else HistoryStep.Undo
        KeyEvent.KEYCODE_Y -> if (shift) null else HistoryStep.Redo
        else -> null
    }
}

/** The values a field held before each change, and after each undo. */
internal class RecipientHistory(initial: String) {
    private val undone = ArrayDeque<String>()
    private val redone = ArrayDeque<String>()

    /** Whether the newest step is an address still being typed, which the next keystroke joins. */
    private var typing = false

    /** The value the field last held as far as this history knows. */
    var current: String = initial
        private set

    /**
     * Files a change to [after]. [typed] is a keystroke in the input, which joins the step before
     * it while the same address is being typed; finishing it is a step of its own.
     */
    fun record(after: String, typed: Boolean) {
        if (after == current) return
        val sameRecipients = committedRecipients(current) == committedRecipients(after)
        if (!(typed && typing && sameRecipients)) {
            undone.addLast(current)
            if (undone.size > MAX_STEPS) undone.removeFirst()
        }
        redone.clear()
        typing = typed && sameRecipients
        current = after
    }

    /** The value to go back to, if there is one. */
    fun undo(): String? = move(undone, redone)

    /** The value an undo left behind, if nothing has changed since. */
    fun redo(): String? = move(redone, undone)

    /** Forgets everything: the field was given [value] from outside it, a reply's recipients say. */
    fun restart(value: String) {
        undone.clear()
        redone.clear()
        typing = false
        current = value
    }

    private fun move(from: ArrayDeque<String>, to: ArrayDeque<String>): String? {
        val value = from.removeLastOrNull() ?: return null
        to.addLast(current)
        typing = false
        current = value
        return value
    }

    private companion object {
        /** The most steps kept, oldest dropped first. */
        const val MAX_STEPS = 100
    }
}
