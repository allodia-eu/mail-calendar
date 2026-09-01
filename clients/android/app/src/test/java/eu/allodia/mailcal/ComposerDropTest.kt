// What the composer does with a dropped file, before any of it reaches the editor.
//
// A drop is the one place where two different things can happen to one file, so the sorting is
// pulled out of the composable and held here: a picture raises the "show it, or send it?" question
// and everything else is simply attached. Getting it wrong is invisible at runtime, the file just
// lands in the wrong half of the message.
package eu.allodia.mailcal

import android.content.ClipDescription
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ComposerDropTest {
    private fun staged(name: String, mediaType: String) = PickedComposerFile(
        id = name,
        path = "/cache/$name",
        fileName = name,
        mediaType = mediaType,
    )

    @Test
    fun `only the pictures in a drop raise the question`() {
        val sorted = sortDroppedFiles(
            listOf(
                staged("screenshot.png", "image/png"),
                staged("report.pdf", "application/pdf"),
                staged("holiday.jpg", "image/jpeg"),
            ),
        )

        assertEquals(
            listOf("screenshot.png", "holiday.jpg"),
            sorted.pictures.map { it.fileName },
        )
        assertEquals(listOf("report.pdf"), sorted.attach.map { it.fileName })
    }

    @Test
    fun `a drop of only files is attached without asking anything`() {
        val sorted = sortDroppedFiles(listOf(staged("notes.txt", "text/plain")))

        assertTrue(sorted.pictures.isEmpty())
        assertEquals(1, sorted.attach.size)
        assertEquals(DroppedComposerFiles(), sortDroppedFiles(emptyList()))
    }

    @Test
    fun `dragged text is left to the editor rather than attached`() {
        // Text dragged into the body is body content: turning it into a file named after a
        // snippet is not what anyone means by dropping a selection into a message.
        assertFalse(acceptsComposerDrop(listOf(ClipDescription.MIMETYPE_TEXT_PLAIN)))
        assertFalse(
            acceptsComposerDrop(
                listOf(ClipDescription.MIMETYPE_TEXT_PLAIN, ClipDescription.MIMETYPE_TEXT_HTML),
            ),
        )
        assertFalse(acceptsComposerDrop(emptyList()))

        assertTrue(acceptsComposerDrop(listOf("application/pdf")))
        assertTrue(acceptsComposerDrop(listOf("image/png")))
        // A file dragged from a document provider commonly describes itself as both.
        assertTrue(
            acceptsComposerDrop(listOf(ClipDescription.MIMETYPE_TEXT_PLAIN, "image/png")),
        )
    }
}
