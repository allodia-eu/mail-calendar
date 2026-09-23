// The destinations the folder pane reaches that are not mail: Calendar, Contacts and Settings,
// pinned under the account trees.
//
// Split from Mailcal.Sidebar.swift, which draws the trees they sit beneath, so each file keeps one
// responsibility and both stay under the repo line limit. The pane's own rules are
// `docs/folder-pane.md`.

#if os(macOS)
import AppKit
#endif
import MailcalBindings
import SwiftUI

// What a pinned row leaves around its label, so its icon stands under the icons in the list, and
// how short the row may end up.
//
// The insets are measured against that list rather than derived, and both constants are per
// platform because the two toolkits disagree: a row's insets belong to the list's style, a bar
// underneath the list is not one of its rows, and SwiftUI publishes neither number. A pointer also
// hits what it is aimed at where a finger needs the platform's minimum target, which the padding
// alone does not reach. A change here is checked by looking at the pane.
#if os(macOS)
private let pinnedRowPadding = EdgeInsets(top: 8, leading: 20, bottom: 8, trailing: 20)
private let pinnedRowMinHeight: CGFloat = 28
#else
private let pinnedRowPadding = EdgeInsets(top: 10, leading: 34, bottom: 10, trailing: 20)
private let pinnedRowMinHeight: CGFloat = 44
#endif

/// A label whose glyph sits in a column of a fixed width, so the names beside three different
/// symbols start at the same place.
///
/// A `List` gives its rows that column; a stack of rows underneath one does not, and the gear, the
/// calendar and the two figures are each a different width, so the three names stepped in and out
/// by a few points each.
private struct PinnedRowLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 6) {
            configuration.icon.frame(width: 18)
            configuration.title
        }
    }
}

#if os(macOS)
/// The sidebar's own material, under the pinned rows.
///
/// The bar has to be opaque, because the tree scrolls beneath it and words read through a
/// see-through bar. Any fill would do that and every one of them is a slab of a different colour
/// in the middle of a vibrant pane, so this is the pane's own material rather than a colour
/// matched to it: a match would be right in one appearance and wrong in the other, and wrong again
/// behind a desktop picture.
private struct SidebarMaterial: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let view = NSVisualEffectView()
        view.material = .sidebar
        view.blendingMode = .behindWindow
        // Vibrancy follows the window, as the tree above does: a bar that stayed lit under an
        // inactive window would be the only part of the pane that had not gone quiet.
        view.state = .followsWindowActiveState
        return view
    }

    func updateNSView(_ nsView: NSVisualEffectView, context: Context) {}
}
#endif

extension ContentView {
    /// Calendar, Contacts and Settings, under the folder tree and out of its scroll.
    ///
    /// **Pinned, not the last rows of the list.** The tree takes whatever height is left over, so
    /// an account with a few dozen folders scrolls for as long as it likes and these three stay
    /// where the user last saw them. As a section at the end of the list they were a scroll to the
    /// bottom of every folder the user has, which is the bug this shape fixes; the Windows pane's
    /// footer items and the Linux bottom bar are the same decision in their own toolkits.
    ///
    /// `showsCalendarAndContacts` is what a platform answers with where its other surfaces live.
    /// The desktop and the iPad columns reach them from here, because the pane is the only
    /// navigation they have; the iPhone reaches them from its tab bar, so listing them here too
    /// would offer the same two destinations twice. Settings is pinned either way, it has no tab,
    /// and it must not be something the user has to remember a gesture to find.
    @ViewBuilder
    func sidebarDestinations(showsCalendarAndContacts: Bool) -> some View {
        VStack(spacing: 0) {
            Divider()
            if showsCalendarAndContacts {
                pinnedDestination(
                    title: L10n.nav_calendar(),
                    icon: "calendar",
                    selected: model.destination == .calendar
                ) { showCalendar() }
                pinnedDestination(
                    title: L10n.nav_contacts(),
                    icon: "person.2",
                    selected: model.destination == .contacts
                ) { showContacts() }
            }
            // ⌘, lives here, on the app's one Settings affordance. It used to hang off a second
            // gear in the message-list header; that button is gone, so the shortcut moved rather
            // than leaving the standard macOS key with nothing to open.
            //
            // Never selected: Settings opens over the app, it does not take the pane's surface.
            pinnedDestination(title: L10n.nav_settings(), icon: "gearshape", selected: false) {
                settingsCategory = .general
            }
            #if os(macOS)
            .keyboardShortcut(",", modifiers: .command)
            #endif
        }
        .labelStyle(PinnedRowLabelStyle())
        #if os(macOS)
        // Opaque, because on macOS the tree scrolls beneath the bar. iOS stacks the bar under the
        // tree instead (`sidebarList`), so there it has nothing to hide and no fill of its own.
        .background(SidebarMaterial())
        #endif
    }

    /// One pinned destination, drawn like the rows above it and highlighted the same way.
    @ViewBuilder
    private func pinnedDestination(
        title: String,
        icon: String,
        selected: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            sidebarRowLabel(title: title, icon: icon, unread: 0)
                .padding(pinnedRowPadding)
                .frame(minHeight: pinnedRowMinHeight)
                .background(selected ? Color.accentColor.opacity(0.2) : Color.clear)
        }
        .buttonStyle(.plain)
        #if os(macOS)
        .help(title)
        #endif
    }
}
