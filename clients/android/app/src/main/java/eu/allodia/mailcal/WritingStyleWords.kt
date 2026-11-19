// The writing assistant's records in the person's words (docs/ai.md rule 27): failures, remedies,
// ranges, what a review found and what a style's facts say. Plain functions with no view in them,
// so the JVM suite pins each one; every decision behind them is the core's.
package eu.allodia.mailcal

import android.content.Context
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.util.Locale
import uniffi.mailcal_bindings.AiEndpointAddressGap
import uniffi.mailcal_bindings.AiEndpointDeclarationGap
import uniffi.mailcal_bindings.AiFailure
import uniffi.mailcal_bindings.AiRemedy
import uniffi.mailcal_bindings.JurisdictionLimit
import uniffi.mailcal_bindings.JurisdictionRefusal
import uniffi.mailcal_bindings.StyleLearnRange
import uniffi.mailcal_bindings.StyleLearningDestination
import uniffi.mailcal_bindings.StyleLearningReport
import uniffi.mailcal_bindings.StyleSentReach
import uniffi.mailcal_bindings.WritingStyleFacts
import uniffi.mailcal_bindings.WritingStyleFrequency
import uniffi.mailcal_bindings.WritingStyleLetter
import uniffi.mailcal_bindings.WritingStyleRefusal
import uniffi.mailcal_bindings.WritingStyleRevealPage
import uniffi.mailcal_bindings.WritingStyleTopic

/** Why learning did not happen, in the person's words; never a server's sentence. */
internal fun aiFailureText(ctx: Context, failure: AiFailure): String = when (failure) {
    AiFailure.RelaySignIn -> L10n.ai_failure_relay_sign_in(ctx)
    AiFailure.EndpointKeyRefused -> L10n.ai_failure_key_refused(ctx)
    is AiFailure.Refused -> when {
        failure.refusal is JurisdictionRefusal.Unknown -> L10n.ai_failure_refused_undeclared(ctx)
        refusalLimit(failure.refusal) == JurisdictionLimit.EU_HOSTED -> L10n.ai_failure_refused_eu_hosted(ctx)
        else -> L10n.ai_failure_refused_eu_native(ctx)
    }
    AiFailure.NowhereToSend -> L10n.ai_failure_nowhere_to_send(ctx)
    AiFailure.Unreachable -> L10n.ai_failure_unreachable(ctx)
    AiFailure.ServerError -> L10n.ai_failure_server_error(ctx)
    AiFailure.TooManyRequests -> L10n.ai_failure_too_many_requests(ctx)
    AiFailure.Unreadable -> L10n.ai_failure_unreadable(ctx)
    AiFailure.OutOfCredits -> L10n.ai_failure_out_of_credits(ctx)
    AiFailure.NotEntitled -> L10n.ai_failure_not_entitled(ctx)
    is AiFailure.NoSentFolder -> L10n.ai_failure_no_sent_folder(ctx)
    AiFailure.NothingToLearn -> L10n.ai_failure_nothing_to_learn(ctx)
    AiFailure.SampleTooLarge -> L10n.ai_failure_sample_too_large(ctx)
    AiFailure.NoStyle -> L10n.ai_failure_no_style(ctx)
    AiFailure.NoStyleForAccount -> L10n.ai_failure_no_style_for_account(ctx)
    AiFailure.MessageGone -> L10n.ai_failure_message_gone(ctx)
}

/** The one action a failure offers, as its button says it. */
internal fun aiRemedyText(ctx: Context, remedy: AiRemedy): String = when (remedy) {
    AiRemedy.SIGN_IN_TO_ALLODIA -> L10n.ai_remedy_sign_in(ctx)
    AiRemedy.OPEN_AI_ENDPOINT -> L10n.ai_remedy_open_endpoint(ctx)
    AiRemedy.TRY_AGAIN -> L10n.ai_remedy_try_again(ctx)
    AiRemedy.CHOOSE_ACCOUNT -> L10n.ai_remedy_choose_account(ctx)
    AiRemedy.CHOOSE_RANGE -> L10n.ai_remedy_choose_range(ctx)
    AiRemedy.LEARN_STYLE -> L10n.ai_remedy_learn_style(ctx)
    AiRemedy.CHOOSE_STYLE -> L10n.ai_remedy_choose_style(ctx)
}

