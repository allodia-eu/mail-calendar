// The reveal's steps and the arithmetic behind its pictures (WritingStyleReveal.swift). Each rule is
// one a view gets wrong while looking right: a letter whose lines ignore the paragraph count, a card
// that repeats its heading as its first sentence, a placeholder drawn with its brackets.

import CoreGraphics
import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct WritingStyleRevealTests {
    @Test func theLanguageControlIsOnTheFourPagesAboutOneLanguage() {
        #expect(RevealStep.allCases.count == 6)
        #expect(RevealStep.allCases.filter(\.isPerLanguage) == [.letter, .habits, .voice, .phrases])
    }

    @Test func thePagerStaysWithinItsPagesAndKnowsWhichWayItWent() {
        var pager = WizardPager(count: 6)
        #expect(pager.isFirst && !pager.isLast)
        pager.go(to: -1)
        #expect(pager.index == 0 && pager.forward)
        pager.go(to: 5)
        #expect(pager.isLast && pager.forward)
        pager.go(to: 9)
        #expect(pager.index == 5)
        pager.go(to: 4)
        #expect(pager.index == 4 && !pager.forward)
        // Standing still is not a move back.
        pager.go(to: 4)
        #expect(!pager.forward)
        pager.go(to: 5)
        #expect(pager.forward)
    }

    /// Seventy words in two paragraphs is about five lines, the longer paragraph first, and each
    /// paragraph ends on a short line.
    @Test func aLetterDrawsItsWordsAtAboutFifteenToALine() {
        let lines = revealLetterLines(words: 70, paragraphs: 2)
        #expect(lines.map(\.count) == [3, 2])
        for paragraph in lines {
            #expect(paragraph.last! < 0.8)
            #expect(paragraph.dropLast().allSatisfy { $0 > 0.9 })
        }
    }

    @Test func aLetterWithoutAParagraphCountDrawsTwo() {
        #expect(revealLetterLines(words: 90, paragraphs: 0).count == 2)
        #expect(revealLetterLines(words: 0, paragraphs: 0).map(\.count) == [2, 2])
    }

    @Test func aLetterHasALineForEveryParagraphAndRoomForAll() {
        #expect(revealLetterLines(words: 10, paragraphs: 3).map(\.count) == [1, 1, 1])
        let long = revealLetterLines(words: 900, paragraphs: 3)
        #expect(long.map(\.count).reduce(0, +) == 12)
        #expect(revealLetterLines(words: 200, paragraphs: 40).count == 6)
    }

    @Test func aCardKeepsTheCoresHeadingOverTheWholeDescription() {
        let card = revealCardText(headline: " Friendly and direct ", description: "On first-name terms. Brief.")
        #expect(card.headline == "Friendly and direct")
        #expect(card.body == "On first-name terms. Brief.")
    }

    @Test func aCardWithoutAHeadingLeadsWithItsFirstSentenceOnce() {
        let card = revealCardText(headline: "", description: "Thanks first, then the answer. Short lines.")
        #expect(card.headline == "Thanks first, then the answer")
        #expect(card.body == "Short lines.")
        let single = revealCardText(headline: "", description: "Says no politely.")
        #expect(single.headline == "Says no politely")
        #expect(single.body.isEmpty)
        #expect(revealCardText(headline: "", description: "  ") == ("", ""))
    }

    @Test func aPlaceholderIsARunOfItsOwnWithoutItsBrackets() {
        #expect(revealRuns("Hi [Name],") == [
            RevealRun(text: "Hi ", placeholder: false),
            RevealRun(text: "Name", placeholder: true),
            RevealRun(text: ",", placeholder: false),
        ])
        #expect(revealRuns("Cheers,") == [RevealRun(text: "Cheers,", placeholder: false)])
        #expect(revealRuns("[Name]") == [RevealRun(text: "Name", placeholder: true)])
        #expect(revealRuns("Hi [] all") == [RevealRun(text: "Hi [] all", placeholder: false)])
    }

    @Test func eachFrequencyReadsItsOwnKey() {
        #expect(revealFrequencyText(.mostly) == L10n.reveal_frequency_mostly())
        #expect(revealFrequencyText(.often) == L10n.reveal_frequency_often())
        #expect(revealFrequencyText(.sometimes) == L10n.reveal_frequency_sometimes())
    }

    @Test func sinceIsTheDayThisYearAndTheMonthBeforeIt() throws {
        let utc = try #require(TimeZone(identifier: "UTC"))
        let locale = Locale(identifier: "en_GB")
        // 2026-09-24 12:00 UTC.
        let now = Date(timeIntervalSince1970: 1_790_251_200)
        // 2026-06-24 and 2025-03-03.
        #expect(revealSinceText(1_782_302_400, now: now, zone: utc, locale: locale) == "24 June")
        #expect(revealSinceText(1_740_960_000, now: now, zone: utc, locale: locale) == "Mar 2025")
    }

    @Test func aCloudCentresEachOfItsLines() {
        let sizes = [CGSize(width: 40, height: 20), CGSize(width: 40, height: 20), CGSize(width: 90, height: 20)]
        let flow = centredFlowGeometry(sizes: sizes, limit: 100, spacing: 10)
        #expect(flow.size == CGSize(width: 100, height: 50))
        #expect(flow.positions == [CGPoint(x: 5, y: 0), CGPoint(x: 55, y: 0), CGPoint(x: 5, y: 30)])
    }

    /// A chip at a fractional origin can be snapped a pixel narrower than it measured, which
    /// truncates its text, so a line moves by whole points only.
    @Test func aCloudMovesItsLinesByWholePoints() {
        let flow = centredFlowGeometry(sizes: [CGSize(width: 40.5, height: 20)], limit: 100, spacing: 8)
        #expect(flow.positions == [CGPoint(x: 29, y: 0)])
    }
}
