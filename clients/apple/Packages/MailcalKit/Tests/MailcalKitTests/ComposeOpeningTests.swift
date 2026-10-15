// How a composer opens from the core's `ComposeOpening` (docs/drafts.md).
//
// What the opening holds is the core's, and is held by its own Rust tests. What is Apple's, and
// what is covered here, is what the client does with it: the mode it names, which of the three
// seeds the editor gets and in which order, the attribution a reopened quote is completed with,
// the style a reopened quote's picker opens on, and what an assistant's draft opens on.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

/// A `ComposeOpening` with every field defaulted, for the suites that build one.
func composeOpening(
    composition: String = "c-1",
    kind: ComposeKind = .new,
    subject: String = "",
    bodyHtml: String = "",
    bodyText: String = "",
    quote: QuoteSource? = nil,
    attachments: [ComposerFileAttachment] = [],
    seedsSignature: Bool = true
) -> ComposeOpening {
    ComposeOpening(
        composition: composition,
        kind: kind,
        account: "acct-1",
        to: "",
        cc: "",
        bcc: "",
        subject: subject,
        bodyHtml: bodyHtml,
        bodyText: bodyText,
        quote: quote,
        attachments: attachments,
        attachmentsFailed: false,
        seedsSignature: seedsSignature
    )
}

/// The context the shell would build for `opening`, with no quote words applied.
func composeContext(_ opening: ComposeOpening, isShare: Bool = false) -> ComposeContext {
    ComposeContext(
        opening: opening,
        initialFrom: nil,
        quoteStyle: .indented,
        quoteStylePerMessage: false,
        quote: nil,
        quoteAttribution: nil,
        isShare: isShare
    )
}

@Suite struct ComposeOpeningTests {
    private var original: QuoteSource {
        QuoteSource(
            from: "Ada <ada@example.test>",
            date: "2026-08-31T07:01:00Z",
            to: "grace@example.test",
            cc: "",
            subject: "Planning",
            bodyHtml: "<p>Earlier</p>",
            bodyPlain: "Earlier"
        )
    }

    private var words: QuoteWords {
        QuoteWords(
            line: "On Monday, Ada wrote:",
            headers: [(label: "From", value: "Ada"), (label: "Subject", value: "Planning")]
        )
    }

    private func object(_ json: String?) throws -> [String: Any] {
        let json = try #require(json)
        return try #require(try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any])
    }

    @Test func theKindNamesTheMode() {
        #expect(ComposeOpeningSeed.mode(.new) == .new)
        #expect(ComposeOpeningSeed.mode(.reply) == .reply)
        #expect(ComposeOpeningSeed.mode(.replyAll) == .replyAll)
        #expect(ComposeOpeningSeed.mode(.forward) == .forward)
    }

    @Test func aStoredReplyReopensTitledReply() {
        // A stored reply reopens as a reply, and seeds from the reply slot if it seeds at all.
        let context = composeContext(composeOpening(kind: .reply, bodyHtml: "<p>Hi</p>"))
        #expect(context.mode == .reply)
        #expect(context.title == L10n.action_reply())
    }

    @Test func aFreshReplySeedsItsQuoteWithThePrefillAboveIt() throws {
        let opening = composeOpening(kind: .reply, bodyText: "Sounds good.", quote: original)
        let seed = try object(ComposeOpeningSeed.quoteSeed(opening, words: words, style: "Indented"))
        #expect(seed["style"] as? String == "Indented")
        #expect(seed["body_html"] as? String == "<p>Earlier</p>")
        #expect(seed["initial_text"] as? String == "Sounds good.")
        let attribution = try #require(seed["attribution"] as? [String: Any])
        #expect(attribution["line"] as? String == "On Monday, Ada wrote:")
        #expect((attribution["headers"] as? [[String: String]])?.first?["label"] == "From")
        // The text rides in the quote, so it is not seeded a second time as a plain body.
        #expect(ComposeOpeningSeed.plainBody(opening, quoteSeed: "{}") == nil)
    }

    @Test func aFreshReplyWithNothingAboveTheQuoteCarriesNoInitialText() throws {
        let opening = composeOpening(kind: .reply, quote: original)
        let seed = try object(ComposeOpeningSeed.quoteSeed(opening, words: words, style: "Indented"))
        #expect(!seed.keys.contains("initial_text"))
    }

    @Test func aStoredMessageOpensThroughItsHTMLAndCompletesItsAttribution() throws {
        let opening = composeOpening(
            kind: .reply, bodyHtml: "<p>Hi</p>", bodyText: "Hi", quote: original
        )
        #expect(ComposeOpeningSeed.quoteSeed(opening, words: words, style: "Indented") == nil)
        #expect(ComposeOpeningSeed.plainBody(opening, quoteSeed: nil) == "Hi")
        let completion = try object(ComposeOpeningSeed.attributionCompletion(opening, words: words))
        #expect(completion["line"] as? String == "On Monday, Ada wrote:")
        #expect((completion["headers"] as? [[String: String]])?.count == 2)
    }

