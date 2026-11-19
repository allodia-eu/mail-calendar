// Records the core would answer for the writing assistant's surfaces (docs/ai.md), built here
// because this suite never loads the cdylib. The shapes are the core's; what each holds in a given
// state is pinned by the core's own tests.
package eu.allodia.mailcal

import java.time.ZoneId
import uniffi.mailcal_bindings.AiEndpointForm
import uniffi.mailcal_bindings.AiEndpointGaps
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.StyleLearnRange
import uniffi.mailcal_bindings.StyleLearningAccount
import uniffi.mailcal_bindings.StyleLearningIntent
import uniffi.mailcal_bindings.StyleLearningStage
import uniffi.mailcal_bindings.StyleSentReach
import uniffi.mailcal_bindings.WritingStyleCard
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleEntry
import uniffi.mailcal_bindings.WritingStyleFacts
import uniffi.mailcal_bindings.WritingStyleFrequency
import uniffi.mailcal_bindings.WritingStyleHabit
import uniffi.mailcal_bindings.WritingStyleHabits
import uniffi.mailcal_bindings.WritingStyleIntent
import uniffi.mailcal_bindings.WritingStyleLanguage
import uniffi.mailcal_bindings.WritingStyleLearning
import uniffi.mailcal_bindings.WritingStyleLetter
import uniffi.mailcal_bindings.WritingStyleOpens
import uniffi.mailcal_bindings.WritingStylePage
import uniffi.mailcal_bindings.WritingStyleParagraph
import uniffi.mailcal_bindings.WritingStylePhrases
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleReveal
import uniffi.mailcal_bindings.WritingStyleRevealPage
import uniffi.mailcal_bindings.WritingStyleSlot
import uniffi.mailcal_bindings.WritingStyleSpan
import uniffi.mailcal_bindings.WritingStyleTopic

internal val STYLE_RUNNING = ControlState(enabled = false, running = true)

internal val UTC: ZoneId = ZoneId.of("UTC")

/** The core's name rule: a blank name is refused. */
internal fun fakeStyleNameRefusal(name: String): WritingStyleRefusal? =
    if (name.isBlank()) WritingStyleRefusal.BLANK_NAME else null

internal fun styleFacts(messages: Int = 42, languages: List<String> = listOf("en", "nl")) = WritingStyleFacts(
    messages = messages.toUInt(),
    learnedFrom = "alice@test.local",
    since = 1_700_000_000L,
    learnedAt = 1_760_000_000L,
    languages = languages,
)

internal fun styleEntry(id: String, name: String, opens: WritingStyleOpens = WritingStyleOpens.DETAIL, open: ControlState = FREE) =
    WritingStyleEntry(id = id, name = name, facts = styleFacts(), opens = opens, open = open)

internal fun styleSlot(account: String, selected: String? = null, picker: ControlState = FREE) =
    WritingStyleSlot(accountId = account, email = "$account@test.local", selected = selected, picker = picker)

internal fun stylePage(
    styles: List<WritingStyleEntry> = emptyList(),
    accounts: List<WritingStyleSlot> = emptyList(),
    learn: ControlState = FREE,
) = WritingStylePage(styles = styles, accounts = accounts, learn = learn, learnBlocked = null, run = null, outcome = null)

internal fun learning(
    stage: StyleLearningStage,
    back: ControlState? = null,
    next: ControlState? = null,
    learn: ControlState? = null,
    stop: ControlState? = null,
    remedy: ControlState? = null,
    close: ControlState = FREE,
) = WritingStyleLearning(stage = stage, back = back, next = next, learn = learn, stop = stop, remedy = remedy, close = close)

internal fun chooseAccount(selected: String? = null) = StyleLearningStage.ChooseAccount(
    accounts = listOf(StyleLearningAccount("alice", "alice@test.local"), StyleLearningAccount("bob", "bob@test.local")),
    selected = selected,
)

