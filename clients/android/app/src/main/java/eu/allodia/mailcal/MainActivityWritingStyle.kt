// The writing assistant's records on the activity (docs/ai.md): read on connect and on the signals
// that move them, and the calls Settings → Writing style and Settings → Advanced make. Every read is
// from the core's memory, so the main thread may make it.
package eu.allodia.mailcal

import java.time.ZoneId
import uniffi.mailcal_bindings.AiIntent
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.writingStyleNameRefusal

/** Reads the page, the flow and the endpoint's form again. */
internal fun MainActivity.pullWritingStyle(app: MailcalApp) {
    writingStyle.page = app.writingStylePage()
    writingStyle.learning = app.writingStyleLearning()
    writingStyle.endpoint = app.aiEndpointForm()
    writingStyle.version += 1
}

/** Reads the endpoint's form again, and nothing else. */
internal fun MainActivity.pullAiEndpoint(app: MailcalApp) {
    writingStyle.endpoint = app.aiEndpointForm()
}

internal fun MainActivity.writingStyleUi(app: MailcalApp) = WritingStyleUi(
    page = writingStyle.page,
    learning = writingStyle.learning,
    version = writingStyle.version,
    reveal = { app.writingStyleReveal(it) },
    detail = { app.writingStyleDetail(it) },
    nameRefusal = ::writingStyleNameRefusal,
    onChange = { app.dispatch(Intent.Ai(AiIntent.WritingStyle(it))) },
    onSignIn = { changeAllodia(allodiaSignIn()) },
    zone = timeZone?.active?.let { runCatching { ZoneId.of(it) }.getOrNull() } ?: ZoneId.systemDefault(),
)

internal fun MainActivity.aiEndpointUi(app: MailcalApp) = AiEndpointUi(
    form = writingStyle.endpoint,
    gap = { address, model, declared -> app.aiEndpointGap(address, model, declared) },
    onChange = { app.dispatch(Intent.Ai(AiIntent.Endpoint(it))) },
)
