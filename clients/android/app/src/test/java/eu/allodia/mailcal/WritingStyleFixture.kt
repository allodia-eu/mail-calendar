// What the Writing style tests share: a snapshot builder, a core that answers from memory and
// records what it was asked, and a background that runs its work at once or when told to.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.AccountWritingStyleRow
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.CorpusLanguage
import uniffi.mailcal_bindings.CorpusReport
import uniffi.mailcal_bindings.CreditBalance
import uniffi.mailcal_bindings.GateRefusal
import uniffi.mailcal_bindings.HabitRow
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.LanguageStyleRow
import uniffi.mailcal_bindings.LearnReport
import uniffi.mailcal_bindings.LearningProgress
import uniffi.mailcal_bindings.OwnEndpointException
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleRow
import uniffi.mailcal_bindings.WritingStyleSnapshot

internal val ALICE_ACCOUNT = AccountWritingStyleRow("acct-alice", "alice@test.local", style = null)
internal val BOB_ACCOUNT = AccountWritingStyleRow("acct-bob", "bob@test.local", style = null)

internal val PLAIN_STYLE = WritingStyleRow(
    id = "style-plain",
    name = "Plain",
    sourceAccount = ALICE_ACCOUNT.accountId,
    languages = listOf("en", "nl"),
    messages = 12u,
    oldest = null,
    newest = null,
    // 2026-09-01T10:00:00Z
    learnedAt = 1_788_256_800,
)

internal fun writingStyleSnapshot(
    route: AiRoute? = AiRoute.OWN_ENDPOINT,
    refused: GateRefusal? = null,
    styles: List<WritingStyleRow> = emptyList(),
    accounts: List<AccountWritingStyleRow> = listOf(ALICE_ACCOUNT),
    learning: LearningProgress? = null,
    balance: CreditBalance? = null,
) = WritingStyleSnapshot(route, refused, styles, accounts, learning, balance)

internal fun corpusReport(usable: UInt = 30u, undetected: UInt = 0u, horizon: Long? = null) = CorpusReport(
    found = 42u,
    usable = usable,
    undetected = undetected,
    languages = if (usable > 0u) listOf(CorpusLanguage("en", usable, usable, 4_000u)) else emptyList(),
    oldest = null,
    newest = null,
    horizon = horizon,
)

internal val PLAIN_DETAIL = WritingStyleDetail(
    row = PLAIN_STYLE,
    notes = "Never use exclamation marks.",
    languages = listOf(
        LanguageStyleRow(
            language = "en",
            greetings = listOf(HabitRow("Hi Anna,", 70u)),
            signOffs = listOf(HabitRow("Best,", 55u)),
            signsAs = "Alice",
            register = "",
            typicalWords = 80u,
            shape = "Short paragraphs.",
            punctuation = "",
            structure = "",
            moves = "",
            phrases = listOf("Happy to help"),
            avoid = emptyList(),
        ),
    ),
)

// Runs the work where it is asked for, so a test reads the outcome on the next line.
internal object ImmediateBackground : Background {
    override fun <T> run(work: () -> T, done: (Result<T>) -> Unit) = done(runCatching(work))
}

// Holds the work until the test says, so it can move the flow on while a call is outstanding.
internal class HeldBackground : Background {
    private val held = mutableListOf<() -> Unit>()

    override fun <T> run(work: () -> T, done: (Result<T>) -> Unit) {
        held += { done(runCatching(work)) }
    }

    fun release() {
        val now = held.toList()
        held.clear()
        now.forEach { it() }
    }
}

internal class FakeWritingStyleActions(
    var reportAnswer: () -> CorpusReport = { corpusReport() },
    var learnAnswer: () -> LearnReport = { LearnReport(PLAIN_STYLE.id, listOf("en"), 30u, null) },
    var endpointRefusal: OwnEndpointException? = null,
) : WritingStyleActions {
    data class Asked(val account: String, val since: Long?, val until: Long?)
    data class Learned(val account: String, val until: Long?, val name: String, val uiLanguage: String)
    data class EndpointSaved(val baseUrl: String, val model: String, val declared: JurisdictionClass?, val key: String?)

    val reports = mutableListOf<Asked>()
    val learns = mutableListOf<Learned>()
    val saved = mutableListOf<Triple<String, String, String>>()
    val forgotten = mutableListOf<String>()
    val assigned = mutableListOf<Pair<String, String?>>()
    val endpoints = mutableListOf<EndpointSaved>()
    var cancels = 0
    var removals = 0
    var balanceRefreshes = 0

    override fun report(account: String, since: Long?, until: Long?): CorpusReport {
        reports += Asked(account, since, until)
        return reportAnswer()
    }

    override fun learn(account: String, since: Long?, until: Long?, name: String, uiLanguage: String): LearnReport {
        learns += Learned(account, until, name, uiLanguage)
        return learnAnswer()
    }

    override fun cancel() {
        cancels++
    }

    override fun detail(id: String): WritingStyleDetail? = PLAIN_DETAIL.takeIf { id == it.row.id }

    override fun save(id: String, name: String, notes: String) {
        saved += Triple(id, name, notes)
    }

    override fun forget(id: String) {
        forgotten += id
    }

    override fun assign(account: String, style: String?) {
        assigned += account to style
    }

    override fun saveEndpoint(
        baseUrl: String,
        model: String,
        declared: JurisdictionClass?,
        key: String?,
    ): OwnEndpointException? {
        endpoints += EndpointSaved(baseUrl, model, declared, key)
        return endpointRefusal
    }

    override fun removeEndpoint(): OwnEndpointException? {
        removals++
        return null
    }

    override fun refreshBalance() {
        balanceRefreshes++
    }
}
