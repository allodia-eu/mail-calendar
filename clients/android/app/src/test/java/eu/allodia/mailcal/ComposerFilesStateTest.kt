// A composer's files as the core last answered them: each change redraws from the core's answer,
// and a refused one leaves the screen as it was.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.ComposerFileTile
import uniffi.mailcal_bindings.ComposerFiles

class ComposerFilesStateTest {
    private fun picked(name: String) = ComposerFileAttachment("/picked/$name", name, "application/pdf")

    private fun names(state: ComposerFilesState) = state.files.tiles.map { it.fileName }

    @Test
    fun attach_runs_from_the_start_of_the_copies_until_their_files_are_in() {
        val state = ComposerFilesState(FakeComposerFileList(emptyList()))
        state.begin()
        assertTrue(state.files.attach.running)
        assertFalse(state.files.attach.enabled)
        state.finish(listOf(picked("film.mov")))
        assertEquals(listOf("film.mov"), names(state))
        assertEquals(FREE, state.files.attach)
    }

    @Test
    fun each_change_redraws_from_the_core_answer() {
        val list = FakeComposerFileList(listOf(ComposerFileTile("file-0", "a.pdf", null, FREE)))
        val state = ComposerFilesState(list)
        state.add(emptyList())
        state.add(listOf(picked("b.pdf")))
        assertEquals(listOf("a.pdf", "b.pdf"), names(state))
        state.remove("file-0")
        assertEquals(listOf("b.pdf"), names(state))
    }

    @Test
    fun a_refused_change_leaves_the_files_as_they_were() {
        val refusing = object : ComposerFileList {
            override fun current(): ComposerFiles? = null

            override fun add(files: List<ComposerFileAttachment>): ComposerFiles? = null

            override fun begin(): ComposerFiles? = null

            override fun finish(files: List<ComposerFileAttachment>): ComposerFiles? = null

            override fun remove(id: String): ComposerFiles? = null
        }
        val state = ComposerFilesState(refusing)
        state.begin()
        state.add(listOf(picked("a.pdf")))
        assertEquals(NO_COMPOSER_FILES, state.files)
        assertEquals(NO_COMPOSER_FILES, ComposerFilesState(null).files)
    }
}
