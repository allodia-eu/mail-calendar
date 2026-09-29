// The words a client puts to Writing style (docs/ai.md, "Client seams"): each failure the core
// names as a variant, the gate's refusal, the own endpoint's errors, and the lines built from the
// snapshot. Plain functions over the catalog, so the JVM suite pins every one without a screen. A
// server's own sentence never reaches any of them: the core hands over a variant, not a message.
package eu.allodia.mailcal

import android.content.Context
import java.net.URI
import java.text.NumberFormat
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale
import uniffi.mailcal_bindings.AiRoute
import uniffi.mailcal_bindings.CorpusReport
import uniffi.mailcal_bindings.CreditBalance
import uniffi.mailcal_bindings.JurisdictionMode
import uniffi.mailcal_bindings.OwnEndpointException
import uniffi.mailcal_bindings.WritingStyleFailure
import uniffi.mailcal_bindings.WritingStyleRow

internal fun writingStyleFailureText(
    ctx: Context,
    failure: WritingStyleFailure,
    route: AiRoute?,
): String = when (failure) {
    is WritingStyleFailure.Unavailable -> L10n.ai_error_unavailable(ctx)
    is WritingStyleFailure.Busy -> L10n.ai_error_busy(ctx)
    is WritingStyleFailure.NoSentFolder -> L10n.ai_error_no_sent_folder(ctx)
    is WritingStyleFailure.NothingToLearn -> L10n.learn_report_nothing(ctx)
    is WritingStyleFailure.NoStyle -> L10n.ai_error_no_style(ctx)
    is WritingStyleFailure.NotFound -> L10n.ai_error_not_found(ctx)
    is WritingStyleFailure.Refused -> writingStyleRefusalText(ctx, failure.mode)
    is WritingStyleFailure.OutOfCredits -> L10n.ai_error_out_of_credits(ctx)
    is WritingStyleFailure.NotEntitled -> L10n.ai_error_not_entitled(ctx)
    // Through the relay it is the Allodia sign-in that was refused; an own endpoint refuses a key.
    is WritingStyleFailure.Unauthorized -> if (route == AiRoute.RELAY) {
        L10n.ai_error_sign_in_again(ctx)
    } else {
        L10n.ai_error_key_refused(ctx)
    }
    is WritingStyleFailure.RateLimited -> L10n.ai_error_rate_limited(ctx)
    is WritingStyleFailure.Unreachable -> L10n.ai_error_unreachable(ctx)
    is WritingStyleFailure.Status -> L10n.ai_error_status(ctx, failure.code.toString())
    is WritingStyleFailure.Malformed -> L10n.ai_error_malformed(ctx)
    is WritingStyleFailure.Cancelled -> L10n.ai_error_cancelled(ctx)
}

// `All` admits every destination, so the gate never refuses under it.
internal fun writingStyleRefusalText(ctx: Context, mode: JurisdictionMode): String = when (mode) {
    JurisdictionMode.EU_HOSTED -> L10n.writing_style_refused_eu_hosted(ctx)
    JurisdictionMode.EU_NATIVE, JurisdictionMode.ALL -> L10n.writing_style_refused_eu_native(ctx)
}

internal fun ownEndpointErrorText(ctx: Context, error: OwnEndpointException): String = when (error) {
    is OwnEndpointException.InvalidUrl -> L10n.ai_endpoint_error_address(ctx)
    is OwnEndpointException.NotHttps -> L10n.ai_endpoint_error_https(ctx)
    is OwnEndpointException.NoModel -> L10n.ai_endpoint_error_model(ctx)
    is OwnEndpointException.Keystore -> L10n.ai_endpoint_error_keystore(ctx)
}

// What a blocking call threw, as the variant the copy above words. Anything else is a fault in
// the binding rather than an answer, and is worded as an answer that could not be read.
internal fun Throwable.asWritingStyleFailure(): WritingStyleFailure =
    this as? WritingStyleFailure ?: WritingStyleFailure.Malformed()

