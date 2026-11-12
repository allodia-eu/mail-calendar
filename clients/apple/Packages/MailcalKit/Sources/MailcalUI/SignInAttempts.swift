// The attempts a setup sign-in button starts (docs/onboarding.md, "Nobody may be stranded on it";
// docs/provider-oauth.md rule 13). A browser the person closed without signing in may never
// answer, so nothing here waits for it to.

/// One sign-in button's attempts. A newer attempt supersedes the one in flight, and an attempt the
/// person escaped from is retired: whatever it answers later belongs to no screen.
struct SignInAttempts: Equatable {
    private var started = 0
    /// The attempt in flight; `nil` when none is.
    private(set) var current: Int?

    var signingIn: Bool { current != nil }

    /// Starts an attempt, superseding the one in flight, and names it.
    mutating func begin() -> Int {
        started &+= 1
        current = started
        return started
    }

    /// The person took the way back: the attempt in flight is retired and the button returns to
    /// its offer, saying nothing.
    mutating func escape() {
        current = nil
    }

    /// `attempt` answered. Whether its outcome is still the screen's to show: `false` for an
    /// attempt superseded or escaped from, whose answer is dropped.
    mutating func finish(_ attempt: Int) -> Bool {
        guard current == attempt else { return false }
        current = nil
        return true
    }
}
