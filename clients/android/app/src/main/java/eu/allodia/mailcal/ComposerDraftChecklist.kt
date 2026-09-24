// A drafted reply's card (docs/ai.md, "Summary and checklist" and "Where they show"): the summary,
// the checklist, what the person has done about it, and whether Send has asked. Held once per
// composer as a plain class, so its rules are tested without drawing the card.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.DraftTask
import uniffi.mailcal_bindings.DraftTaskKind

internal class DraftChecklist {
    data class Item(
        val id: Int,
        val kind: DraftTaskKind,
        // The placeholder exactly as the reply carries it for a fill-in item, otherwise the task.
        val text: String,
        val ticked: Boolean = false,
    ) {
        // What the row says.
        fun title(ctx: Context): String =
            if (kind == DraftTaskKind.FILL_IN) L10n.composer_task_fill_in(ctx, placeholder = text) else text
    }

    var summary by mutableStateOf("")
        private set

    var items by mutableStateOf(emptyList<Item>())
        private set

    // Which draft the card is for, so what follows the editor starts again with a later one.
    var draft by mutableIntStateOf(0)
        private set

    // Whether Send has asked about open items. Once per composer, whatever the answer, so a later
    // draft keeps it.
    var hasAsked by mutableStateOf(false)
        private set

    // The person's own collapse or expand, which a later draft keeps; null until they choose.
    var collapsedChoice by mutableStateOf<Boolean?>(null)

    private var attachmentsAtDraft = 0

    // Replaces the card with a draft's, in the core's order.
    fun show(summary: String, tasks: List<DraftTask>, attachments: Int) {
        this.summary = summary
        items = tasks.mapIndexed { index, task -> Item(index, task.kind, task.text) }
        attachmentsAtDraft = attachments
        draft++
    }

    val isEmpty: Boolean get() = summary.isEmpty() && items.isEmpty()

    // The placeholders the editor is asked about.
    val placeholders: List<String> get() = items.filter { it.kind == DraftTaskKind.FILL_IN }.map { it.text }

    // Whether a fill-in item is still open, which is what keeps the editor being asked.
    val awaitsPlaceholders: Boolean get() = items.any { it.kind == DraftTaskKind.FILL_IN && !it.ticked }

    // The draft while the editor is followed, null once no fill-in item is open; a fill-in the read
    // at Send opens again starts the following again.
    val following: Int? get() = draft.takeIf { awaitsPlaceholders }

    // A fill-in item is ticked exactly while its placeholder is gone from the reply, so one that
    // comes back, by an undo, opens again.
    fun placeholdersLeft(left: List<String>) {
        items = items.map { if (it.kind == DraftTaskKind.FILL_IN) it.copy(ticked = it.text !in left) else it }
    }

    // The person's tick. A fill-in item follows the reply alone.
    fun toggle(id: Int) {
        items = items.map { if (it.id == id && it.kind != DraftTaskKind.FILL_IN) it.copy(ticked = !it.ticked) else it }
    }

    // Ticks every attach item once as many files have been added since the draft as there are
    // attach items: which file answers which item cannot be told, so fewer files tick none.
    fun attachmentsChanged(count: Int) {
        val attach = items.count { it.kind == DraftTaskKind.ATTACH }
        if (attach == 0 || count - attachmentsAtDraft < attach) return
        items = items.map { if (it.kind == DraftTaskKind.ATTACH) it.copy(ticked = true) else it }
    }

    val openCount: Int get() = items.count { !it.ticked }

    // Whether Send asks before it sends. It never blocks: either answer ends the asking.
    val asksBeforeSend: Boolean get() = !hasAsked && openCount > 0

    fun sendAsked() {
        hasAsked = true
    }

    // Collapsed when the person collapsed it, and otherwise on a phone with more than three rows.
    fun isCollapsed(onPhone: Boolean): Boolean = collapsedChoice ?: (onPhone && items.size > 3)
}