    @Test func onlyAStoredMessageWithAnOriginalIsCompleted() {
        let fresh = composeOpening(kind: .reply, quote: original)
        let orphan = composeOpening(kind: .reply, bodyHtml: "<p>Hi</p>")
        #expect(ComposeOpeningSeed.attributionCompletion(fresh, words: words) == nil)
        #expect(ComposeOpeningSeed.attributionCompletion(orphan, words: nil) == nil)
    }

    @Test func aPrefilledNewMessageOpensWithItsText() {
        let opening = composeOpening(bodyText: "From a link")
        #expect(ComposeOpeningSeed.quoteSeed(opening, words: nil, style: "Indented") == nil)
        #expect(ComposeOpeningSeed.plainBody(opening, quoteSeed: nil) == "From a link")
        #expect(ComposeOpeningSeed.plainBody(composeOpening(), quoteSeed: nil) == nil)
    }

    @Test func theEditorIsSeededStoredBodyThenQuoteThenText() throws {
        let stored = try #require(
            RichComposerEditor.openingSeedScript(html: "<p>Hi</p>", quote: "{}", text: "Hi")
        )
        #expect(stored.hasPrefix("window.setComposerBody("))
        let quoted = try #require(
            RichComposerEditor.openingSeedScript(html: nil, quote: "{}", text: "Hi")
        )
        #expect(quoted.hasPrefix("window.setComposerQuote("))
        #expect(
            RichComposerEditor.openingSeedScript(html: "", quote: nil, text: "Hi")
                == "window.setPlainText(\"Hi\")"
        )
        #expect(RichComposerEditor.openingSeedScript(html: nil, quote: nil, text: nil) == nil)
    }

    @Test func aReopenedQuoteIsReadBackInTheStyleItWasSavedIn() {
        let document = """
        {"blocks":[{"Paragraph":{"text":"Hi"}},{"Quote":{"style":"LineAndHeader"}}],"attachments":[]}
        """
        #expect(ComposeOpeningSeed.quoteStyle(ofDocument: document) == .lineAndHeader)
        #expect(
            ComposeOpeningSeed.quoteStyle(ofDocument: #"{"blocks":[{"Quote":{"style":"Indented"}}]}"#)
                == .indented
        )
        #expect(ComposeOpeningSeed.quoteStyle(ofDocument: #"{"blocks":[],"attachments":[]}"#) == nil)
        #expect(ComposeOpeningSeed.quoteStyle(ofDocument: "not json") == nil)
        #expect(ComposeOpeningSeed.quoteStyle(ofDocument: nil) == nil)
    }

    @Test func anAssistantsReplyOpensAsAReplyInItsConversation() {
        // A reply names the message it answers, so it stays in that conversation.
        let draft = AgentDraft(
            account: "acct-2", to: "ada@example.test", cc: "", bcc: "", subject: "Re: Planning",
            bodyText: "Yes.\n\nBob", replyToAccount: "acct-1", replyToKey: "m-7"
        )
        let (origin, prefill) = ComposeOpeningSeed.agentDraftOpening(draft, selected: "acct-3")
        #expect(origin == .reply(account: "acct-1", key: "m-7", all: false))
        #expect(prefill.to == "ada@example.test")
        #expect(prefill.bodyText == "Yes.\n\nBob")
        // Its author signed it, so the account's signature is not seeded under it.
        #expect(prefill.signed)
        // It answers from the account it names, which need not be the one the message is in.
        #expect(prefill.account == "acct-2")
    }

    @Test func anAssistantsNewMessageOpensFromTheAccountItNames() {
        let named = AgentDraft(
            account: "acct-2", to: "", cc: "", bcc: "", subject: "", bodyText: "",
            replyToAccount: nil, replyToKey: nil
        )
        #expect(
            ComposeOpeningSeed.agentDraftOpening(named, selected: "acct-3").origin
                == .new(account: "acct-2")
        )
        let unnamed = AgentDraft(
            account: nil, to: "", cc: "", bcc: "", subject: "", bodyText: "",
            replyToAccount: "acct-1", replyToKey: nil
        )
        #expect(
            ComposeOpeningSeed.agentDraftOpening(unnamed, selected: "acct-3").origin
                == .new(account: "acct-3")
        )
    }

    @Test func aDraftThatCouldNotOpenSaysSoInItsOwnWords() {
        #expect(
            composeOpenFailedMessage(for: .draft(account: "a", key: "k"))
                == L10n.compose_draft_open_failed()
        )
        #expect(composeOpenFailedMessage(for: .new(account: nil)) == L10n.compose_open_failed())
        #expect(
            composeOpenFailedMessage(for: .forward(account: "a", key: "k"))
                == L10n.compose_open_failed()
        )
    }

    @Test func theSameMessageOpenedTwiceIsTwoComposers() {
        // iOS presents a context with `.sheet(item:)` or `.fullScreenCover(item:)`, which do nothing
        // when the new item's id equals the one already presented.
        let opening = composeOpening()
        #expect(composeContext(opening) != composeContext(opening))
    }

    @Test func aWindowShowingADraftIsNamedForItsSubject() {
        #expect(composeContext(composeOpening(subject: "Terms")).windowTitle == "Terms")
        // The Window menu, Cmd-Tab and Mission Control read this, and an empty string there is a
        // window with no name at all (`docs/reading-window.md`).
        #expect(composeContext(composeOpening()).windowTitle == L10n.compose_title_new())
    }
}
