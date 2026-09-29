// The frame the two Writing style sheets share (docs/ai.md, "Learning"): a fixed page on macOS and
// the platform's full-height sheet on iPhone, page dots between Back and Next, and a page that
// arrives from the side the person moved towards. Under Reduce Motion nothing slides, grows or
// counts: every value is drawn final.

import MailcalBindings
import SwiftUI

/// A footer button the caller supplies.
struct WizardAction {
    let title: String
    var role: ButtonRole?
    var prominent = true
    var enabled = true
    let action: () -> Void
}

/// What stands where Next does.
enum WizardPrimary {
    /// Next, which the frame performs.
    case next(enabled: Bool)
    /// The caller's own action in its place: Save, Learn, Stop, Close.
    case action(WizardAction)
    /// Nothing yet.
    case none
}

struct WizardSheet<Principal: View, Page: View>: View {
    @Binding var pager: WizardPager
    let cancel: WizardAction?
    let showsBack: Bool
    let primary: WizardPrimary
    /// Whether a dot moves to its page; only where every page can be reached in any order.
    let allowsJump: Bool
    /// Whether `principal` (the reveal's language control) is on screen.
    let principalVisible: Bool
    let principal: Principal
    let page: (Int) -> Page

    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    init(
        pager: Binding<WizardPager>,
        cancel: WizardAction?,
        showsBack: Bool,
        primary: WizardPrimary,
        allowsJump: Bool = false,
        principalVisible: Bool = false,
        @ViewBuilder principal: () -> Principal,
        @ViewBuilder page: @escaping (Int) -> Page
    ) {
        _pager = pager
        self.cancel = cancel
        self.showsBack = showsBack
        self.primary = primary
        self.allowsJump = allowsJump
        self.principalVisible = principalVisible
        self.principal = principal()
        self.page = page
    }

    var body: some View {
        #if os(macOS)
        VStack(spacing: 0) {
            ZStack(alignment: .top) {
                stage
                principal
                    .frame(height: WizardMetrics.topBar, alignment: .bottom)
                    .opacity(principalVisible ? 1 : 0)
                    .offset(y: principalVisible ? 0 : -6)
                    .allowsHitTesting(principalVisible)
                    .accessibilityHidden(!principalVisible)
            }
            Divider()
            footer
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
        }
        .frame(width: 560, height: 640)
        .background(WizardPalette(scheme).sheet)
        #else
        NavigationStack {
            stage
                .safeAreaInset(edge: .bottom, spacing: 0) {
                    VStack(spacing: 0) {
                        Divider()
                        footer
                            .padding(.horizontal, 16)
                            .padding(.vertical, 10)
                    }
                    .background(WizardPalette(scheme).sheet)
                }
                .background(WizardPalette(scheme).sheet)
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    if let cancel {
                        ToolbarItem(placement: .cancellationAction) {
                            Button(cancel.title, role: cancel.role, action: cancel.action)
                        }
                    }
                    ToolbarItem(placement: .principal) {
                        principal
                            .opacity(principalVisible ? 1 : 0)
                            .allowsHitTesting(principalVisible)
                            .accessibilityHidden(!principalVisible)
                    }
                }
        }
        #endif
    }

    /// The page on screen. The one leaving only fades: a transition is read from the last frame a
    /// view was drawn in, so a slide on the way out would take the direction of the move before.
    private var stage: some View {
        let edge: CGFloat = pager.forward ? 44 : -44
        return ZStack {
            page(pager.index)
                .id(pager.index)
                .transition(.asymmetric(
                    insertion: .offset(x: edge).combined(with: .opacity),
                    removal: .opacity
                ))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipped()
    }

    @ViewBuilder
    private var footer: some View {
        #if os(macOS)
        ZStack {
            HStack(spacing: 8) {
                if let cancel {
                    Button(cancel.title, role: cancel.role, action: cancel.action)
                        .keyboardShortcut(.cancelAction)
                }
                Spacer()
                backButton
                primaryButton
            }
            dots
        }
        #else
        HStack(spacing: 8) {
            backButton.frame(maxWidth: .infinity, alignment: .leading)
            dots
            primaryButton.frame(maxWidth: .infinity, alignment: .trailing)
        }
        #endif
    }

    /// Back keeps its place while hidden, so Next does not move under the pointer.
    private var backButton: some View {
        Button(L10n.wizard_back()) { move(to: pager.index - 1) }
            .buttonStyle(.bordered)
            .opacity(showsBack ? 1 : 0)
            .disabled(!showsBack)
            .accessibilityHidden(!showsBack)
    }

    @ViewBuilder
    private var primaryButton: some View {
        switch primary {
        case let .next(enabled):
            Button(L10n.wizard_next()) { move(to: pager.index + 1) }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(!enabled)
        case let .action(action) where action.prominent:
            Button(action.title, role: action.role, action: action.action)
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(!action.enabled)
        case let .action(action):
            Button(action.title, role: action.role, action: action.action)
                .buttonStyle(.bordered)
                .disabled(!action.enabled)
        case .none:
            EmptyView()
        }
    }

    private var dots: some View {
        let palette = WizardPalette(scheme)
        return HStack(spacing: 2) {
            ForEach(0..<pager.count, id: \.self) { index in
                Capsule()
                    .fill(index == pager.index ? Color.accentColor : palette.dot)
                    .frame(width: index == pager.index ? 18 : 6, height: 6)
                    .padding(.vertical, 5)
                    .padding(.horizontal, 3)
                    .contentShape(Rectangle())
                    .onTapGesture { if allowsJump { move(to: index) } }
            }
        }
        .animation(reduceMotion ? nil : WizardMotion.settle, value: pager.index)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(L10n.a11y_wizard_step(step: String(pager.index + 1), total: String(pager.count)))
    }

    private func move(to target: Int) {
        withAnimation(reduceMotion ? nil : WizardMotion.page) { pager.go(to: target) }
    }
}