/** What pressing a remedy does on this client: the core moves the flow, or the client opens a place. */
internal enum class RemedyAction {
    /** `StyleLearningIntent.Remedy`: the core moves the flow. */
    MOVE_FLOW,

    /** The Allodia account's browser sign-in. */
    SIGN_IN,

    /** Settings → Advanced, where the own endpoint's form is. */
    OPEN_ENDPOINT,

    /** The learning flow, from the page's Learn. */
    LEARN,

    /** Settings → Writing style, where each account's style is chosen. */
    CHOOSE_STYLE,
}

internal fun remedyAction(remedy: AiRemedy): RemedyAction = when (remedy) {
    AiRemedy.CHOOSE_ACCOUNT, AiRemedy.CHOOSE_RANGE, AiRemedy.TRY_AGAIN -> RemedyAction.MOVE_FLOW
    AiRemedy.SIGN_IN_TO_ALLODIA -> RemedyAction.SIGN_IN
    AiRemedy.OPEN_AI_ENDPOINT -> RemedyAction.OPEN_ENDPOINT
    AiRemedy.LEARN_STYLE -> RemedyAction.LEARN
    AiRemedy.CHOOSE_STYLE -> RemedyAction.CHOOSE_STYLE
}

/** Why a change to the library was not made. */
internal fun writingStyleRefusalText(ctx: Context, refusal: WritingStyleRefusal): String = when (refusal) {
    WritingStyleRefusal.BLANK_NAME -> L10n.writing_style_refused_blank_name(ctx)
    WritingStyleRefusal.GONE -> L10n.writing_style_refused_gone(ctx)
}

/** What is wrong with the endpoint's address, said under the field. */
internal fun endpointAddressGapText(ctx: Context, gap: AiEndpointAddressGap): String = when (gap) {
    AiEndpointAddressGap.MISSING -> L10n.ai_endpoint_gap_address_missing(ctx)
    AiEndpointAddressGap.UNUSABLE -> L10n.ai_endpoint_gap_address_unusable(ctx)
    AiEndpointAddressGap.NEEDS_HTTPS -> L10n.ai_endpoint_gap_needs_https(ctx)
}

/** What is wrong with where the endpoint is declared to run, said under the choice. */
internal fun endpointDeclarationGapText(ctx: Context, gap: AiEndpointDeclarationGap): String = when (gap) {
    AiEndpointDeclarationGap.Missing -> L10n.ai_endpoint_gap_where_missing(ctx)
    is AiEndpointDeclarationGap.Refused -> when (refusalLimit(gap.refusal)) {
        JurisdictionLimit.EU_HOSTED -> L10n.ai_endpoint_gap_refused_eu_hosted(ctx)
        JurisdictionLimit.EU_NATIVE -> L10n.ai_endpoint_gap_refused_eu_native(ctx)
    }
}

// The mode a refusal is worded by: only `EuNative` refuses an endpoint run in the EU by a company
// based outside it.
private fun refusalLimit(refusal: JurisdictionRefusal): JurisdictionLimit = when (refusal) {
    JurisdictionRefusal.EuHosted -> JurisdictionLimit.EU_NATIVE
    is JurisdictionRefusal.NonEu -> refusal.mode
    is JurisdictionRefusal.Unknown -> refusal.mode
}

/** A language by its ISO 639-1 code, named in the language the app is read in. */
internal fun languageText(code: String, locale: Locale): String =
    Locale.forLanguageTag(code).getDisplayLanguage(locale)
        .takeIf { it.isNotBlank() && it != code }
        ?.replaceFirstChar { it.titlecase(locale) }
        ?: code

internal fun languagesText(codes: List<String>, locale: Locale): String =
    codes.joinToString(", ") { languageText(it, locale) }

