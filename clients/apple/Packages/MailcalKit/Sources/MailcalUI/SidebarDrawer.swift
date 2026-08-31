// The phone's accounts-and-folders sidebar: the same tree the desktop draws, slid in from the
// leading edge over everything else, including the tab bar, so what is on screen is the sidebar
// and not a sidebar sharing the screen with navigation it cannot act on.
//
// This is a real pane, not a menu. That is the whole point: a menu has no tree, so an account
// cannot be expanded and `docs/folder-pane.md` rules 2 and 3, expansion independent of selection,
// persisted across launches, have nothing on a phone to apply to.

#if os(iOS)
import MailcalBindings
import SwiftUI

/// How wide the panel is, and how much of the screen behind it stays visible.
///
/// The strip left showing is what makes the drawer read as *over* the mailbox rather than as a
/// screen of its own; it is also the target most people reach for to dismiss it.
private let panelInset: CGFloat = 56
private let panelMaxWidth: CGFloat = 340

/// How wide the leading strip is that starts an open-drag.
///
/// The system's own back-swipe owns this edge inside a `NavigationStack`, so the strip is
/// deliberately narrow: wide enough to catch a thumb arriving from off-screen, narrow enough that
/// it does not swallow a swipe meant for the row underneath.
private let edgeGrabWidth: CGFloat = 20

private let slide: Animation = .interactiveSpring(response: 0.32, dampingFraction: 0.86)

/// Wraps `content` in a leading drawer holding `sidebar`.
struct SidebarDrawer<Sidebar: View, Content: View>: View {
    @Binding var isOpen: Bool
    /// Whether a drag from the leading edge opens the drawer.
    ///
    /// **Off wherever a `NavigationStack` has something to pop**: the system's back-swipe owns that
    /// edge, and a grab strip laid over it wins, so a pushed message would become impossible to
    /// leave by the gesture everyone uses to leave it.
    let edgeSwipeEnabled: Bool
    private let sidebar: Sidebar
    private let content: Content

    init(
        isOpen: Binding<Bool>,
        edgeSwipeEnabled: Bool,
        @ViewBuilder sidebar: () -> Sidebar,
        @ViewBuilder content: () -> Content
    ) {
        _isOpen = isOpen
        self.edgeSwipeEnabled = edgeSwipeEnabled
        self.sidebar = sidebar()
        self.content = content()
    }

    /// The live drag, or `nil` when no finger is down. Held apart from `isOpen` so releasing mid-way
    /// animates to whichever end it settled on rather than snapping from wherever it was let go.
    @State private var drag: CGFloat?

    var body: some View {
        GeometryReader { proxy in
            let width = min(panelMaxWidth, proxy.size.width - panelInset)
            let offset = sidebarDrawerOffset(isOpen: isOpen, translation: drag ?? 0, width: width)
            // How far in the panel is, 0…1, the scrim and the panel's shadow both follow it, so
            // the drawer dims and lifts with the drag instead of appearing when it finishes.
            let progress = sidebarDrawerProgress(offset: offset, width: width)

            ZStack(alignment: .leading) {
                content
                scrim(progress, width: width)
                panel(width: width, offset: offset, progress: progress)
                // Only when shut: open, the panel and the scrim carry their own drag, and a grab
                // strip left lying over the folder list would eat its scroll.
                //
                // Deliberately **not** also gated on `drag == nil`. Two recognizers share that
                // state, so one being interrupted rather than ended leaves it set, and a strip
                // that waits for nil then never comes back, taking the open-swipe with it for the
                // rest of the launch.
                if edgeSwipeEnabled, !isOpen {
                    Color.clear
                        .frame(width: edgeGrabWidth)
                        .contentShape(Rectangle())
                        .gesture(swipe(width: width))
                }
            }
            .animation(slide, value: isOpen)
        }
    }

    /// The dimmed mailbox beside the panel. Tapping it shuts the drawer, the dismissal most people
    /// reach for, and the only one a pointer or a switch control can reach.
    ///
    /// A tap gesture on the `Color` rather than a `Button` wrapping one: as a button label an
    /// edge-to-edge colour reports no interaction region of its own, so it dimmed the mailbox
    /// convincingly and swallowed every tap on it.
    @ViewBuilder private func scrim(_ progress: CGFloat, width: CGFloat) -> some View {
        if progress > 0 {
            Color.black.opacity(0.35 * progress)
                .ignoresSafeArea()
                .contentShape(Rectangle())
                .onTapGesture { withAnimation(slide) { isOpen = false } }
                .gesture(swipe(width: width))
                .accessibilityLabel(L10n.a11y_close_folders())
                .accessibilityAddTraits(.isButton)
                .accessibilityAction { isOpen = false }
        }
    }

    private func panel(width: CGFloat, offset: CGFloat, progress: CGFloat) -> some View {
        sidebar
            .frame(width: width)
            .background(.background)
            .compositingGroup()
            // Faded with the drag, because the shadow is cast to the trailing side: a shut panel
            // sits at `-width`, off-screen, and a shadow drawn at full strength there still spilled
            // a dark band down the leading edge of the mailbox. See sidebarDrawerShadowOpacity.
            .shadow(
                color: .black.opacity(sidebarDrawerShadowOpacity(progress: progress)),
                radius: 12,
                x: 2
            )
            .offset(x: offset)
            .simultaneousGesture(swipe(width: width))
            // VoiceOver must not wander into the mailbox behind an open drawer, it is covered, and
            // tapping what it reads there does nothing a sighted user could do.
            .accessibilityElement(children: .contain)
            .accessibilityAddTraits(.isModal)
            .accessibilityHidden(offset <= -width)
            // The scrim is a Button, but `.isModal` takes it out of VoiceOver along with everything
            // else outside the panel, so without this the one dismissal left is a swipe, and a
            // drawer you cannot shut is a screen you cannot leave. The two-finger scrub lands here.
            .accessibilityAction(.escape) { isOpen = false }
    }

    private func swipe(width: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 8, coordinateSpace: .global)
            .onChanged { value in
                // Once a drag is ours it stays ours; deciding afresh each frame would drop the
                // pane the moment a finger curved. The first frame decides, and it decides on
                // direction, a vertical drag belongs to the folder list underneath.
                guard drag != nil
                    || abs(value.translation.width) > abs(value.translation.height)
                else { return }
                drag = value.translation.width
            }
            .onEnded { value in
                guard drag != nil else { return }
                let opens = sidebarDrawerSettlesOpen(
                    isOpen: isOpen,
                    predictedTranslation: value.predictedEndTranslation.width,
                    width: width
                )
                drag = nil
                withAnimation(slide) { isOpen = opens }
            }
    }
}
#endif
