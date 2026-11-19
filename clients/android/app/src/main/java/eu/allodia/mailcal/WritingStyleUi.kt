// What Settings → Writing style and the own endpoint's form draw, and what they ask of the core
// (docs/ai.md rules 6 to 16). The records are read again on the WRITING_STYLE, AI_ENDPOINT and
// SETTINGS signals; nothing here is a copy the client keeps of its own.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.time.ZoneId
import uniffi.mailcal_bindings.AiEndpointForm
import uniffi.mailcal_bindings.AiEndpointGaps
import uniffi.mailcal_bindings.AiEndpointIntent
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleLearning
import uniffi.mailcal_bindings.WritingStylePage
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleReveal

/** The assistant's records as last read, held by the activity. */
internal class WritingStyleState {
    // Null while the feature is not offered: the category and the form are then absent.
    var page by mutableStateOf<WritingStylePage?>(null)
    var learning by mutableStateOf<WritingStyleLearning?>(null)
    var endpoint by mutableStateOf<AiEndpointForm?>(null)

    // Counts WRITING_STYLE signals, so an open reveal or detail reads its record again.
    var version by mutableIntStateOf(0)

    // Counts REPLY_DRAFT and WRITING_STYLE signals, so an open composer reads Draft a reply again.
    var draftVersion by mutableIntStateOf(0)
}

/** Settings → Writing style's records and the calls its screens make. */
internal class WritingStyleUi(
    val page: WritingStylePage?,
    val learning: WritingStyleLearning?,
    val version: Int,
    val reveal: (String) -> WritingStyleReveal?,
    val detail: (String) -> WritingStyleDetail?,
    // The core's `writingStyleNameRefusal`, passed in so the JVM suite, which never loads the
    // cdylib, can draw the reveal and the detail.
    val nameRefusal: (String) -> WritingStyleRefusal?,
    val onChange: (WritingStyleIntent) -> Unit,
    // A remedy the client opens: the Allodia sign-in.
    val onSignIn: () -> Unit,
    val zone: ZoneId = ZoneId.systemDefault(),
) {
    fun learn(intent: StyleLearningIntent) = onChange(WritingStyleIntent.Learning(intent))
}

/** Settings → Advanced's own endpoint form and the calls it makes. */
internal class AiEndpointUi(
    val form: AiEndpointForm?,
    // The core's `aiEndpointGap` of the fields as typed.
    val gap: (address: String, model: String, declared: JurisdictionClass?) -> AiEndpointGaps,
    val onChange: (AiEndpointIntent) -> Unit,
)

/**
 * Which outcomes a surface has already acted on: the one in force when it opened is not its own,
 * and each later one is acted on once, by its serial.
 */
internal class OutcomeSerials(opened: ULong?) {
    private var seen: ULong = opened ?: 0uL

    /** Whether [serial] is newer than any acted on, marking it acted on. */
    fun fresh(serial: ULong?): Boolean {
        if (serial == null || serial <= seen) return false
        seen = serial
        return true
    }
}

/** Whether no field of the endpoint's form names a gap, so Save may be pressed. */
internal fun AiEndpointGaps.none(): Boolean = address == null && !modelMissing && declaration == null
