// Writing style's wiring (docs/ai.md): the snapshot and the own endpoint the screens draw, pulled
// on connect and on every WRITING_STYLE signal, and the core calls behind their actions. The calls
// that block (reading the Sent folder, waiting for the endpoint, asking the relay for the balance)
// never run here on the main thread: LearnFlow, the composer's draft control and refreshBalance
// each hand them to a background thread.
package eu.allodia.mailcal

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.CorpusReport
import uniffi.mailcal_bindings.JurisdictionClass
import uniffi.mailcal_bindings.LearnReport
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.OwnAiEndpoint
import uniffi.mailcal_bindings.OwnEndpointException
import uniffi.mailcal_bindings.WritingStyleDetail
import uniffi.mailcal_bindings.WritingStyleSnapshot

internal class WritingStyleState {
    // Null until the first pull. Its `route` decides whether Writing style is offered anywhere.
    var snapshot by mutableStateOf<WritingStyleSnapshot?>(null)

    var ownEndpoint by mutableStateOf<OwnAiEndpoint?>(null)
}

internal fun MainActivity.pullWritingStyle(app: MailcalApp) {
    writingStyle.snapshot = app.writingStyles()
    writingStyle.ownEndpoint = app.ownAiEndpoint()
}

internal fun MainActivity.writingStyleSettings(app: MailcalApp) = WritingStyleSettings(
    snapshot = writingStyle.snapshot,
    ownEndpoint = writingStyle.ownEndpoint,
    actions = CoreWritingStyleActions(app, this),
)

private class CoreWritingStyleActions(
    private val app: MailcalApp,
    private val activity: MainActivity,
) : WritingStyleActions {
    override fun report(account: String, since: Long?, until: Long?): CorpusReport =
        app.sentCorpusReport(account, since, until)

    override fun learn(
        account: String,
        since: Long?,
        until: Long?,
        name: String,
        uiLanguage: String,
    ): LearnReport = app.learnWritingStyle(account, since, until, name, uiLanguage)

    override fun cancel() = app.cancelWritingStyleLearning()

    override fun detail(id: String): WritingStyleDetail? = app.writingStyleDetail(id)

    override fun save(id: String, name: String, notes: String) {
        app.renameWritingStyle(id, name)
        app.updateWritingStyleNotes(id, notes)
    }

    override fun forget(id: String) {
        app.deleteWritingStyle(id)
    }

    override fun assign(account: String, style: String?) = app.setAccountWritingStyle(account, style)

    // Re-read either way: the endpoint record is not part of the snapshot the signal refreshes.
    override fun saveEndpoint(
        baseUrl: String,
        model: String,
        declared: JurisdictionClass?,
        key: String?,
    ): OwnEndpointException? = try {
        app.setOwnAiEndpoint(baseUrl, model, declared, key)
        null
    } catch (e: OwnEndpointException) {
        e
    } finally {
        activity.pullWritingStyle(app)
    }

    override fun removeEndpoint(): OwnEndpointException? = try {
        app.clearOwnAiEndpoint()
        null
    } catch (e: OwnEndpointException) {
        e
    } finally {
        activity.pullWritingStyle(app)
    }

    // The answer lands through the core's own WRITING_STYLE signal, and a failure leaves the line
    // it last reported, which is all the screen needs.
    override fun refreshBalance() {
        if (activity.writingStyle.snapshot?.route != AiRoute.RELAY) return
        ThreadBackground.run({ app.refreshAiBalance() }) {}
    }
}
