// The composer's three ways of dealing with its draft: saving it, leaving, and discarding
// (docs/drafts.md, "Leaving a composer").
//
// A plain class over the composer's state rather than lambdas inside RichComposeScreen.kt, which
// is at the 500-line limit, and so the JVM suite can drive every branch without an editor.
package eu.allodia.mailcal

/** What leaving the composer does with what it holds. */
internal enum class ComposerExit {
    /** Nothing was written, so there is nothing to keep. */
    CLOSE,

    /** Store it in Drafts and close, in the one call that keeps the close behind the save. */
    SAVE_AND_CLOSE,

    /** A composer keeping no drafts has nowhere to put written work, so it asks first. */
    ASK,
}

/** The decision behind the ✕ and the system back. */
internal fun composerExit(written: Boolean, keepsDrafts: Boolean): ComposerExit = when {
    !written -> ComposerExit.CLOSE
    keepsDrafts -> ComposerExit.SAVE_AND_CLOSE
    else -> ComposerExit.ASK
}

/**
 * Whether Discard asks before it throws the draft away: when something was written, or when a copy
 * is in Drafts or queued for it. With neither there is nothing to lose, and asking would be noise.
 */
internal fun composerDiscardAsks(written: Boolean, stored: Boolean): Boolean = written || stored

/**
 * Saving, leaving and discarding, over the composer's own state.
 *
 * Every input is read when an action runs rather than when this is built, because the composer
 * rebuilds it on each recomposition and the fields move under it.
 */
internal class ComposerExits(
    private val drafts: ComposerDrafts?,
    private val composition: String,
    /** Whether a header field differs from what the composer opened with (`composerHeadersEdited`). */
    private val headersEdited: () -> Boolean,
    /**
     * Reads the editor's document and answers with it, or with null when there is no editor yet or
     * it cannot render one. Asynchronous, being a hop into the WebView.
     */
    private val readDocument: ((String?) -> Unit) -> Unit,
    /** The document the editor seeded with, or null until it has answered. */
    private val seedDocument: () -> String?,
    private val contentOf: (String) -> ComposerSubmission,
    /** Raises the "Discard draft?" question. */
    private val ask: () -> Unit,
    /** Records that the composition is finished with, so disposal does not close it again. */
    private val left: () -> Unit,
    private val dismiss: () -> Unit,
) {
    /**
     * Stores the draft now, whatever the idle timer is doing. Both triggers land here and the core
     * cannot tell them apart, which is deliberate: saving an unchanged draft reaches no server.
     *
     * A document the editor cannot render is dropped rather than shown. Saving is never something
     * the user waits for; a save the *server* refuses is reported through the hint.
     */
    fun save() {
        val store = drafts ?: return
        readDocument { document -> document?.let { store.save(composition, contentOf(it)) } }
    }

    /** The ✕ and the system back: written work goes to Drafts with no question asked. */
    fun leave() = readWork(needsDocument = drafts != null) { written, document ->
        when (composerExit(written, keepsDrafts = drafts != null && document != null)) {
            ComposerExit.CLOSE -> dismiss()
            ComposerExit.ASK -> ask()
            ComposerExit.SAVE_AND_CLOSE -> {
                document?.let { drafts?.saveAndClose(composition, contentOf(it)) }
                left()
                dismiss()
            }
        }
    }

    /** Discard: asks first whenever something would be lost, and otherwise closes at once. */
    fun discard() = readWork(needsDocument = false) { written, _ ->
        if (composerDiscardAsks(written, drafts?.isStored(composition) == true)) {
            ask()
        } else {
            confirmDiscard()
        }
    }

    /** What the question's Discard does, and Discard itself when there was nothing to ask. */
    fun confirmDiscard() {
        drafts?.discard(composition)
        left()
        dismiss()
    }

    /**
     * Answers whether the composer holds written work, with the document when `needsDocument` asks
     * for it, for the save that keeps it. The editor is read only when its answer matters: an
     * edited header already says the composer was written in, and only a seeded editor has a body
     * to compare.
     */
    private fun readWork(
        needsDocument: Boolean,
        answer: (written: Boolean, document: String?) -> Unit,
    ) {
        val headers = headersEdited()
        val seed = seedDocument()
        if ((headers && !needsDocument) || (!headers && seed == null)) {
            answer(headers, null)
            return
        }
        readDocument { document ->
            val bodyEdited = seed != null && document != null && document != seed
            answer(headers || bodyEdited, document)
        }
    }
}
