// The attempts a setup sign-in button starts, and when the person is owed a way back from one
// (docs/onboarding.md, "Nobody may be stranded on it"; docs/provider-oauth.md rule 13). A browser
// the person closed without signing in may never answer, so nothing here waits for it to.

/// One sign-in button's attempts. A newer attempt supersedes the one in flight, and an attempt the
/// person escaped from is retired: whatever it answers later belongs to no screen.
struct SignInAttempts: Equatable {
    /// How long a hop runs before the screen owes the person a way back. The contract caps this at
    /// ten seconds; eight matches Android and Linux. An ordinary hop has the browser in front of
    /// somebody inside a second, and a button drawn for that one is noise on every sign-in that
    /// works.
    static let escapeAfter = Duration.seconds(8)

    private var started = 0
    /// The attempt in flight; `nil` when none is.
    private(set) var current: Int?
    /// Whether the attempt in flight has outlasted `escapeAfter`, so the way back is drawn.
    private(set) var escapable = false

    var signingIn: Bool { current != nil }

    /// Starts an attempt, superseding the one in flight, and names it.
    mutating func begin() -> Int {
        started &+= 1
        current = started
        escapable = false
        return started
    }

    /// `attempt` has run for `escapeAfter`. Only the attempt in flight earns the way back: an older
    /// one's clock must not draw it for a newer one.
    mutating func outlasted(_ attempt: Int) {
        if current == attempt { escapable = true }
    }

    /// The person took the way back: the attempt in flight is retired and the button returns to
    /// its offer, saying nothing.
    mutating func escape() {
        current = nil
        escapable = false
    }

    /// `attempt` answered. Whether its outcome is still the screen's to show: `false` for an
    /// attempt superseded or escaped from, whose answer is dropped.
    mutating func finish(_ attempt: Int) -> Bool {
        guard current == attempt else { return false }
        current = nil
        escapable = false
        return true
    }
}