// ISO 639-1 codes as the endonyms the language picker uses; a language the catalog does not ship
// reads as its code.
internal fun writingStyleLanguages(ctx: Context, codes: List<String>): String =
    codes.joinToString(", ") { L10n.languageName(ctx, it) }

// The catalog locale the app is shown in, which the core writes a style's description in. Read off
// the configuration the strings resolve against, so the description and the screen agree.
internal fun catalogLocale(ctx: Context): String =
    ctx.resources.configuration.locales[0].language.takeIf { it in L10n.LOCALES }
        ?: L10n.LOCALES.first()

// The display zone the core holds, as java.time reads it; the device's own when it has none or
// java.time does not know it.
internal fun displayZone(activeZoneId: String?): ZoneId =
    activeZoneId?.let { runCatching { ZoneId.of(it) }.getOrNull() } ?: ZoneId.systemDefault()

// A library row's lines under its name: what it was learned from, and in which languages. A style
// with no learning date (one that came from another device) says nothing about it.
internal fun writingStyleRowLines(
    ctx: Context,
    row: WritingStyleRow,
    zone: ZoneId,
    locale: Locale,
): List<String> = buildList {
    if (row.learnedAt > 0) {
        val date = epochDate(row.learnedAt, zone, locale)
        add(L10n.writing_style_learned_from(ctx, count = row.messages.toInt(), date = date))
    }
    if (row.languages.isNotEmpty()) {
        add(L10n.writing_style_languages(ctx, languages = writingStyleLanguages(ctx, row.languages)))
    }
}

// What the device found in the range, for the consent sheet. A line that would say nothing is
// left out rather than stated as zero.
internal fun corpusReportLines(
    ctx: Context,
    report: CorpusReport,
    zone: ZoneId,
    locale: Locale,
): List<String> = buildList {
    add(L10n.learn_report_found(ctx, count = report.found.toInt()))
    add(L10n.learn_report_usable(ctx, count = report.usable.toInt()))
    if (report.languages.isNotEmpty()) {
        val codes = report.languages.map { it.language }
        add(L10n.learn_report_languages(ctx, languages = writingStyleLanguages(ctx, codes)))
    }
    if (report.undetected > 0u) {
        add(L10n.learn_report_undetected(ctx, count = report.undetected.toInt()))
    }
    report.horizon?.let { add(L10n.learn_report_horizon(ctx, date = epochDate(it, zone, locale))) }
}

// A calendar date for a Unix instant, in the display zone.
internal fun epochDate(seconds: Long, zone: ZoneId, locale: Locale): String =
    Instant.ofEpochSecond(seconds).atZone(zone).toLocalDate()
        .format(DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).withLocale(locale))

// The last second of `day`, local time: the core's `until` is inclusive, so "up to and including"
// the chosen day ends here rather than at the next midnight.
internal fun endOfDay(day: LocalDate, zone: ZoneId): Long =
    day.plusDays(1).atStartOfDay(zone).toEpochSecond() - 1

// A Unix instant in the engine's timestamp shape, so it goes through the list rows' own formatter.
internal fun engineTimestamp(seconds: Long): String =
    DateTimeFormatter.ofPattern("yyyy-MM-dd'T'HH:mm:ss'Z'")
        .withZone(ZoneOffset.UTC)
        .format(Instant.ofEpochSecond(seconds))

// The credits line: the relay's figure to at most one decimal, in the locale's own digits, and
// `asOf` already formatted by the caller the way a timestamp is.
internal fun writingStyleCreditsText(
    ctx: Context,
    balance: CreditBalance,
    locale: Locale,
    asOf: String,
): String {
    val credits = NumberFormat.getNumberInstance(locale).apply { maximumFractionDigits = 1 }
        .format(balance.credits)
    return L10n.writing_style_credits(ctx, credits = credits, time = asOf)
}

// The host the consent sheet names for an own endpoint. The address was validated when it was
// saved, so an unreadable one is shown whole rather than as nothing.
internal fun endpointHost(baseUrl: String): String =
    runCatching { URI(baseUrl).host }.getOrNull()?.takeIf { it.isNotEmpty() } ?: baseUrl
