// The chrome both account-setup roots sit in, and the card that holds their fields.
//
// It exists because the two roots (`AccountSetupDetectView` and `AccountSetupView`) each carried
// their own copy of the frame stack, with a comment on each saying the two "must lay out
// identically", a rule with nothing enforcing it. They now share one scaffold, so they cannot
// drift.
//
// The layout deliberately mirrors `WelcomeView`, the screen immediately before this one: a
// width-capped column, centred in a ScrollView with a `minHeight` floor. Setup used to pin itself
// top-leading instead, so a user went from a composed, centred welcome to a form crushed into the
// corner of an iPad with most of the screen empty beside it.
//
// Fields sit in `SetupCard` rather than a SwiftUI `Form`. A `Form` nested inside a `VStack` is an
// inset-grouped list that claims every available point of height and insets its own rows: on an
// 11-inch iPad that rendered a ~1,400pt grey slab around ~500pt of fields, put the title, the card
// and the rows on three different left edges, and stranded the explanatory footnote far below the
// fields it explains.

import SwiftUI

/// The shared setup chrome: a width-capped column, centred and scrollable on iOS/iPadOS, and the
/// fixed card macOS has always used.
struct SetupScaffold<Content: View>: View {
    @ViewBuilder var content: () -> Content

    var body: some View {
        #if os(macOS)
        VStack(alignment: .leading, spacing: 14) {
            content()
        }
        .padding(20)
        .frame(width: 460)
        #else
        // `minHeight: proxy.size.height` is what centres it: a ScrollView sizes its content to
        // itself, so without a floor there is no free space to centre within. Content that
        // outgrows the screen, the detected-settings card, a large accessibility font, the
        // keyboard, simply exceeds the floor and scrolls, which the old fixed layout could not do
        // at all (it clipped).
        GeometryReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    content()
                }
                .frame(maxWidth: 480, alignment: .leading)
                .padding(24)
                .frame(maxWidth: .infinity, minHeight: proxy.size.height)
            }
            .scrollDismissesKeyboard(.interactively)
        }
        #endif
    }
}

/// A titled group of setup fields. Replaces a nested `Form` section: it wraps its rows in a
/// `GroupBox`, the same card `WelcomeView` puts its consent question in, so the card's edge and
/// the title above it share one left edge, and the group is only as tall as its contents.
struct SetupCard<Content: View>: View {
    var title: String?
    var systemImage: String?
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let title {
                HStack(spacing: 6) {
                    if let systemImage {
                        Image(systemName: systemImage)
                    }
                    Text(title)
                }
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(.secondary)
            }
            GroupBox {
                VStack(alignment: .leading, spacing: 10) {
                    content()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(4)
            }
        }
    }
}

/// The setup screens' footer: the actions, trailing-aligned, with a full-width hairline above them
/// so they read as the end of the form rather than as controls floating in the empty space the old
/// layout left below the fields.
struct SetupFooter<Content: View>: View {
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(spacing: 12) {
            Divider()
            HStack(spacing: 12) {
                Spacer()
                content()
            }
        }
        .padding(.top, 4)
    }
}

extension View {
    /// A setup text field: the keyboard/content-type configuration for what it holds, plus the
    /// bordered style. The style is explicit because these fields no longer sit in a `Form`, which
    /// is what used to draw their chrome.
    func setupField(_ kind: TextFieldKind) -> some View {
        fieldConfig(kind).textFieldStyle(.roundedBorder)
    }
}
