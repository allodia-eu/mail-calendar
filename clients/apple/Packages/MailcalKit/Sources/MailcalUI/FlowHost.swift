// The one host for a flow of several steps (docs/ai.md, "The flow host"). It draws a record the
// core gives it: the stage on screen, its title, Cancel, Back and the primary action, each from a
// `ControlState`, and a step count only when the record has one. It holds no step, index or rule:
// which stage follows, whether Back exists and what a press does are the caller's record's. A
// paged result is `FlowPages`, which keeps only which page is on screen.
//
// macOS: the content of a sheet, a button row under the stage with Cancel at its leading edge and
// Back and the primary at its trailing edge, the primary the default button. iOS and iPadOS: the
// content of a sheet holding a `NavigationStack`, each stage pushed, so the back button and the
// edge swipe are Back; Cancel at the leading toolbar placement and the primary at the
// confirmation placement. Presented with `.sheet`, which is full height on iPhone and a page sheet
// on iPad.

import MailcalBindings
import SwiftUI

/// A control of a flow, as the record gives it: the client words it, the core holds or runs it.
struct FlowControl {
    let title: String
    let control: ControlState
    var role: ButtonRole?
    let action: () -> Void
}

/// Where a flow is, when its record knows: drawn as dots. A flow that branches gives none.
struct FlowProgress: Equatable {
    /// Zero-based.
    let step: Int
    let count: Int
    /// What assistive technology reads for the dots, worded by the caller.
    let label: String
}

/// The path an iOS host pushes for a trail, and whether a new path is the system's Back.
enum FlowNavigation {
    /// Every stage after the first, which is the stack's root.
    static func path<Stage>(_ trail: [Stage]) -> [Stage] {
        Array(trail.dropFirst())
    }

    /// Whether the stack went from `old` to `new` by popping: the back button or the edge swipe.
    static func popped<Stage>(from old: [Stage], to new: [Stage]) -> Bool {
        new.count < old.count
    }
}

struct FlowHost<Stage: Hashable, Page: View>: View {
    /// Every stage from the first to the one on screen, as the record gives it; the last is drawn.
    let trail: [Stage]
    let title: (Stage) -> String
    let cancel: FlowControl
    /// Back, where the stage has one.
    let back: FlowControl?
    /// The primary action, where the stage has one: Next, Learn, Stop, Close.
    let primary: FlowControl?
    let progress: FlowProgress?
    let page: (Stage) -> Page

    #if os(macOS)
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Whether the last move went deeper, so the next stage arrives from that side.
    @State private var forward = true
    #else
    /// The stack's path, following `trail`; kept so a pop the system has drawn already is not
    /// pushed again before the record catches up.
    @State private var path: [Stage]
    #endif

    init(
        trail: [Stage], title: @escaping (Stage) -> String, cancel: FlowControl,
        back: FlowControl?, primary: FlowControl?, progress: FlowProgress? = nil,
        @ViewBuilder page: @escaping (Stage) -> Page
    ) {
        self.trail = trail
        self.title = title
        self.cancel = cancel
        self.back = back
        self.primary = primary
        self.progress = progress
        self.page = page
        #if os(iOS)
        _path = State(initialValue: FlowNavigation.path(trail))
        #endif
    }

    var body: some View {
        #if os(macOS)
        VStack(spacing: 0) {
            if let stage = trail.last {
                FlowStage(id: stage, forward: forward) {
                    FlowPage(title: title(stage)) { page(stage) }
                }
            }
            Divider()
            FlowFooter(cancel: cancel, progress: progress, back: back, primary: primary)
        }
        .frame(minWidth: 520, idealWidth: 560, minHeight: 480, idealHeight: 600)
        .onChange(of: trail.count) { old, new in
            forward = new >= old
        }
        .animation(reduceMotion ? nil : FlowMotion.page, value: trail.last)
        #else
        NavigationStack(path: pathBinding) {
            if let root = trail.first {
                stagePage(root)
                    .navigationDestination(for: Stage.self) { stagePage($0) }
            }
        }
        .onChange(of: trail) { _, latest in path = FlowNavigation.path(latest) }
        #endif
    }