extension WizardSheet where Principal == EmptyView {
    init(
        pager: Binding<WizardPager>,
        cancel: WizardAction?,
        showsBack: Bool,
        primary: WizardPrimary,
        @ViewBuilder page: @escaping (Int) -> Page
    ) {
        self.init(
            pager: pager, cancel: cancel, showsBack: showsBack, primary: primary,
            principal: { EmptyView() }, page: page
        )
    }
}

/// A page: its heading, then its content, which may push its last part to the bottom with a
/// `Spacer` and scrolls when it does not fit.
struct WizardPage<Content: View>: View {
    let title: String
    /// Whether the macOS top bar is above the page, which then starts below it and never scrolls
    /// under it.
    var underTopBar = false
    @ViewBuilder let content: Content

    var body: some View {
        GeometryReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 14) {
                    Text(title)
                        .font(WizardFont.title)
                        .accessibilityAddTraits(.isHeader)
                        .fixedSize(horizontal: false, vertical: true)
                    content
                }
                .padding(.horizontal, WizardMetrics.gutter)
                .padding(.top, insets.inner)
                .padding(.bottom, 20)
                .frame(maxWidth: .infinity, minHeight: proxy.size.height, alignment: .topLeading)
            }
            .scrollBounceBehavior(.basedOnSize)
        }
        .padding(.top, insets.outer)
    }

    private var insets: (outer: CGFloat, inner: CGFloat) {
        #if os(macOS)
        underTopBar ? (WizardMetrics.topBar, 16) : (0, 30)
        #else
        (0, 16)
        #endif
    }
}

enum WizardMetrics {
    /// The band the macOS language control sits in.
    static let topBar: CGFloat = 52
    #if os(macOS)
    static let gutter: CGFloat = 24
    #else
    static let gutter: CGFloat = 18
    #endif
}

