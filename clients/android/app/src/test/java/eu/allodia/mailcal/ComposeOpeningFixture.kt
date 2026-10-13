// A `ComposeOpening` as the core answers `openComposer`, for the suites that render a composer or
// read the rules over one. Every field defaults to what a blank new message opens with.
package eu.allodia.mailcal

import uniffi.mailcal_bindings.ComposeKind
import uniffi.mailcal_bindings.ComposeOpening
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.QuoteSource

internal fun testOpening(
    kind: ComposeKind = ComposeKind.NEW,
    composition: String = "c-0",
    account: String = "work",
    to: String = "",
    cc: String = "",
    bcc: String = "",
    subject: String = "",
    bodyHtml: String = "",
    bodyText: String = "",
    quote: QuoteSource? = null,
    attachments: List<ComposerFileAttachment> = emptyList(),
    attachmentsFailed: Boolean = false,
    seedsSignature: Boolean = bodyHtml.isEmpty(),
) = ComposeOpening(
    composition = composition,
    kind = kind,
    account = account,
    to = to,
    cc = cc,
    bcc = bcc,
    subject = subject,
    bodyHtml = bodyHtml,
    bodyText = bodyText,
    quote = quote,
    attachments = attachments,
    attachmentsFailed = attachmentsFailed,
    seedsSignature = seedsSignature,
)

internal fun testQuote(
    from: String = "sender@remote.test",
    date: String = "2026-07-10T13:34:00Z",
    to: String = "me@allodia.local",
    cc: String = "",
    subject: String = "Quarterly report",
    bodyHtml: String = "<p>Original</p>",
    bodyPlain: String = "Original",
) = QuoteSource(
    from = from,
    date = date,
    to = to,
    cc = cc,
    subject = subject,
    bodyHtml = bodyHtml,
    bodyPlain = bodyPlain,
)
