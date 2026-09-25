// The reveal's steps and the arithmetic behind its pictures (docs/ai.md, "Learning" step 5). Plain
// values with no view in them, so the suite pins them: which page shows one language, how many grey
// lines a miniature letter draws, and what a card says when the core gave it no heading.

import CoreGraphics
import Foundation
import MailcalBindings

/// The reveal's pages, in the order the sheet walks them.
enum RevealStep: Int, CaseIterable {
    case read, letter, habits, voice, phrases, name

    /// Whether the page is about one language, which is what puts the language control on screen.
    var isPerLanguage: Bool {
        switch self {
        case .letter, .habits, .voice, .phrases: true
        case .read, .name: false
        }
    }
}

/// Where a stepped sheet stands: which of `count` pages is on screen, and which way the last move
/// went, which is the side the page arrives from.
struct WizardPager: Equatable {
    let count: Int
    private(set) var index = 0
    private(set) var forward = true

    init(count: Int) {
        self.count = max(count, 1)
    }

    var isFirst: Bool { index == 0 }
    var isLast: Bool { index == count - 1 }

    /// Moves to `target`, held to the pages there are.
    mutating func go(to target: Int) {
        let next = min(max(target, 0), count - 1)
        guard next != index else { return }
        forward = next > index
        index = next
    }
}

/// The miniature letter's grey lines: one list per paragraph, each line's width as a share of the
/// letter's. About fifteen words to a line, two paragraphs when the style does not say, and never
/// more lines than the letter has room for, so a long typical reply still reads as a letter.
func revealLetterLines(words: UInt32, paragraphs: UInt32) -> [[Double]] {
    let full = [1, 0.96, 0.98]
    let last = [0.58, 0.74, 0.44, 0.66]
    let count = min(paragraphs == 0 ? 2 : Int(paragraphs), 6)
    let wanted = words == 0 ? count * 2 : Int((Double(words) / 15).rounded())
    let lines = min(max(wanted, count), 12)
    return (0..<count).map { paragraph in
        let length = lines / count + (paragraph < lines % count ? 1 : 0)
        return (0..<length).map { line in
            line == length - 1 ? last[paragraph % last.count] : full[line % full.count]
        }
    }
}

/// A card's heading and the text beneath it. The core's heading goes over the whole description;
/// without one, the description's first sentence becomes the heading and the rest stays beneath,
/// so no sentence is shown twice.
func revealCardText(headline: String, description: String) -> (headline: String, body: String) {
    let heading = headline.trimmingCharacters(in: .whitespacesAndNewlines)
    let text = description.trimmingCharacters(in: .whitespacesAndNewlines)
    guard heading.isEmpty else { return (heading, text) }
    var first: Range<String.Index>?
    text.enumerateSubstrings(in: text.startIndex..<text.endIndex, options: .bySentences) { _, range, _, stop in
        first = range
        stop = true
    }
    guard let first else { return ("", "") }
    var sentence = text[first].trimmingCharacters(in: .whitespacesAndNewlines)
    if sentence.hasSuffix(".") { sentence.removeLast() }
    let rest = text[first.upperBound...].trimmingCharacters(in: .whitespacesAndNewlines)
    return (sentence, rest)
}

/// One run of a greeting or sign-off: words, or a bracketed placeholder such as `[Name]`, which the
/// reveal draws as a pill because there is no name to put in its place.
struct RevealRun: Equatable {
    let text: String
    let placeholder: Bool
}

/// `text` cut into runs, each placeholder apart from the words around it and without its brackets.
func revealRuns(_ text: String) -> [RevealRun] {
    var runs: [RevealRun] = []
    var rest = text[...]
    while let match = rest.firstMatch(of: /\[([^\]\[]+)\]/) {
        if match.range.lowerBound > rest.startIndex {
            runs.append(RevealRun(text: String(rest[..<match.range.lowerBound]), placeholder: false))
        }
        runs.append(RevealRun(text: String(match.output.1), placeholder: true))
        rest = rest[match.range.upperBound...]
    }
    if !rest.isEmpty { runs.append(RevealRun(text: String(rest), placeholder: false)) }
    return runs
}

/// The word beside a greeting's or sign-off's bar. The core decides which, so every client agrees.
func revealFrequencyText(_ frequency: HabitFrequency) -> String {
    switch frequency {
    case .mostly: L10n.reveal_frequency_mostly()
    case .often: L10n.reveal_frequency_often()
    case .sometimes: L10n.reveal_frequency_sometimes()
    }
}

/// The "Since" figure: the day and month within this year, the month and year before it, because
/// it is drawn at the size of the counts beside it and a full date does not fit there.
func revealSinceText(_ seconds: Int64, now: Date, zone: TimeZone, locale: Locale) -> String {
    let date = Date(timeIntervalSince1970: TimeInterval(seconds))
    var calendar = Calendar(identifier: .gregorian)
    calendar.timeZone = zone
    var style = Date.FormatStyle(locale: locale, calendar: calendar, timeZone: zone)
    if calendar.component(.year, from: date) == calendar.component(.year, from: now) {
        style = style.day().month(.wide)
    } else {
        style = style.month(.abbreviated).year()
    }
    return date.formatted(style)
}

/// `recipientFlowGeometry`'s lines, each centred in `limit`: the reveal's cloud of phrases.
///
/// Each line moves by whole points. A chip placed at a fractional origin is snapped to the pixel
/// grid at both edges, can come out a pixel narrower than it measured, and its text then truncates.
func centredFlowGeometry(
    sizes: [CGSize], limit: CGFloat, spacing: CGFloat
) -> (size: CGSize, positions: [CGPoint]) {
    let flow = recipientFlowGeometry(sizes: sizes, limit: limit, spacing: spacing)
    guard limit.isFinite else { return flow }
    var positions = flow.positions
    var start = 0
    while start < positions.count {
        var end = start
        while end + 1 < positions.count, positions[end + 1].y == positions[start].y { end += 1 }
        let width = positions[end].x + min(sizes[end].width, limit)
        let shift = ((limit - width) / 2).rounded(.down)
        for index in start...end { positions[index].x += shift }
        start = end + 1
    }
    return (CGSize(width: limit, height: flow.size.height), positions)
}
