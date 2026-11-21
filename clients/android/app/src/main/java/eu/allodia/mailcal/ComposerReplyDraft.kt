// Draft a reply in the composer (docs/ai.md rules 17 to 24): the core's record for this
// composition, read again on the REPLY_DRAFT and WRITING_STYLE signals and whenever From changes,
// and what each press sends. Whether the control is held, whether to ask before replacing or at
// Send, and every tick are the record's; this keeps only which question or sheet is open and the
// line being typed.
package eu.allodia.mailcal

import android.webkit.WebView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.ComposerReplyDraft
import uniffi.mailcal_bindings.ReplyDraftIntent

/** What a composer asks of the core for Draft a reply. */
internal class ComposerReplyDraftUi(
    // `composerReplyDraft(composition, from)`.
    val read: (composition: String, from: String?) -> ComposerReplyDraft?,
    // `composerReplyDraftReport`, handed every message the editor posts.
    val report: (composition: String, message: String) -> Unit,
    val onChange: (ReplyDraftIntent) -> Unit,
    // A remedy the client opens. The composer stays open under it (docs/ai.md rule 27).
    val onRemedy: (remedy: AiRemedy) -> Unit,
    // Grows with each REPLY_DRAFT and WRITING_STYLE signal, so the record is read again.
    val version: Int,
    // The catalog locale the app is shown in, which the summary and the checklist are written in.
    val language: String,
)

/** One composer's Draft a reply: the record as last read, and which of its questions is open. */
internal class ReplyDraftHost(
    private val ui: ComposerReplyDraftUi,
    val composition: String,
    record: ComposerReplyDraft,
) {
    var record by mutableStateOf(record)
    var from by mutableStateOf<String?>(null)
    var sheetOpen by mutableStateOf(false)
    var line by mutableStateOf("")
    var askingReplace by mutableStateOf(false)
    var askingSend by mutableStateOf(false)

    // The serial of the draft the editor was last handed, so each is handed once.
    var handed: ULong? = null

    /** Draft a reply: opens the instruction's input holding the line the last draft was asked with. */
    fun open() {
        line = record.instruction
        sheetOpen = true
    }

    /** Draft, or the keyboard's action in the line: asks first while the record says to. */
    fun pressDraft() {
        if (record.askBeforeReplacing) askingReplace = true else draft(replace = false)
    }

    /** The answer to the question before a draft replaces what was written. */
    fun answerReplace(replace: Boolean) {
        askingReplace = false
        if (replace) draft(replace = true)
    }

    private fun draft(replace: Boolean) {
        sheetOpen = false
        ui.onChange(ReplyDraftIntent.Draft(composition, from, line, ui.language, replace))
    }

    fun stop() = ui.onChange(ReplyDraftIntent.Stop(composition))

    fun answerArrived(replace: Boolean) = ui.onChange(ReplyDraftIntent.AnswerArrived(composition, replace))

    fun tick(index: UInt, done: Boolean) = ui.onChange(ReplyDraftIntent.Tick(composition, index, done))

    /** Send: asks about the open items first while the record says it is due, otherwise [submit]s. */
    fun send(submit: () -> Unit) {
        if (record.sendQuestion != null) askingSend = true else submit()
    }

    /** Send anyway or Keep editing: either way the question is not asked again; Send anyway sends. */
    fun answerSend(sendAnyway: Boolean, submit: () -> Unit) {
        askingSend = false
        ui.onChange(ReplyDraftIntent.SendQuestionAnswered(composition))
        if (sendAnyway) submit()
    }

    /**
     * A remedy: Try again opens the input again; any other closes the input, the client's own, and
     * opens what it names over the composer, which stays as it is.
     */
    fun remedy(remedy: AiRemedy) {
        if (remedy == AiRemedy.TRY_AGAIN) {
            open()
        } else {
            sheetOpen = false
            ui.onRemedy(remedy)
        }
    }

    /** The script for the editor, once per serial: null when there is none new. */
    fun scriptToHand(): String? {
        val script = record.editorScript ?: return null
        if (script.serial == handed) return null
        handed = script.serial
        return script.script
    }
}

/**
 * Send in a composer: through [draft]'s question while the core offers Draft a reply there,
 * otherwise straight to [submit], since a question nobody draws must never hold a send.
 */
internal fun sendThrough(draft: ReplyDraftHost?, submit: () -> Unit) {
    if (draft == null) submit() else draft.send(submit)
}

/**
 * The composer's Draft a reply, or null while the core offers none for it: the feature not offered,
 * or the composition not a reply. [editor] is the WebView once its page has parsed, which the draft
 * is handed to.
 */
@Composable
internal fun rememberReplyDraft(
    ui: ComposerReplyDraftUi?,
    composition: String,
    from: String?,
    editor: WebView?,
): ReplyDraftHost? {
    ui ?: return null
    val record = remember(ui.version, from) { ui.read(composition, from) } ?: return null
    val host = remember { ReplyDraftHost(ui, composition, record) }
    SideEffect {
        host.record = record
        host.from = from
    }
    LaunchedEffect(record.editorScript?.serial, editor) {
        val target = editor ?: return@LaunchedEffect
        host.record = record
        host.scriptToHand()?.let { target.evaluateJavascript(it, null) }
    }
    return host
}