/// The sheets' type. macOS sizes follow the prototype's desktop scale; iOS takes the matching text
/// styles, so Dynamic Type still reaches them.
enum WizardFont {
    #if os(macOS)
    static let title = Font.title.weight(.semibold)
    static let label = Font.callout.weight(.semibold)
    static let text = Font.body
    static let small = Font.callout
    static let caption = Font.subheadline
    static let habit = Font.system(size: 14, weight: .medium)
    static let chip = Font.system(size: 14)
    static let headline = Font.system(size: 16, weight: .semibold)
    static let pill = Font.system(size: 11, weight: .semibold)
    #else
    static let title = Font.title2.weight(.semibold)
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

/// The prototype's curves.
enum WizardMotion {
    static let page = Animation.spring(response: 0.38, dampingFraction: 0.9)
    static let settle = Animation.timingCurve(0.32, 0.72, 0, 1, duration: 0.3)
    static func grow(_ delay: Double) -> Animation { .timingCurve(0.2, 0.8, 0.2, 1, duration: 0.7).delay(delay) }
    static func rise(_ delay: Double) -> Animation { .timingCurve(0.2, 0.8, 0.2, 1, duration: 0.5).delay(delay) }
    static func pop(_ delay: Double) -> Animation { .timingCurve(0.3, 1.25, 0.5, 1, duration: 0.45).delay(delay) }
    /// An ease-out cubic, as the counts use.
    static func count(_ duration: Double, _ delay: Double) -> Animation {
        .timingCurve(0.33, 1, 0.68, 1, duration: duration).delay(delay)
    }
}

/// The sheets' colours, by scheme. The accent is the system's.
struct WizardPalette {
    let dark: Bool

    init(_ scheme: ColorScheme) {
        dark = scheme == .dark
    }

    private static let grey = Color(red: 118 / 255, green: 118 / 255, blue: 128 / 255)
    private static let label = Color(red: 60 / 255, green: 60 / 255, blue: 67 / 255)
    private static let darkLabel = Color(red: 235 / 255, green: 235 / 255, blue: 245 / 255)

    var sheet: Color {
        #if os(macOS)
        dark ? Color(red: 42 / 255, green: 42 / 255, blue: 46 / 255) : Color(red: 245 / 255, green: 245 / 255, blue: 247 / 255)
        #else
        Color(uiColor: .systemGroupedBackground)
        #endif
    }

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
    var dot: Color { dark ? Self.darkLabel.opacity(0.28) : Self.label.opacity(0.30) }
    var hairline: Color { dark ? Color.white.opacity(0.08) : Color.black.opacity(0.06) }
    var shadow: Color { Color.black.opacity(dark ? 0 : 0.08) }
}

/// How a part of a page arrives: rising into place, or popping in with a slight overshoot.
enum WizardEntrance {
    case rise, pop
}

extension View {
    /// Draws this hidden until `shown`, then brings it in `delay` seconds later.
    func wizardEntrance(_ shown: Bool, _ kind: WizardEntrance, delay: Double) -> some View {
        modifier(WizardEntranceModifier(shown: shown, kind: kind, delay: delay))
    }

    /// A card's edge: a hairline, and a soft shadow in the light.
    func wizardCard(_ palette: WizardPalette, radius: CGFloat) -> some View {
        background(palette.paper, in: RoundedRectangle(cornerRadius: radius))
            .overlay(RoundedRectangle(cornerRadius: radius).strokeBorder(palette.hairline, lineWidth: 0.5))
            .shadow(color: palette.shadow, radius: 1.5, y: 0.5)
    }
}

private struct WizardEntranceModifier: ViewModifier {
    let shown: Bool
    let kind: WizardEntrance
    let delay: Double

    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        let visible = shown || reduceMotion
        content
            .opacity(visible ? 1 : 0)
            .offset(y: visible ? 0 : (kind == .rise ? 8 : 6))
            .scaleEffect(visible || kind == .rise ? 1 : 0.94)
            .animation(
                reduceMotion ? nil : (kind == .rise ? WizardMotion.rise(delay) : WizardMotion.pop(delay)),
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
