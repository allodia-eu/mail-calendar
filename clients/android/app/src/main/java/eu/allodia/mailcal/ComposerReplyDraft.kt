// Draft a reply, in the composer (docs/ai.md, "Drafting a reply"): the core writes a draft in the
// person's style, and this puts it above the signature and the quote, where they edit it. Nothing
// on this path sends. The state is a plain class so the JVM suite can drive it without a WebView;
// the button and its dialogs are in ComposerReplyDraftViews.kt.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.DraftReply
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.WritingStyleFailure

// The message a reply answers.
internal data class ReplyTarget(val account: String, val key: String)

// What drafting needs from the core. Provided once, above every composer, rather than threaded
// through each list row and reading screen that can open one; absent means AI is not available,
// and then no composer offers to draft.
internal class ReplyDrafting(
    val route: AiRoute,
    // The style an account drafts in, or null when it has none.
    val styleFor: (account: String) -> String?,
    // Blocking; throws `WritingStyleFailure`.
    val draft: (account: String, key: String, from: String?, intent: String?) -> DraftReply,
    val background: Background = ThreadBackground,
)

internal val LocalReplyDrafting = compositionLocalOf<ReplyDrafting?> { null }

internal fun replyDrafting(app: MailcalApp, route: AiRoute, ctx: Context) = ReplyDrafting(
    route = route,
    styleFor = { app.resolveWritingStyle(it) },
    // The style and the language are the core's to choose: the From account's style, and the
    // language of the message being answered. The summary and checklist follow the app's language.
    draft = { account, key, from, intent ->
        app.draftReply(account, key, from, null, intent, null, catalogLocale(ctx))
    },
)

// Replies only: a forward and a new message have nothing to answer.
internal fun replyDraftOffered(mode: RichComposeMode): Boolean =
    mode == RichComposeMode.Reply || mode == RichComposeMode.ReplyAll

// The two editor seams a draft needs, behind an interface so the state below is testable.
internal interface DraftEditor {
    // Whether the person has written anything above the signature and the quote.
    fun leadHasText(answer: (Boolean) -> Unit)

    fun insert(text: String, draftId: String)
}

internal class ReplyDraftControl(
    private val target: ReplyTarget,
    private val drafting: ReplyDrafting,
    private val editor: DraftEditor,
) {
    var sheetOpen by mutableStateOf(false)
        private set

    // The one-line intent; a chip puts its words here.
    var intent by mutableStateOf("")

    var confirmingReplace by mutableStateOf(false)
        private set

    var busy by mutableStateOf(false)
        private set

    // Shown near the body from a draft with gaps until the next draft or a send.
    var checkBrackets by mutableStateOf(false)
        private set

    var failure by mutableStateOf<WritingStyleFailure?>(null)
        private set

    val route: AiRoute get() = drafting.route

    private var confirmingFrom: String? = null

    // A From account with no style has nothing to draft in; the control says so rather than
    // failing after a round trip.
    fun hasStyle(from: String?): Boolean = from != null && drafting.styleFor(from) != null

    fun open() {
        sheetOpen = true
    }

    fun dismiss() {
        sheetOpen = false
    }

    // Asks before a draft replaces what the person has written.
    fun create(from: String?) {
        sheetOpen = false
        editor.leadHasText { written ->
            if (written) {
                confirmingFrom = from
                confirmingReplace = true
            } else {
                draft(from)
            }
        }
    }

    fun replace() {
        confirmingReplace = false
        draft(confirmingFrom)
    }

    fun keep() {
        confirmingReplace = false
    }

    fun sending() {
        checkBrackets = false
    }

    private fun draft(from: String?) {
        busy = true
        failure = null
        checkBrackets = false
        // `from` only when the person moved the reply off the account that received the message.
        val sender = from?.takeIf { it != target.account }
        val said = intent.trim().ifEmpty { null }
        drafting.background.run({ drafting.draft(target.account, target.key, sender, said) }) { result ->
            busy = false
            result.onSuccess {
                editor.insert(it.text, it.draftId)
                checkBrackets = it.gaps.isNotEmpty()
            }.onFailure {
                failure = it.asWritingStyleFailure()
            }
        }
    }
}
