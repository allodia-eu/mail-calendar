// The page an editor's message is written on (docs/reading-dark-mode.md, "Writing in a dark app"):
// decided as the reading view decides it, named before the editor's document runs and again after,
// and the editor's colours and sun/moon answered before anything else reads a message. What the
// page and each moved colour are is the core's, tested there.

import MailcalBindings
import Testing
import WebKit

@testable import MailcalUI

@Suite struct ComposerPageTests {
    @Test func thePageIsDarkExactlyWhenTheAppIsAndDarkMessagesAreOn() {
        #expect(MessageScheme.of(appDark: true, darkMessages: true) == .dark)
        #expect(MessageScheme.of(appDark: true, darkMessages: false) == .light)
        #expect(MessageScheme.of(appDark: false, darkMessages: true) == .light)
        #expect(MessageScheme.of(appDark: false, darkMessages: false) == .light)
    }

    @Test func namesTheChromeTheAppsSchemeAndThePageTheReadingViews() {
        let dark = EditorHost.schemeScript(dark: true, page: .dark)
        #expect(dark == #"window.setComposerScheme("dark");"# + composerPageScript(scheme: .dark))
        // A reader who keeps messages light writes on a light page in a dark app.
        let keptLight = EditorHost.schemeScript(dark: true, page: .light)
        #expect(keptLight == #"window.setComposerScheme("dark");"# + composerPageScript(scheme: .light))
        let light = EditorHost.schemeScript(dark: false, page: .light)
        #expect(light == #"window.setComposerScheme("light");"# + composerPageScript(scheme: .light))
    }

    @Test func handsBothOverBeforeTheDocumentRuns() {
        let preload = EditorHost.preloadScript(dark: true, page: .dark)
        #expect(preload == #"window.composerScheme = "dark";"# + composerPagePreloadScript(scheme: .dark))
        let keptLight = EditorHost.preloadScript(dark: true, page: .light)
        #expect(keptLight == #"window.composerScheme = "dark";"# + composerPagePreloadScript(scheme: .light))
    }
}

@Suite @MainActor struct ComposerPageViewTests {
    /// Until the editor's document paints, what shows is the web view's own background, white,
    /// unless it draws none and the app's surface shows through.
    @Test func anEditorsWebViewDrawsNoBackgroundOfItsOwn() {
        let view = EditorHost(fallbackHTML: "<!doctype html>").mount()
        #if os(macOS)
        #expect(view.value(forKey: "drawsBackground") as? Bool == false)
        #else
        #expect(!view.isOpaque)
        #expect(view.backgroundColor == .clear)
        #expect(view.scrollView.backgroundColor == .clear)
        #endif
    }
}

@Suite @MainActor struct ComposerPageChannelTests {
    private let colours =
        ##"{"id":4,"request":{"colours":{"scheme":"dark","pieces":[{"ink":"#000000"}]}}}"##
    private let background = #"{"id":5,"request":{"background":{"scheme":"light"}}}"#
    private let caret = #"{"id":0,"request":{"caret":{"top":1,"bottom":2}}}"#

    @Test func aColoursRequestIsAnsweredAsTheCoreWroteItAndGoesNoFurther() {
        let channel = ComposerHostChannel()
        var ran: [String] = []
        channel.run = { ran.append($0) }
        var chosen: [String] = []
        channel.choosesBackground = { chosen.append($0); return false }
        var handed: [String] = []
        channel.onMessage = { handed.append($0) }
        channel.receive(colours)
        #expect(ran == [composerColoursAnswerScript(message: colours)].compactMap { $0 })
        #expect(ran.count == 1)
        #expect(chosen.isEmpty, "a colours request is not the sun or moon")
        #expect(handed == [colours], "every message still reaches the report")
    }

    @Test func theSunOrMoonGoesToTheCoreAndNoFurther() {
        let channel = ComposerHostChannel()
        var ran: [String] = []
        channel.run = { ran.append($0) }
        var chosen: [String] = []
        channel.choosesBackground = { chosen.append($0); return $0 == background }
        var carets = 0
        channel.onCaret = { _ in carets += 1 }
        channel.receive(background)
        channel.receive(caret)
        #expect(chosen == [background, caret])
        #expect(ran.isEmpty)
        #expect(carets == 1, "a message the core did not take is still parsed")
    }
}