internal fun chooseRange(
    selected: StyleLearnRange? = StyleLearnRange.Everything,
    presets: List<StyleLearnRange> = listOf(StyleLearnRange.Everything, StyleLearnRange.UntilEndOfLastYear),
    earliest: String? = "2025-03-01",
) = StyleLearningStage.ChooseRange(
    email = "alice@test.local",
    presets = presets,
    earliest = earliest,
    latest = "2026-11-19",
    selected = selected,
    reach = earliest?.let { StyleSentReach.Since(it) } ?: StyleSentReach.Everything,
)

internal fun span(text: String, placeholder: Boolean = false) = WritingStyleSpan(text, placeholder)

internal fun styleLanguage(code: String, full: Boolean = true) = WritingStyleLanguage(
    language = code,
    letter = WritingStyleLetter(
        opening = listOf(span("Hi "), span("Name", placeholder = true), span(",")),
        paragraphs = listOf(WritingStyleParagraph(listOf(1.0, 0.6)), WritingStyleParagraph(listOf(0.8))),
        closing = listOf(span("Best,")),
        signsAs = "Alice",
        words = 80u,
        shape = "Two short paragraphs.",
    ).takeIf { full },
    habits = WritingStyleHabits(
        greetings = listOf(WritingStyleHabit(listOf(span("Hi")), 60u, 70u, WritingStyleFrequency.MOSTLY)),
        signOffs = listOf(WritingStyleHabit(listOf(span("Best")), 30u, 100u, WritingStyleFrequency.OFTEN)),
        signsAs = "Alice",
    ).takeIf { full },
    cards = if (full) listOf(WritingStyleCard(WritingStyleTopic.REGISTER, "Warm and direct", "Rarely formal.")) else emptyList(),
    phrases = WritingStylePhrases(used = listOf("Sounds good"), avoided = listOf("Per my last email")).takeIf { full },
)

internal fun styleReveal(
    id: String = "style-1",
    pages: List<WritingStyleRevealPage> = WritingStyleRevealPage.entries.toList(),
    languages: List<WritingStyleLanguage> = listOf(styleLanguage("en"), styleLanguage("nl", full = false)),
    save: ControlState = FREE,
    close: ControlState = FREE,
) = WritingStyleReveal(
    id = id,
    facts = styleFacts(),
    pages = pages,
    languages = languages,
    name = "My style",
    notes = "",
    save = save,
    close = close,
)

internal fun styleDetail(
    id: String = "style-1",
    rename: ControlState = FREE,
    forget: ControlState = FREE,
    drafting: List<String> = listOf("alice@test.local"),
) = WritingStyleDetail(
    id = id,
    facts = styleFacts(),
    languages = listOf(styleLanguage("en")),
    name = "Work",
    notes = "Keep it short.",
    rename = rename,
    notesControl = FREE,
    forget = forget,
    draftingAccounts = drafting,
)

internal fun endpointForm(
    address: String = "",
    model: String = "",
    declared: JurisdictionClass? = null,
    keyKept: Boolean = false,
    save: ControlState = FREE,
    remove: ControlState = HELD,
) = AiEndpointForm(
    address = address,
    model = model,
    declared = declared,
    keyKept = keyKept,
    gaps = AiEndpointGaps(null, false, null),
    save = save,
    remove = remove,
    outcome = null,
)

/** A Settings → Writing style the tests drive, every change recorded in [sent]. */
internal fun testWritingStyleUi(
    page: WritingStylePage?,
    learning: WritingStyleLearning? = null,
    sent: MutableList<WritingStyleIntent> = mutableListOf(),
    reveal: (String) -> WritingStyleReveal? = { null },
    detail: (String) -> WritingStyleDetail? = { null },
    signIns: MutableList<Unit> = mutableListOf(),
    version: Int = 0,
) = WritingStyleUi(
    page = page,
    learning = learning,
    version = version,
    reveal = reveal,
    detail = detail,
    nameRefusal = ::fakeStyleNameRefusal,
    onChange = { sent += it },
    onSignIn = { signIns += Unit },
    zone = UTC,
)

/** The learning intents among [sent], in order. */
internal fun learningSent(sent: List<WritingStyleIntent>): List<StyleLearningIntent> =
    sent.filterIsInstance<WritingStyleIntent.Learning>().map { it.intent }
