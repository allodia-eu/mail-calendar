// The core's split and join for an address with no IPv6 literal in it, standing in for the FFI
// this suite does not load. The rule itself, brackets included, is the core's and is tested in
// `mailcal-account`; what these suites pin is what a form does with its answer.
package eu.allodia.mailcal

internal val plainAddressing = ServerAddressing(
    split = { address ->
        val trimmed = address.trim()
        val at = trimmed.indexOf(':')
        val port = if (at < 0) null else trimmed.substring(at + 1)
        if (port == null || !port.all { it in '0'..'9' }) trimmed to "" else trimmed.substring(0, at) to port
    },
    join = { host, port ->
        if (host.isBlank() || port.isBlank()) host.trim() else "${host.trim()}:${port.trim()}"
    },
)
