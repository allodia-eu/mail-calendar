// Learning a writing style, as a state machine the sheet draws (docs/ai.md, "Learning"): which
// account, which sent mail, what the device found there and what would be sent where, then the
// run itself. Plain Kotlin, so the JVM suite drives every step without a screen or a core.
//
// The two calls that read the Sent folder and wait for the endpoint block, so each goes through a
// [Background] and lands back on the main thread. A result that arrives after the person moved on
// (another range, a closed sheet) is dropped rather than drawn over what they are looking at now.
package eu.allodia.mailcal

import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.time.LocalDate
import java.time.ZoneId
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.AccountWritingStyleRow
import uniffi.mailcal_bindings.CorpusReport
import uniffi.mailcal_bindings.LearnReport
import uniffi.mailcal_bindings.WritingStyleFailure

// Runs blocking work off the main thread and hands the outcome back on it.
internal interface Background {
    fun <T> run(work: () -> T, done: (Result<T>) -> Unit)
}

internal object ThreadBackground : Background {
    private val main = Handler(Looper.getMainLooper())

    override fun <T> run(work: () -> T, done: (Result<T>) -> Unit) {
        thread(name = "mailcal-writing-style") {
            val result = runCatching(work)
            main.post { done(result) }
        }
    }
}

// The three core calls a learning run makes. `report` and `learn` block and throw
// `WritingStyleFailure`.
internal interface LearnCore {
    fun report(account: String, since: Long?, until: Long?): CorpusReport

    fun learn(account: String, since: Long?, until: Long?, name: String, uiLanguage: String): LearnReport

    fun cancel()
}

internal sealed interface LearnRange {
    data object Everything : LearnRange

    data class Until(val day: LocalDate) : LearnRange
}

internal sealed interface CorpusState {
    data object Reading : CorpusState

    data class Ready(val report: CorpusReport) : CorpusState

    data class Failed(val failure: WritingStyleFailure) : CorpusState
}

internal sealed interface LearnStep {
    data class ChooseAccount(val accounts: List<AccountWritingStyleRow>) : LearnStep

    // The range, what the device holds in it and the consent, on one sheet: choosing a range reads
    // it straight away, because reading sends nothing anywhere.
    data class Consent(val account: String, val range: LearnRange, val corpus: CorpusState) : LearnStep

    data class Learning(val account: String) : LearnStep

    data class Learned(val styleId: String) : LearnStep

    data class Failed(val failure: WritingStyleFailure) : LearnStep
}

internal class LearnFlow(
    private val accounts: List<AccountWritingStyleRow>,
    private val core: LearnCore,
    private val background: Background,
    private val zone: ZoneId,
) {
    var step: LearnStep by mutableStateOf(LearnStep.ChooseAccount(accounts))
        private set

    // Bumped by every call that makes an earlier one's answer stale.
    private var ticket = 0

    // With one account there is nothing to ask.
    fun start() {
        accounts.singleOrNull()?.let { chooseAccount(it.accountId) }
    }

    fun chooseAccount(account: String) = read(account, LearnRange.Everything)

    fun chooseRange(range: LearnRange) {
        val current = step as? LearnStep.Consent ?: return
        read(current.account, range)
    }

    // Pressing Learn on the consent sheet is the consent; there is no other prompt.
    fun consent(name: String, uiLanguage: String) {
        val current = step as? LearnStep.Consent ?: return
        val corpus = current.corpus as? CorpusState.Ready ?: return
        if (corpus.report.usable == 0u) return
        val mine = ++ticket
        val (since, until) = bounds(current.range)
        step = LearnStep.Learning(current.account)
        background.run({ core.learn(current.account, since, until, name, uiLanguage) }) { result ->
            if (mine != ticket) return@run
            step = result.fold(
                { LearnStep.Learned(it.styleId) },
                { LearnStep.Failed(it.asWritingStyleFailure()) },
            )
        }
    }

    // The core stops before its next request and answers `Cancelled`, which the sheet then shows.
    fun stop() {
        if (step is LearnStep.Learning) core.cancel()
    }

    // Closing the sheet stops a run: nobody is left to see where it ends.
    fun close() {
        stop()
        ticket++
    }

    private fun read(account: String, range: LearnRange) {
        val mine = ++ticket
        val (since, until) = bounds(range)
        step = LearnStep.Consent(account, range, CorpusState.Reading)
        background.run({ core.report(account, since, until) }) { result ->
            if (mine != ticket) return@run
            val corpus = result.fold(
                { CorpusState.Ready(it) },
                { CorpusState.Failed(it.asWritingStyleFailure()) },
            )
            step = LearnStep.Consent(account, range, corpus)
        }
    }

    private fun bounds(range: LearnRange): Pair<Long?, Long?> = when (range) {
        LearnRange.Everything -> null to null
        is LearnRange.Until -> null to endOfDay(range.day, zone)
    }
}
