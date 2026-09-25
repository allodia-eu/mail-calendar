// What the Writing style screens and the Draft a reply control say, pinned as which catalog key
// each outcome reads (docs/ai.md). A failure is a variant the client words, never a server's
// sentence, and two of them read differently depending on where requests go or on the gate's mode,
// which is exactly the kind of rule a view gets backwards without anything noticing.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct WritingStyleCopyTests {
    @Test func everyFailureReadsItsOwnKey() {
        let expected: [(WritingStyleFailure, String)] = [
            (.Unavailable, L10n.ai_error_unavailable()),
            (.Busy, L10n.ai_error_busy()),
            (.NoSentFolder, L10n.ai_error_no_sent_folder()),
            (.NothingToLearn, L10n.learn_report_nothing()),
            (.NoStyle, L10n.ai_error_no_style()),
            (.NotFound, L10n.ai_error_not_found()),
            (.OutOfCredits, L10n.ai_error_out_of_credits()),
            (.NotEntitled, L10n.ai_error_not_entitled()),
            (.RateLimited, L10n.ai_error_rate_limited()),
            (.Unreachable, L10n.ai_error_unreachable()),
            (.Status(code: 503), L10n.ai_error_status(code: "503")),
            (.Malformed, L10n.ai_error_malformed()),
            (.Cancelled, L10n.ai_error_cancelled()),
        ]
        for (failure, text) in expected {
            #expect(writingStyleFailureText(failure, route: .ownEndpoint) == text)
        }
    }

    /// ⚠️ A refused sign-in to the relay is repaired by signing in again; a refused key by changing
    /// the key under Advanced. The same variant, two different sentences.
    @Test func anUnauthorisedRequestSaysWhatToRepairForTheRoute() {
        #expect(writingStyleFailureText(.Unauthorized, route: .relay) == L10n.ai_error_sign_in_again())
        #expect(writingStyleFailureText(.Unauthorized, route: .ownEndpoint) == L10n.ai_error_key_refused())
    }

    @Test func aRefusalIsWordedByTheModeInForce() {
        #expect(
            writingStyleFailureText(.Refused(mode: .euNative, class: .nonEu), route: .ownEndpoint)
                == L10n.writing_style_refused_eu_native()
        )
        #expect(
            writingStyleFailureText(.Refused(mode: .euHosted, class: .nonEu), route: .ownEndpoint)
                == L10n.writing_style_refused_eu_hosted()
        )
    }

    @Test func eachEndpointErrorNamesTheFieldToFix() {
        #expect(ownEndpointErrorText(.InvalidUrl) == L10n.ai_endpoint_error_address())
        #expect(ownEndpointErrorText(.NotHttps) == L10n.ai_endpoint_error_https())
        #expect(ownEndpointErrorText(.NoModel) == L10n.ai_endpoint_error_model())
        #expect(ownEndpointErrorText(.Keystore("denied")) == L10n.ai_endpoint_error_keystore())
    }

    /// The key never comes back to the screen, so an empty field has to mean "keep", or saving an
    /// edit to the model name would erase a key nobody can see.
    @Test func anEmptyKeyFieldKeepsTheStoredKey() {
        #expect(ownEndpointKeyArgument("") == nil)
        #expect(ownEndpointKeyArgument("   ") == nil)
        #expect(ownEndpointKeyArgument(" sk-local ") == "sk-local")
    }

    @Test func onlyAReplyOrAReplyAllOffersADraftAndOnlyWithARoute() {
        #expect(offersDraftReply(mode: .reply, route: .ownEndpoint))
        #expect(offersDraftReply(mode: .replyAll, route: .relay))
        #expect(!offersDraftReply(mode: .forward, route: .ownEndpoint))
        #expect(!offersDraftReply(mode: .new, route: .ownEndpoint))
        #expect(!offersDraftReply(mode: .reply, route: nil))
    }

    @Test func aDraftNamesTheSenderOnlyWhenThePersonChangedIt() {
        #expect(draftReplyFrom(answered: "work", sending: "work") == nil)
        #expect(draftReplyFrom(answered: "work", sending: "home") == "home")
        #expect(draftReplyFrom(answered: "work", sending: nil) == nil)
    }

    @Test func anEmptyIntentIsNoIntent() {
        #expect(draftReplyIntent("") == nil)
        #expect(draftReplyIntent(" \n ") == nil)
        #expect(draftReplyIntent(" Yes ") == "Yes")
    }

    /// At most one decimal, in the locale's own digits, and today's report reads as the clock.
    @Test func theCreditsLineRoundsToOneDecimalAndDatesLikeAListRow() throws {
        let utc = try #require(TimeZone(identifier: "UTC"))
        // 2026-09-23 09:05 UTC.
        let asOf: Int64 = 1_790_154_300
        let now = Date(timeIntervalSince1970: TimeInterval(asOf) + 3600)
        let balance = CreditBalance(credits: 12.34, asOf: asOf)
        #expect(
            writingStyleCreditsLine(balance, now: now, zone: utc, locale: Locale(identifier: "en"))
                == L10n.writing_style_credits(credits: "12.3", time: "09:05")
        )
        #expect(
            writingStyleCreditsLine(balance, now: now, zone: utc, locale: Locale(identifier: "nl"))
                == L10n.writing_style_credits(credits: "12,3", time: "09:05")
        )
        let whole = CreditBalance(credits: 5, asOf: asOf)
        #expect(
            writingStyleCreditsLine(whole, now: now, zone: utc, locale: Locale(identifier: "en"))
                == L10n.writing_style_credits(credits: "5", time: "09:05")
        )
    }
}
