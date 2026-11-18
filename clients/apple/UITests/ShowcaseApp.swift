// Launching and steering the app under test.
//
// Every test in this bundle but `AccountPageTests` drives the in-memory **showcase** dataset. It
// needs no network, no Docker harness and no stored account. `AccountPageTests` says why it is the
// exception.
//
// The language is pinned with `-AppleLanguages`, the argument the screenshot runs already use
// (`scripts/dev/showcase.sh`), so a Dutch Mac and a CI runner drive the same English chrome. The
// strings asserted in the tests are written out rather than read back from the app: a test that
// asks the app what it says and then checks it says that cannot fail.

import UIKit
import XCTest

@MainActor
enum ShowcaseApp {
    static let timeout = App.timeout

    /// Launches on the showcase dataset, in English, with its test agent listening.
    ///
    /// `hooks` carries the `MAILCAL_*` launch hooks (`docs/debugging.md`). Reaching a screen
    /// through one is deterministic where tapping through the UI is not, so prefer a hook wherever
    /// the app offers one.
    static func launch(_ hooks: [String: String] = [:]) throws -> App {
        try launch(hooks.merging(["MAILCAL_SHOWCASE": "en"]) { current, _ in current }, args: [])
    }

    /// Launches with exactly `environment`, and the agent's socket.
    static func launch(_ environment: [String: String], args: [String]) throws -> App {
        let input = XCUIApplication()
        let agent = Agent(path: Agent.socketPath)
        input.launchArguments = ["-AppleLanguages", "(en)"] + args
        input.launchEnvironment = environment.merging(["MAILCAL_TEST_AGENT": agent.path]) { $1 }
        input.launch()
        XCTAssertTrue(input.wait(for: .runningForeground, timeout: timeout), "the app did not launch")
        try agent.connect(timeout: timeout)
        return App(input: input, agent: agent)
    }

    /// Brings the mailbox up, wherever the app happens to have started.
    ///
    /// The app restores the surface it was last on (`@SceneStorage`) and a test process launches it
    /// many times, so a test that assumes a starting screen is a test whose result depends on the
    /// test before it. Pressing the Mail tab while already on it pops back to the list, which is
    /// where the tests want to start anyway. An iPad has no tab bar: the mailbox is the split view's
    /// own column.
    static func showMailbox(_ app: App) throws {
        if !pad {
            try tab("Mail", in: app)
        }
        try app.wait(.button("New Mail"), "the mailbox did not come up")
    }

    static var pad: Bool { UIDevice.current.userInterfaceIdiom == .pad }

    /// The mailbox bar's button that brings the folders: the iPhone's drawer, the iPad's sidebar.
    static var folders: String { pad ? "Show Sidebar" : "Folders" }

    /// Goes to the surface called `name`: a tab on the iPhone's tab bar, an entry in the iPad's
    /// sidebar.
    ///
    /// An upright iPad keeps its sidebar hidden and lays it over the columns when shown, so it is
    /// shown for the press and hidden again, leaving the surface as a person would then see it.
    static func tab(_ name: String, in app: App) throws {
        guard pad else {
            let bar = try app.wait(Filter().type("UITabBar"), "there is no tab bar")
            try app.press(.button(name).under(bar), "the tab bar has no \(name)")
            return
        }
        let hidden = try app.exists(.button("Show Sidebar"))
        if hidden {
            try app.press(.button("Show Sidebar"))
        }
        try app.press(.button(name), "the sidebar has no \(name)")
        // Each column's bar carries its own Hide Sidebar, and any one of them hides it.
        if hidden, let hide = try app.waitAll(.button("Hide Sidebar")).first {
            try app.activate(hide)
            try app.wait(.button("Show Sidebar"), "the sidebar stayed open")
        }
    }

    /// Back, from a pushed screen to the one it was pushed from.
    ///
    /// A finger on the bar's back button: UIKit's back button element takes no accessibility
    /// activation, and VoiceOver reaches it only by tapping where it is drawn.
    static func back(_ app: App) throws {
        try app.tap(app.wait(.id("BackButton"), "there is no way back"))
    }

