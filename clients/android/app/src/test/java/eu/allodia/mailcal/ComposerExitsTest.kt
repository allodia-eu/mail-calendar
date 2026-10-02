// The composer's ways out, without a composer on screen (docs/drafts.md, "Leaving a composer").
//
// ComposerDraftTest drives the real composer, whose WebView answers nothing in this suite, so the
// branches that turn on the BODY (the editor's document against the seed it opened with) are
// covered here, over a stand-in for the editor.
package eu.allodia.mailcal

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.mailcal_bindings.DraftStatus
import uniffi.mailcal_bindings.Recipients

class ComposerExitsTest {
    /** One composer's state and everything its exits did. */
    private class Harness(
        val headersEdited: Boolean = false,
        val document: String? = SEED,
        val seed: String? = SEED,
        val stored: Boolean = false,
        keepsDrafts: Boolean = true,
    ) {
        val savedAndClosed = mutableListOf<String>()
        val discarded = mutableListOf<String>()
        var asked = false
        var left = false
        var dismissed = false

        val exits = ComposerExits(
            drafts = ComposerDrafts(
                save = { _, _ -> },
                saveAndClose = { _, content -> savedAndClosed += content.documentJson },
                discard = { discarded += it },
                close = { },
                status = { DraftStatus.IDLE },
                isStored = { stored },
                version = 0,
                idleSeconds = 30u,
            ).takeIf { keepsDrafts },
            composition = "c-1",
            headersEdited = { headersEdited },
            readDocument = { answer -> answer(document) },
            seedDocument = { seed },
            contentOf = { json ->
                ComposerSubmission(null, Recipients("", "", ""), "", json, emptyList(), "c-1")
            },
            ask = { asked = true },
            left = { left = true },
            dismiss = { dismissed = true },
        )
    }

    @Test
    fun leaving_a_composer_whose_body_was_written_in_saves_it_and_closes() {
        val harness = Harness(document = WRITTEN)
        harness.exits.leave()
        assertEquals(listOf(WRITTEN), harness.savedAndClosed)
        assertFalse("leaving never asks", harness.asked)
        assertTrue(harness.left && harness.dismissed)
    }

    @Test
    fun leaving_a_reply_that_holds_only_its_quote_saves_nothing() {
        // The seed is the quote; a body still equal to it was not written in.
        val harness = Harness(document = SEED)
        harness.exits.leave()
        assertTrue(harness.savedAndClosed.isEmpty())
        assertFalse("disposal forgets the composition", harness.left)
        assertTrue(harness.dismissed)
    }

    @Test
    fun leaving_after_a_header_edit_saves_the_document_the_editor_holds() {
        val harness = Harness(headersEdited = true, document = SEED)
        harness.exits.leave()
        assertEquals(listOf(SEED), harness.savedAndClosed)
    }

    @Test
    fun leaving_with_an_unreadable_editor_asks_rather_than_losing_the_headers() {
        val harness = Harness(headersEdited = true, document = null)
        harness.exits.leave()
        assertTrue(harness.asked)
        assertTrue(harness.savedAndClosed.isEmpty())
        assertFalse(harness.dismissed)
    }

    @Test
    fun a_composer_keeping_no_drafts_asks_before_leaving_written_work() {
        val harness = Harness(document = WRITTEN, keepsDrafts = false)
        harness.exits.leave()
        assertTrue(harness.asked)
        assertFalse(harness.dismissed)
    }

    @Test
    fun discard_asks_when_the_body_was_written_in() {
        val harness = Harness(document = WRITTEN)
        harness.exits.discard()
        assertTrue(harness.asked)
        assertTrue("nothing goes until the user answers", harness.discarded.isEmpty())
    }

    @Test
    fun discard_asks_when_a_copy_is_in_drafts() {
        val harness = Harness(stored = true)
        harness.exits.discard()
        assertTrue(harness.asked)
    }

    @Test
    fun discard_with_nothing_to_lose_removes_and_closes_at_once() {
        val harness = Harness()
        harness.exits.discard()
        assertFalse(harness.asked)
        assertEquals(listOf("c-1"), harness.discarded)
        assertTrue(harness.left && harness.dismissed)
    }

    @Test
    fun confirming_the_question_discards() {
        val harness = Harness(stored = true)
        harness.exits.confirmDiscard()
        assertEquals(listOf("c-1"), harness.discarded)
        assertTrue(harness.left && harness.dismissed)
    }

    private companion object {
        const val SEED = """{"blocks":["quote"]}"""
        const val WRITTEN = """{"blocks":["quote","written"]}"""
    }
}
