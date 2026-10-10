// `SingleFlight`: a value made once and shared, by a job that runs at most once at a time.
//
// What it guards is the composer's rule list (`ComposerRemoteBlock`), where a second run is a second
// compiled file kept mapped for the life of the process. The job here is a stand-in the test
// finishes by hand, so the run's timing is the test's to choose rather than WebKit's.

import Testing

@testable import MailcalUI

@MainActor
@Suite struct SingleFlightTests {

    private final class Made {}

    /// A job that counts its runs and finishes only when told to.
    @MainActor
    private final class Job {
        var runs = 0
        private var finish: [@MainActor (Made?) -> Void] = []

        func start(_ done: @escaping @MainActor (Made?) -> Void) {
            runs += 1
            finish.append(done)
        }

        func complete(with made: Made?) {
            let pending = finish
            finish = []
            for done in pending { done(made) }
        }
    }

    /// Asks for the value, recording the answer when it comes.
    @MainActor
    private final class Answer {
        var value: Made?
        var answered = false
    }

    private func ask(_ flight: SingleFlight<Made>) -> Answer {
        let answer = Answer()
        flight.value { made in
            answer.value = made
            answer.answered = true
        }
        return answer
    }

    @Test func callersArrivingDuringARunShareThatRun() {
        let job = Job()
        let flight = SingleFlight<Made>(job: job.start)
        let first = ask(flight)
        let second = ask(flight)
        #expect(job.runs == 1)

        let made = Made()
        job.complete(with: made)
        #expect(first.value === made)
        #expect(second.value === made)
    }

    @Test func aCallerAfterTheRunIsAnsweredFromIt() async {
        let job = Job()
        let flight = SingleFlight<Made>(job: job.start)
        _ = ask(flight)
        let made = Made()
        job.complete(with: made)

        let later = ask(flight)
        // Asynchronous even from what was made, as a compile is, so no caller depends on which.
        #expect(!later.answered)
        await Task.yield()
        #expect(later.value === made)
        #expect(job.runs == 1)
    }

    @Test func aRunThatMadeNothingIsTriedAgain() {
        let job = Job()
        let flight = SingleFlight<Made>(job: job.start)
        let failed = ask(flight)
        job.complete(with: nil)
        #expect(failed.answered)
        #expect(failed.value == nil)

        let retried = ask(flight)
        #expect(job.runs == 2)
        let made = Made()
        job.complete(with: made)
        #expect(retried.value === made)
    }
}
