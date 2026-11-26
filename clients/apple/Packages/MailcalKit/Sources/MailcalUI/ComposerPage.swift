// The page an editor's message is written on (docs/reading-dark-mode.md, "Writing in a dark app"):
// the reading view's, decided by the same rule, for the composer and the signature editor alike.
// The model is out of an editor's reach, so whoever mounts one hands it the reader's preference
// and the core's door for the toolbar's sun or moon through the environment.

import MailcalBindings
import SwiftUI

extension MessageScheme {
    /// The page a message is drawn on: dark exactly when the app is and the reader left dark
    /// messages on.
    static func of(appDark: Bool, darkMessages: Bool) -> MessageScheme {
        appDark && darkMessages ? .dark : .light
    }
}

/// What an editor needs from the model to put its message on the reading page.
struct ComposerPageSource {
    /// The reader's dark-messages preference. On, the product default, until the core is read.
    var darkMessages = true
    /// Hands the core a message the editor posted, answering whether it was the toolbar's sun or
    /// moon (`composerBackgroundChoice`).
    var choosesBackground: @MainActor (String) -> Bool = { _ in false }
}

extension EnvironmentValues {
    @Entry var composerPage = ComposerPageSource()
}

extension View {
    /// Puts every editor inside on the page `model`'s reading view would draw.
    func composerPage(_ model: MailboxModel) -> some View {
        environment(
            \.composerPage,
            ComposerPageSource(
                darkMessages: model.displaySettings.darkMessageBackground,
                choosesBackground: { model.app?.composerBackgroundChoice(message: $0) ?? false }
            )
        )
    }
}
