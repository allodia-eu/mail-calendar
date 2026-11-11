// The test agent: a socket a test, a script or a coding agent drives the running app through. It
// speaks the protocol every client's agent shares (docs/test-agent.md): one JSON object per line
// each way.
//
// Compiled into a debug build only, and started only when `MAILCAL_TEST_AGENT` names the socket to
// listen on, so no installed copy answers. The socket is created mode 0600 in a directory only its
// owner can enter. On the iOS Simulator the path is on the Mac, whose filesystem the simulator
// shares; a physical device has no such path and stays on XCUITest.
//
// It reads the app's own accessibility objects in process, on the main thread (NSAccessibility on
// macOS, UIAccessibility on iOS), so `text`, `id`, `role`, `enabled` and `bounds` are what VoiceOver
// is given. It acts through the accessibility actions VoiceOver performs (press, increment, set a
// value), never a synthesised event or a coordinate, so it refuses what a person could not do.

#if DEBUG
import Foundation
import Synchronization

enum TestAgent {
    /// The variable that names the socket. Read here and nowhere else.
    nonisolated static let switchName = "MAILCAL_TEST_AGENT"

    /// Every change signal the core has sent, counted. A settle that saw this move has seen the core
    /// still working, however still the screen looked.
    nonisolated static let surfaces = Atomic<UInt64>(0)

    /// Set by `quit`, and acted on once its reply is written.
    nonisolated static let quitAsked = Atomic<Bool>(false)

    nonisolated private static let started = Atomic<Bool>(false)

    /// Called for each change signal the core sends, from whichever thread sends it.
    nonisolated static func surfaceChanged() {
        surfaces.add(1, ordering: .relaxed)
    }

    /// Starts listening when `MAILCAL_TEST_AGENT` names a socket, and does nothing otherwise.
    @MainActor
    static func start() {
        guard let path = ProcessInfo.processInfo.environment[switchName], !path.isEmpty,
              started.compareExchange(expected: false, desired: true, ordering: .relaxed).exchanged
        else { return }
        do {
            let listener = try bindSocket(at: path)
            AgentPlatform.prepare()
            say("listening on \(path)")
            let thread = Thread { serveForever(listener) }
            thread.name = "test agent"
            thread.start()
        } catch {
            say("cannot listen on \(path) (\(error))")
        }
    }

    struct SocketError: Error, CustomStringConvertible {
        let description: String
    }

    /// Binds the socket, replacing one an earlier run left behind, and leaves it to its owner alone.
    nonisolated static func bindSocket(at path: String) throws -> Int32 {
        let parent = (path as NSString).deletingLastPathComponent
        if !parent.isEmpty {
            try FileManager.default.createDirectory(
                atPath: parent, withIntermediateDirectories: true)
            guard chmod(parent, 0o700) == 0 else { throw failed("chmod \(parent)") }
        }
        guard unlink(path) == 0 || errno == ENOENT else { throw failed("unlink \(path)") }
        var address = sockaddr_un()
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw SocketError(description: "\(path) is too long for a socket's path")
        }
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: bytes) }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw failed("socket") }
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard bound == 0, chmod(path, 0o600) == 0, listen(fd, 4) == 0 else {
            let error = failed("bind \(path)")
            close(fd)
            throw error
        }
        return fd
    }

    nonisolated private static func failed(_ what: String) -> SocketError {
        SocketError(description: "\(what): \(String(cString: strerror(errno)))")
    }

    nonisolated static func say(_ line: String) {
        FileHandle.standardError.write(Data("test agent: \(line)\n".utf8))
    }

    /// Serves one client at a time; when it goes, the next connection is served.
    nonisolated private static func serveForever(_ listener: Int32) {
        while true {
            let client = accept(listener, nil, nil)
            guard client >= 0 else {
                if errno == EINTR { continue }
                say("stopped accepting (\(String(cString: strerror(errno))))")
                return
            }
            // A client that hangs up before its reply is written must not take the app with it.
            var on: Int32 = 1
            setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &on, socklen_t(MemoryLayout<Int32>.size))
            serve(client)
            close(client)
        }
    }

    /// Answers one client, a line at a time, until it hangs up.
    nonisolated private static func serve(_ client: Int32) {
        var pending = [UInt8]()
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while true {
            let count = read(client, &buffer, buffer.count)
            guard count > 0 else { return }
            pending.append(contentsOf: buffer[0..<count])
            while let newline = pending.firstIndex(of: UInt8(ascii: "\n")) {
                let line = String(decoding: pending[..<newline], as: UTF8.self)
                pending.removeSubrange(...newline)
                if line.allSatisfy(\.isWhitespace) { continue }
                guard writeAll(answer(line) + "\n", to: client) else { return }
                if quitAsked.exchange(false, ordering: .relaxed) {
                    Task { @MainActor in AgentPlatform.quit() }
                }
            }
        }
    }

    nonisolated private static func writeAll(_ text: String, to client: Int32) -> Bool {
        var bytes = Array(text.utf8)[...]
        while !bytes.isEmpty {
            let written = bytes.withUnsafeBytes { write(client, $0.baseAddress, $0.count) }
            if written < 0 && errno == EINTR { continue }
            guard written > 0 else { return false }
            bytes = bytes.dropFirst(written)
        }
        return true
    }

    /// Runs one request on the main thread, where every accessibility object lives, and waits for
    /// its reply.
    nonisolated private static func answer(_ line: String) -> String {
        let reply = Reply()
        let done = DispatchSemaphore(value: 0)
        Task { @MainActor in
            reply.text = await AgentProtocol.handle(line)
            done.signal()
        }
        done.wait()
        return reply.text
    }

    /// Carries a reply from the main thread back to the thread serving the client, which waits for
    /// it on a semaphore before reading it.
    private final class Reply: @unchecked Sendable {
        var text = ""
    }
}
#endif