    #if os(iOS)
    /// The system's Back pops the path itself; the record is told, and moves the stage.
    private var pathBinding: Binding<[Stage]> {
        Binding(
            get: { path },
            set: { latest in
                let wentBack = FlowNavigation.popped(from: path, to: latest)
                path = latest
                if wentBack { back?.action() }
            }
        )
    }

    private func stagePage(_ stage: Stage) -> some View {
        // The stage's title is its page's heading, which wraps, rather than the bar's, which would
        // cut a question short between Cancel and the primary.
        FlowPage(title: title(stage)) { page(stage) }
            .navigationBarTitleDisplayMode(.inline)
            // Without Back on the record, or held, the stack offers neither the button nor the
            // swipe.
            .navigationBarBackButtonHidden(back?.control.enabled != true)
            .safeAreaInset(edge: .bottom, spacing: 0) {
                if let progress { FlowDots(progress: progress).padding(.vertical, 8) }
            }
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { FlowButton(cancel) }
                if let primary {
                    ToolbarItem(placement: .confirmationAction) {
                        FlowButton(primary).fontWeight(.semibold)
                    }
                }
            }
    }
    #endif
}

/// A paged result in the flow's host, such as a style shown back once learned: the caller's pages,
/// moved by the person, and the record's Cancel and primary. Keeps only which page is on screen.
///
/// macOS: the same sheet, the page dots between Cancel and Back. iOS and iPadOS: a paged `TabView`
/// with its page indicator.
struct FlowPages<Page: View>: View {
    let count: Int
    let title: (Int) -> String
    let cancel: FlowControl
    /// The primary action on a page, where that page has one.
    let primary: (Int) -> FlowControl?
    /// The words for the macOS row's own Back and Next, and for the dots.
    let backTitle: String
    let nextTitle: String
    let progressLabel: (Int) -> String
    /// Moving between pages, held by the record while a change runs.
    var paging: ControlState = .free()
    @ViewBuilder let page: (Int) -> Page

    @State private var index = 0
    #if os(macOS)
    @State private var forward = true
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    #endif

    var body: some View {
        #if os(macOS)
        VStack(spacing: 0) {
            FlowStage(id: index, forward: forward) {
                FlowPage(title: title(index)) { page(index) }
            }
            Divider()
            FlowFooter(
                cancel: cancel,
                progress: FlowProgress(step: index, count: count, label: progressLabel(index)),
                back: index > 0 ? FlowControl(title: backTitle, control: paging) { move(-1) } : nil,
                primary: primary(index)
                    ?? (index + 1 < count
                        ? FlowControl(title: nextTitle, control: paging) { move(1) } : nil)
            )
        }
        .frame(minWidth: 520, idealWidth: 560, minHeight: 480, idealHeight: 600)
        #else
        NavigationStack {
            HeldFields(control: paging) {
                TabView(selection: $index) {
                    ForEach(0..<count, id: \.self) { page in
                        FlowPage(title: title(page)) { self.page(page) }
                            .tag(page)
                    }
                }
                .tabViewStyle(.page(indexDisplayMode: .always))
                .indexViewStyle(.page(backgroundDisplayMode: .always))
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { FlowButton(cancel) }
                if let primary = primary(index) {
                    ToolbarItem(placement: .confirmationAction) {
                        FlowButton(primary).fontWeight(.semibold)
                    }
                }
            }
        }
        #endif
    }

    #if os(macOS)
    private func move(_ by: Int) {
        forward = by > 0
        withAnimation(reduceMotion ? nil : FlowMotion.page) {
            index = min(max(index + by, 0), count - 1)
        }
    }
    #endif
}

/// One control of the flow, through the control components so it waits as every control does.
private struct FlowButton: View {
    let flow: FlowControl