    /// Tells the simulator a hardware keyboard is attached, from the mailbox, before a test that
    /// sends chords.
    ///
    /// ⚠️ The first chord of a run is not delivered: it only attaches the keyboard. Which test sends
    /// that chord depends on which classes run and in what order, so a chord test that does not
    /// spend it first fails when run alone and passes in the full suite. ⌘A on the mailbox outside
    /// Select is the chord to spend, because delivered it does nothing
    /// (`MessageSelectionTests.testCommandAInSelectModePicksEveryRow`). It is not always enough on
    /// an iPad, which is what `App.chord` is for.
    static func attachHardwareKeyboard(_ app: App) {
        app.input.typeKey("a", modifierFlags: .command)
    }

    /// A message row on screen whose subject, sender or preview mentions `text`.
    ///
    /// A list row's own label is not the subject: the subject, the sender and the preview are each
    /// a text **inside** the cell. The folder drawer's rows are cells too, but stay off the left
    /// edge while it is shut, and the agent leaves what is not on screen out.
    static func row(mentioning text: String) -> Filter {
        Filter().type("UICollectionViewCell").has(.textLike(text))
    }

    /// Opens the first message in the mailbox, from wherever the app started.
    ///
    /// ⚠️ By tapping the row, **not** through the `MAILCAL_OPEN_FIRST` launch hook. The hook fires
    /// once, as the rows arrive, against whatever destination the restored scene put the app on:
    /// with the calendar restored it opens nothing, and a test built on it passes or fails on which
    /// test ran before it.
    static func openFirstMessage(_ app: App) throws {
        try showMailbox(app)
        let rows = try app.waitAll(Filter().type("UICollectionViewCell").has(.role("AXStaticText")),
                                   "the mailbox drew no message rows to open")
        let top = rows.min { ($0.frame?.minY ?? .infinity) < ($1.frame?.minY ?? .infinity) }!
        try app.tap(top)
    }

    /// Opens the message whose row mentions `text`, from wherever the app started; by the row
    /// rather than a launch hook, for the reason `openFirstMessage` gives.
    static func openMessage(containing text: String, in app: App) throws {
        try showMailbox(app)
        let because = "the mailbox drew no row mentioning '\(text)'"
        var found = try app.wait(row(mentioning: text), because)
        // A row low in the list can lie under the iPhone's floating tab bar, which would take the
        // tap: a swipe brings the end of the list up above it. A swipe rather than the agent's
        // scroll, which a SwiftUI list does not take.
        if let bar = try app.query(Filter().type("UITabBar")).first?.frame,
           let frame = found.frame, frame.maxY > bar.minY {
            app.input.swipeUp()
            found = try app.wait(row(mentioning: text), because)
        }
        try app.tap(found)
    }

    /// The first line of the message in an open composer: just under the header's action row,
    /// where the message starts on every size (docs/composer-layout.md). A point a fraction of the
    /// way down the web view misses it on an iPad, whose sheet is tall enough to put that point
    /// below the text, where a tap focuses nothing.
    static func composerBody(_ app: App) throws -> XCUICoordinate {
        let attach = try app.wait(.button("Attach files"), "the composer has no action row")
        let row = try XCTUnwrap(attach.frame, "the composer's action row has no frame")
        return app.input.coordinate(withNormalizedOffset: .zero)
            .withOffset(CGVector(dx: row.minX + 100, dy: row.maxY + 24))
    }

    /// Whether the button called `name` is on screen and a person could press it.
    static func reachable(_ name: String, in app: App) throws -> Bool {
        try app.query(.button(name)).contains { $0.enabled }
    }

    /// The on-screen order of the buttons labelled `labels`, left to right, as the reader meets
    /// them.
    ///
    /// Measured off the frames rather than read out of the tree: the rule under test is about what
    /// the eye meets first, and tree order is not evidence of that. The folder drawer keeps its
    /// rows while it is shut, parked off screen, so a mailbox named Archive would be a second
    /// `Archive` on every reading screen; the agent leaves what is not on screen out.
    static func leftToRight(_ labels: [String], in app: App) throws -> [String] {
        try labels.flatMap { try app.query(.button($0)) }
            .compactMap { element in element.frame.map { (element.label, $0.midX) } }
            .sorted { $0.1 < $1.1 }
            .map(\.0)
    }
}
