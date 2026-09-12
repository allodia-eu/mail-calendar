// What every window of this app shares.
//
// One object, held by the app entry point and handed to each scene, because the thing it holds is
// the thing there must be exactly one of: the model, and through it the `MailcalApp` that owns the
// engine and the SQLite store. A second model is a second connection to the same database
// (`docs/reading-window.md`), so a window is given this rather than being allowed to build its
// own.
//
// It is the package's one public handle on the model: the model itself stays internal, so the app
// target composes scenes without the sixty published properties behind them becoming API.

import SwiftUI

@MainActor
@Observable
public final class AppSession {
    /// The SwiftUI source of truth, driven by the Rust core.
    let model = MailboxModel()
    #if os(macOS)
    /// The drafts the open composer windows are writing. Host state: a message exists to the core
    /// only once it is sent.
    let drafts = DesktopDrafts()
    #endif

    public init() {}
}
