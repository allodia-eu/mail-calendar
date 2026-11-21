// The reveal's and the detail's look: type, colours, motion and the entrances a page plays once it
// is on screen. Under Reduce Motion nothing slides, grows or counts: every value is drawn final.

import SwiftUI

/// The type. macOS sizes follow the desktop's scale; iOS takes the matching text styles, so Dynamic
/// Type reaches them.
enum RevealFont {
    #if os(macOS)
    static let label = Font.callout.weight(.semibold)
    static let text = Font.body
    static let small = Font.callout
    static let caption = Font.subheadline
    static let habit = Font.system(size: 14, weight: .medium)
    static let chip = Font.system(size: 14)
    static let headline = Font.system(size: 16, weight: .semibold)
    static let pill = Font.system(size: 11, weight: .semibold)
    #else
    static let label = Font.footnote.weight(.semibold)
    static let text = Font.subheadline
    static let small = Font.footnote
    static let caption = Font.caption
    static let habit = Font.body.weight(.medium)
    static let chip = Font.subheadline
    static let headline = Font.headline
    static let pill = Font.caption.weight(.semibold)
    #endif
}

/// The curves.
enum RevealMotion {
    static func grow(_ delay: Double) -> Animation {
        .timingCurve(0.2, 0.8, 0.2, 1, duration: 0.7).delay(delay)
    }

    static func rise(_ delay: Double) -> Animation {
        .timingCurve(0.2, 0.8, 0.2, 1, duration: 0.5).delay(delay)
    }

    static func pop(_ delay: Double) -> Animation {
        .timingCurve(0.3, 1.25, 0.5, 1, duration: 0.45).delay(delay)
    }

    /// An ease-out cubic, as the counts use.
    static func count(_ duration: Double, _ delay: Double) -> Animation {
        .timingCurve(0.33, 1, 0.68, 1, duration: duration).delay(delay)
    }
}

/// The colours, by scheme. The accent is the system's.
struct RevealPalette {
    let dark: Bool

    init(_ scheme: ColorScheme) {
        dark = scheme == .dark
    }

    private static let grey = Color(red: 118 / 255, green: 118 / 255, blue: 128 / 255)
    private static let label = Color(red: 60 / 255, green: 60 / 255, blue: 67 / 255)
    private static let darkLabel = Color(red: 235 / 255, green: 235 / 255, blue: 245 / 255)

    /// A card, a chip, the miniature letter.
    var paper: Color {
        #if os(macOS)
        dark ? Color(red: 50 / 255, green: 50 / 255, blue: 54 / 255) : .white
        #else
        Color(uiColor: .secondarySystemGroupedBackground)
        #endif
    }

    /// The letter's grey lines.
    var ink: Color { dark ? Self.darkLabel.opacity(0.16) : Self.label.opacity(0.17) }
    var track: Color { Self.grey.opacity(dark ? 0.12 : 0.06) }
    var fill: Color { Self.grey.opacity(dark ? 0.24 : 0.12) }
    var fillStrong: Color { Self.grey.opacity(dark ? 0.36 : 0.20) }
    var accentSoft: Color { Color.accentColor.opacity(dark ? 0.32 : 0.17) }
    var accentFaint: Color { Color.accentColor.opacity(dark ? 0.16 : 0.09) }
    var separator: Color { dark ? Color.white.opacity(0.10) : Self.label.opacity(0.14) }
    var hairline: Color { dark ? Color.white.opacity(0.08) : Color.black.opacity(0.06) }
    var shadow: Color { Color.black.opacity(dark ? 0 : 0.08) }
}

/// How a part of a page arrives: rising into place, or popping in with a slight overshoot.
enum RevealEntrance {
    case rise, pop
}

extension View {
    /// Draws this hidden until `shown`, then brings it in `delay` seconds later.
    func revealEntrance(_ shown: Bool, _ kind: RevealEntrance, delay: Double) -> some View {
        modifier(RevealEntranceModifier(shown: shown, kind: kind, delay: delay))
    }

    /// A card's edge: a hairline, and a soft shadow in the light.
    func revealCard(_ palette: RevealPalette, radius: CGFloat) -> some View {
        background(palette.paper, in: RoundedRectangle(cornerRadius: radius))
            .overlay(
                RoundedRectangle(cornerRadius: radius).strokeBorder(palette.hairline, lineWidth: 0.5))
            .shadow(color: palette.shadow, radius: 1.5, y: 0.5)
    }
}

private struct RevealEntranceModifier: ViewModifier {
    let shown: Bool
    let kind: RevealEntrance
    let delay: Double

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        let visible = shown || reduceMotion
        content
            .opacity(visible ? 1 : 0)
            .offset(y: visible ? 0 : (kind == .rise ? 8 : 6))
            .scaleEffect(visible || kind == .rise ? 1 : 0.94)
            .animation(
                reduceMotion
                    ? nil : (kind == .rise ? RevealMotion.rise(delay) : RevealMotion.pop(delay)),
                value: shown
            )
    }
}

/// A bar or a line drawn from the leading edge to `fraction` of the width, so its growth animates
/// without stretching its rounded ends.
struct GrowingBar: Shape {
    var fraction: Double
    var cornerRadius: CGFloat

    var animatableData: Double {
        get { fraction }
        set { fraction = newValue }
    }

    func path(in rect: CGRect) -> Path {
        let width = rect.width * min(max(fraction, 0), 1)
        guard width > 0 else { return Path() }
        return Path(
            roundedRect: CGRect(x: rect.minX, y: rect.minY, width: width, height: rect.height),
            cornerRadius: min(cornerRadius, width / 2, rect.height / 2)
        )
    }
}

/// A whole number that counts towards its value as the value animates.
struct CountingNumber: View, Animatable {
    var value: Double
    let text: (Int) -> Text

    nonisolated var animatableData: Double {
        get { value }
        set { value = newValue }
    }

    var body: some View {
        text(Int(value.rounded()))
    }
}
