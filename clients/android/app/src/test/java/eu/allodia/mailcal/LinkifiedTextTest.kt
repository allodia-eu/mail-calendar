// Sender and event text with its addresses as links. The core finds the addresses; what is the
// client's to get right is that every run reaches the screen as the text it was, and that only the
// runs the core linked carry a link, whose tap goes to the gate rather than to the platform.
package eu.allodia.mailcal

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.LinkAnnotation
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.mailcal_bindings.LinkedText

class LinkifiedTextTest {
    private val runs = listOf(
        LinkedText(text = "Join ", link = null),
        LinkedText(text = "https://meet.example/abc", link = "https://meet.example/abc"),
        LinkedText(text = " **bold** <b>x</b> & co", link = null),
    )

    @Test
    fun `every run reaches the string as the text it was`() {
        val annotated = linkedAnnotatedString(runs, Color.Blue) {}
        assertEquals("Join https://meet.example/abc **bold** <b>x</b> & co", annotated.text)
    }

    @Test
    fun `only the linked run carries a link and a tap hands its target to the caller`() {
        val opened = mutableListOf<String>()
        val annotated = linkedAnnotatedString(runs, Color.Blue) { opened += it }
        val links = annotated.getLinkAnnotations(0, annotated.length)
        assertEquals(1, links.size)
        val link = links.single()
        assertEquals("https://meet.example/abc", annotated.text.substring(link.start, link.end))
        // `Clickable`, never `Url`: a `Url` would open through the platform, around the gate.
        val clickable = link.item as LinkAnnotation.Clickable
        clickable.linkInteractionListener!!.onClick(clickable)
        assertEquals(listOf("https://meet.example/abc"), opened)
    }
}
