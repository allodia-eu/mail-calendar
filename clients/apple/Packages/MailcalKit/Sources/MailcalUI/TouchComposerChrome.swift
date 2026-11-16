// The touch composer's one scroll (`docs/composer-layout.md`, rules 2 and 7): the editor's own page,
// with the header and the files drawn inside the web view's scroll view, above the message.
//
// Inside rather than over it: a header laid over the web view would need every drag on it handed
// to the page, as Android does, while a subview of the scroll view moves with the page by itself,
// stops where the page stops, and takes a drag the way the message does. The page keeps the
// header's height clear at its top (`setComposerTopInset`), and the bundle's toolbar rides on the
// keyboard (`useNativeComposerChrome`), which is why the web view's frame ends there.

#if os(iOS)
import SwiftUI
import UIKit
import WebKit

struct TouchComposerChrome<Header: View>: UIViewRepresentable {
    let editor: RichComposerEditor
    @ViewBuilder let header: () -> Header

    func makeCoordinator() -> HeaderHost { HeaderHost() }

    func makeUIView(context: Context) -> WKWebView {
        let view = editor.mount()
        context.coordinator.install(in: view.scrollView, header: hostedHeader)
        followScheme(context)
        return view
    }

    func updateUIView(_ uiView: WKWebView, context: Context) {
        context.coordinator.update(hostedHeader)
        followScheme(context)
    }

    private func followScheme(_ context: Context) {
        editor.host.setDark(context.environment.colorScheme == .dark)
    }

    /// The header as the scroll view hosts it: at the top of the frame it is given, its height
    /// reported to the page, and the recipient suggestions drawn over it, because a suggestion
    /// list reaches its layer through SwiftUI preferences, which do not leave a hosting controller.
    private var hostedHeader: AnyView {
        let editor = editor
        return AnyView(
            header()
                // At its own height, as a scroll view would lay it out: the room it is hosted in is
                // the page's, and a field offered that much would stretch to fill it.
                .fixedSize(horizontal: false, vertical: true)
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                    editor.setTopInset($0)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
                .recipientSuggestionLayer()
        )
    }

    /// Holds the hosted header in the web view's scroll view, over the top of the page, as wide as
    /// the page and as tall as what the page shows, so a suggestion list has room to open over the
    /// message.
    @MainActor
    final class HeaderHost {
        private let container = PassThroughView()
        private var hosting: UIHostingController<AnyView>?
        private var observations: [NSKeyValueObservation] = []

        func install(in scrollView: UIScrollView, header: AnyView) {
            let hosting = UIHostingController(rootView: header)
            hosting.view.backgroundColor = .clear
            // The header scrolls under the navigation bar and over the keyboard with the page; a
            // hosting controller that kept to the safe area would push it back into view instead.
            hosting.safeAreaRegions = []
            hosting.view.translatesAutoresizingMaskIntoConstraints = false
            container.addSubview(hosting.view)
            NSLayoutConstraint.activate([
                hosting.view.topAnchor.constraint(equalTo: container.topAnchor),
                hosting.view.leadingAnchor.constraint(equalTo: container.leadingAnchor),
                hosting.view.trailingAnchor.constraint(equalTo: container.trailingAnchor),
                hosting.view.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            ])
            container.passesThrough = hosting.view
            // A hosting controller outside the view-controller tree draws its SwiftUI views and
            // none of the UIKit ones they wrap, which every text field is; so it is made a child of
            // the controller the web view ends up under, as soon as there is one.
            container.onWindow = { [weak self] view in self?.adopt(near: view) }
            scrollView.addSubview(container)
            self.hosting = hosting
            let resize: (UIScrollView) -> Void = { [weak self] scrollView in
                MainActor.assumeIsolated { self?.fit(to: scrollView) }
            }
            observations = [
                scrollView.observe(\.bounds) { view, _ in resize(view) },
                scrollView.observe(\.contentSize) { view, _ in resize(view) },
            ]
            fit(to: scrollView)
        }

        private func adopt(near view: UIView) {
            guard let hosting, hosting.parent == nil else { return }
            var responder: UIResponder? = view.next
            while let next = responder, !(next is UIViewController) { responder = next.next }
            guard let parent = responder as? UIViewController else { return }
            parent.addChild(hosting)
            hosting.didMove(toParent: parent)
        }

        func update(_ header: AnyView) {
            hosting?.rootView = header
        }

        private func fit(to scrollView: UIScrollView) {
            let frame = CGRect(
                x: 0,
                y: 0,
                width: scrollView.bounds.width,
                height: max(scrollView.bounds.height, scrollView.contentSize.height)
            )
            if container.frame != frame { container.frame = frame }
            // Above the page WebKit draws, which it may re-add beneath its scroll view's subviews.
            scrollView.bringSubviewToFront(container)
        }
    }

    /// A view a touch passes through wherever the header draws nothing, to the message underneath.
    final class PassThroughView: UIView {
        weak var passesThrough: UIView?
        var onWindow: ((UIView) -> Void)?

        override func didMoveToWindow() {
            super.didMoveToWindow()
            if window != nil { onWindow?(self) }
        }

        override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
            let hit = super.hitTest(point, with: event)
            return hit === self || hit === passesThrough ? nil : hit
        }
    }
}
#endif