/** A `YYYY-MM-DD` day as the core gives it, worded whole: no zone moves it. */
internal fun dayText(day: String, locale: Locale): String =
    runCatching { LocalDate.parse(day) }.getOrNull()
        ?.format(DateTimeFormatter.ofLocalizedDate(FormatStyle.LONG).withLocale(locale))
        ?: day

/** An instant the core counts in seconds, as the day it fell on where the person is. */
internal fun epochDayText(seconds: Long, zone: ZoneId, locale: Locale): String =
    Instant.ofEpochSecond(seconds).atZone(zone).toLocalDate()
        .format(DateTimeFormatter.ofLocalizedDate(FormatStyle.LONG).withLocale(locale))

/** A range the stage offers, as its choice is labelled. */
internal fun learnRangeText(ctx: Context, range: StyleLearnRange, locale: Locale): String = when (range) {
    StyleLearnRange.Everything -> L10n.learn_range_all(ctx)
    StyleLearnRange.UntilEndOfLastYear -> L10n.learn_range_until_last_year(ctx)
    is StyleLearnRange.Between -> L10n.learn_range_between(ctx)
}

/** How far back the device holds the account's mail. */
internal fun sentReachText(ctx: Context, reach: StyleSentReach, locale: Locale): String = when (reach) {
    StyleSentReach.Everything -> L10n.learn_report_horizon_all(ctx)
    is StyleSentReach.Since -> L10n.learn_report_horizon(ctx, date = dayText(reach.day, locale))
}

/** What the review found, one line each, in the order the contract names them. */
internal fun learnReportLines(ctx: Context, report: StyleLearningReport, zone: ZoneId, locale: Locale): List<String> =
    buildList {
        add(L10n.learn_report_found(ctx, report.found.toInt()))
        add(L10n.learn_report_usable(ctx, report.usable.toInt()))
        if (report.languages.isNotEmpty()) {
            add(L10n.learn_report_languages(ctx, languagesText(report.languages.map { it.language }, locale)))
        }
        if (report.otherLanguages > 0u) add(L10n.learn_report_undetected(ctx, report.otherLanguages.toInt()))
        val oldest = report.oldest
        val newest = report.newest
        if (oldest != null && newest != null) {
            add(
                L10n.learn_report_dates(
                    ctx,
                    oldest = epochDayText(oldest, zone, locale),
                    newest = epochDayText(newest, zone, locale),
                ),
            )
        }
    }

/** Where Learn sends what the review found. */
internal fun destinationText(ctx: Context, destination: StyleLearningDestination): String = when (destination) {
    StyleLearningDestination.Relay -> L10n.learn_consent_relay(ctx)
    is StyleLearningDestination.OwnEndpoint -> L10n.learn_consent_own(ctx, host = destination.host)
}

/** A run's progress: a count once the run knows its total, otherwise only that it runs. */
internal fun learnProgressText(ctx: Context, done: UInt, total: UInt): String =
    if (total == 0u) {
        L10n.learn_progress_starting(ctx)
    } else {
        L10n.learn_progress(ctx, done = done.toString(), total = total.toString())
    }

/** A run's progress as a fraction, or null while the run does not know its total. */
internal fun learnProgressFraction(done: UInt, total: UInt): Float? =
    if (total == 0u) null else (done.toFloat() / total.toFloat()).coerceIn(0f, 1f)

/** A library row's line under its name: how much, and when, it was learned from. */
internal fun styleFactsText(ctx: Context, facts: WritingStyleFacts, zone: ZoneId, locale: Locale): String {
    val learned = facts.learnedAt?.let {
        L10n.writing_style_learned_from(ctx, facts.messages.toInt(), epochDayText(it, zone, locale))
    }
    val languages = facts.languages.takeIf { it.isNotEmpty() }
        ?.let { L10n.writing_style_languages(ctx, languagesText(it, locale)) }
    return listOfNotNull(learned, languages).joinToString("\n")
}

