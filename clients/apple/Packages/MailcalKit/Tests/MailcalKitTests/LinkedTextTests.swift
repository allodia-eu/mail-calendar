// Sender text with its addresses as links (LinkedText.swift). Finding the addresses is the core's
// and is held by its Rust tests; what is Apple's is that the runs are appended as text, so nothing
// in them is parsed, and that only the core's target becomes a link.

import Foundation
import MailcalBindings
import Testing

@testable import MailcalUI

@Suite struct LinkedTextTests {

    @Test func runsAreTextAndOnlyTheAddressIsALink() throws {
        let url = "https://meet.example/abc"
        let value = LinkedText.attributed([
            LinkedText(text: "Join ", link: nil),
            LinkedText(text: url, link: url),
            LinkedText(text: " **bold** <b>x</b>", link: nil),
        ])

        #expect(String(value.characters) == "Join https://meet.example/abc **bold** <b>x</b>")

        var links: [URL] = []
        var linkedRuns: [String] = []
        for (link, range) in value.runs[\.link] {
            guard let link else { continue }
            links.append(link)
            linkedRuns.append(String(value[range].characters))
        }
        #expect(links == [try #require(URL(string: url))])
        #expect(linkedRuns == [url])
    }
}
