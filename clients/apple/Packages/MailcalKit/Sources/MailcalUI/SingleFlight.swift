// A value made once and shared, by a job that runs at most once at a time.
//
// The first caller starts the job; a caller arriving while it runs waits for that run rather than
// starting another; every caller after it is answered from what the job made. A run that makes
// nothing is not kept, so the next caller tries again.

import Foundation

@MainActor
final class SingleFlight<Value: AnyObject> {
    typealias Completion = @MainActor (Value?) -> Void

    private let job: (@escaping Completion) -> Void
    private var made: Value?
    /// The callers waiting on the run under way, `nil` when none is.
    private var waiting: [Completion]?

    /// - Parameter job: makes the value and calls back with it, or with `nil` when it could not.
    init(job: @escaping (@escaping Completion) -> Void) {
        self.job = job
    }

    /// Answers with the value, starting the job only when it has not made one and is not running.
    /// Always asynchronous, so a caller sees the same order whether or not the value was made.
    func value(_ completion: @escaping Completion) {
        if let made {
            Task { @MainActor in completion(made) }
            return
        }
        if waiting != nil {
            waiting?.append(completion)
            return
        }
        waiting = [completion]
        // Held until the run answers, so every caller waiting on it is answered.
        job { value in
            self.made = value
            let callers = self.waiting ?? []
            self.waiting = nil
            for caller in callers {
                caller(value)
            }
        }
    }
}