/** The word beside a greeting's or sign-off's bar. */
internal fun frequencyText(ctx: Context, frequency: WritingStyleFrequency): String = when (frequency) {
    WritingStyleFrequency.MOSTLY -> L10n.reveal_frequency_mostly(ctx)
    WritingStyleFrequency.OFTEN -> L10n.reveal_frequency_often(ctx)
    WritingStyleFrequency.SOMETIMES -> L10n.reveal_frequency_sometimes(ctx)
}

/** What a tone-and-approach card is about. */
internal fun topicText(ctx: Context, topic: WritingStyleTopic): String = when (topic) {
    WritingStyleTopic.REGISTER -> L10n.reveal_register(ctx)
    WritingStyleTopic.STRUCTURE -> L10n.reveal_structure(ctx)
    WritingStyleTopic.MOVES -> L10n.reveal_moves(ctx)
    WritingStyleTopic.PUNCTUATION -> L10n.reveal_punctuation(ctx)
}

/** A reveal page's heading, which the detail uses for the same section. */
internal fun revealPageTitle(ctx: Context, page: WritingStyleRevealPage): String = when (page) {
    WritingStyleRevealPage.READ -> L10n.reveal_title(ctx)
    WritingStyleRevealPage.LETTER -> L10n.reveal_step_letter(ctx)
    WritingStyleRevealPage.HABITS -> L10n.reveal_step_habits(ctx)
    WritingStyleRevealPage.VOICE -> L10n.reveal_step_voice(ctx)
    WritingStyleRevealPage.PHRASES -> L10n.reveal_phrases(ctx)
    WritingStyleRevealPage.NAME -> L10n.reveal_step_name(ctx)
}

/** Whether a reveal page is about one language at a time, which puts the language control on it. */
internal val WritingStyleRevealPage.perLanguage: Boolean
    get() = this == WritingStyleRevealPage.LETTER ||
        this == WritingStyleRevealPage.HABITS ||
        this == WritingStyleRevealPage.VOICE ||
        this == WritingStyleRevealPage.PHRASES

/**
 * The locale the app's words are in: the catalog's language, and the region its copy is written
 * for (British English, European Portuguese and Spanish), so a date reads as the words around it do
 * rather than as the device's own region would have it.
 */
internal fun catalogLocale(ctx: Context): Locale = when (val language = appLanguage(ctx)) {
    "en" -> Locale.UK
    "pt" -> Locale.forLanguageTag("pt-PT")
    "es" -> Locale.forLanguageTag("es-ES")
    else -> Locale.forLanguageTag(language)
}

/** The language the app is read in, as the ISO 639-1 code `Learn` carries. */
internal fun appLanguage(ctx: Context): String =
    ctx.resources.configuration.locales[0].language.takeIf { it in L10n.LOCALES } ?: L10n.LOCALES.first()

/**
 * The miniature letter as one sentence for TalkBack (docs/ai.md rule 15): opening, length,
 * paragraphs, closing and name, each left out when it is empty or zero. A greeting or a sign-off is
 * its runs' text as written, a placeholder as its word; the paragraphs are the record's count,
 * never the lines drawn.
 */
internal fun letterSpoken(ctx: Context, letter: WritingStyleLetter): String {
    val opening = letter.opening.joinToString("") { it.text }.trim()
    val closing = letter.closing.joinToString("") { it.text }.trim()
    val name = letter.signsAs.trim()
    return listOfNotNull(
        opening.takeIf { it.isNotEmpty() }?.let { L10n.a11y_letter_opening(ctx, it) },
        letter.words.takeIf { it > 0u }?.let { L10n.a11y_letter_words(ctx, it.toInt()) },
        letter.paragraphCount.takeIf { it > 0u }?.let { L10n.a11y_letter_paragraphs(ctx, it.toInt()) },
        closing.takeIf { it.isNotEmpty() }?.let { L10n.a11y_letter_closing(ctx, it) },
        name.takeIf { it.isNotEmpty() }?.let { L10n.a11y_letter_name(ctx, it) },
    ).joinToString(", ")
}