    init(_ flow: FlowControl) {
        self.flow = flow
    }

    var body: some View {
        ControlButton(title: flow.title, role: flow.role, control: flow.control, action: flow.action)
    }
}

/// A stage's page: its heading, then its content, scrolling when it does not fit.
private struct FlowPage<Content: View>: View {
    let title: String
    @ViewBuilder let content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text(title)
                    .font(FlowMetrics.title)
                    .accessibilityAddTraits(.isHeader)
                    .fixedSize(horizontal: false, vertical: true)
                content
            }
            .padding(.horizontal, FlowMetrics.gutter)
            .padding(.vertical, FlowMetrics.top)
            .frame(maxWidth: .infinity, alignment: .topLeading)
        }
        .scrollBounceBehavior(.basedOnSize)
        #if os(iOS)
        // Grouped, so an inset group on the page stands out from it as it does in Settings.
        .background(Color(uiColor: .systemGroupedBackground))
        #endif
    }
}

#if os(macOS)
/// The page on screen. The one leaving only fades: a transition is read from the last frame a view
/// was drawn in, so a slide on the way out would take the direction of the move before.
private struct FlowStage<ID: Hashable, Content: View>: View {
    let id: ID
    let forward: Bool
    @ViewBuilder let content: Content

    var body: some View {
        ZStack {
            content
                .id(id)
                .transition(.asymmetric(
                    insertion: .offset(x: forward ? 44 : -44).combined(with: .opacity),
                    removal: .opacity
                ))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .clipped()
    }
}

/// The macOS button row: Cancel at the leading edge, the dots, then Back and the primary at the
/// trailing edge. Where the labels do not fit on one row, the buttons stack, each label whole.
private struct FlowFooter: View {
    let cancel: FlowControl
    let progress: FlowProgress?
    let back: FlowControl?
    let primary: FlowControl?

    var body: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: 8) {
                cancelButton.fixedSize()
                Spacer(minLength: 12)
                if let progress { FlowDots(progress: progress) }
                Spacer(minLength: 12)
                backButton?.fixedSize()
                primaryButton?.fixedSize()
            }
            VStack(alignment: .trailing, spacing: 8) {
                primaryButton
                backButton
                if let progress { FlowDots(progress: progress) }
                cancelButton
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
    }

    private var cancelButton: some View {
        FlowButton(cancel).keyboardShortcut(.cancelAction)
    }

    private var backButton: (some View)? {
        back.map { FlowButton($0).buttonStyle(.bordered) }
    }

    /// The default button, unless it destroys something, which Return must not do by accident.
    private var primaryButton: (some View)? {
        primary.map {
            FlowButton($0)
                .buttonStyle(.borderedProminent)
                .keyboardShortcut($0.role == .destructive ? nil : .defaultAction)
        }
    }
}
#endif

/// Where the flow is, as dots: the current one long and in the accent colour.
private struct FlowDots: View {
    let progress: FlowProgress
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 6) {
            ForEach(0..<progress.count, id: \.self) { index in
                Capsule()
                    .fill(index == progress.step ? Color.accentColor : Color.secondary.opacity(0.35))
                    .frame(width: index == progress.step ? 18 : 6, height: 6)
            }
        }
        .animation(reduceMotion ? nil : FlowMotion.settle, value: progress.step)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(progress.label)
    }
}

private enum FlowMetrics {
    #if os(macOS)
    static let gutter: CGFloat = 24
    static let top: CGFloat = 24
    static let title = Font.title.weight(.semibold)
    #else
    static let gutter: CGFloat = 20
    static let top: CGFloat = 8
    static let title = Font.title2.weight(.bold)
    #endif
}

private enum FlowMotion {
    static let page = Animation.spring(response: 0.38, dampingFraction: 0.9)
    static let settle = Animation.timingCurve(0.32, 0.72, 0, 1, duration: 0.3)
}
