// The temp-file name an attachment is written under before the OS opens it.
//
// The extension decides which viewer the OS picks, Quick Look on iOS/iPadOS, the default handler
// on macOS, so a PDF whose part carried no extension opens in nothing at all. The presentation
// around it is UIKit, which `swift test` (macOS) cannot reach; the naming is the part that decides
// what the user sees, so it is asserted here.
//
// That leaves which surface Open raises, Quick Look, not the share sheet, provable only on a
// running client: boot the iPhone simulator against the harness (`scripts/dev/boot.sh iphone`),
// open a message carrying a PDF, tap Open, and `scripts/dev/control.sh iphone ui-dump` names Quick
// Look's own chrome (Markup / Search / Share) rather than an activity sheet.

import Testing

@testable import MailcalUI

@Suite("Attachment temp-file naming")
struct AttachmentFileNameTests {
    @Test("A name that already carries an extension is left alone")
    func keepsSuppliedExtension() {
        #expect(attachmentFileName(name: "report.pdf", mediaType: "application/pdf") == "report.pdf")
    }

    @Test("An extension-less name is typed from its media type")
    func typesFromMediaType() {
        #expect(attachmentFileName(name: "invoice", mediaType: "application/pdf") == "invoice.pdf")
        #expect(attachmentFileName(name: "scan", mediaType: "image/png") == "scan.png")
    }

    @Test("A nameless part still opens in the right viewer")
    func namelessPart() {
        #expect(attachmentFileName(name: "", mediaType: "application/pdf") == "attachment.pdf")
        #expect(attachmentFileName(name: "   ", mediaType: "image/jpeg") == "attachment.jpeg")
    }

    @Test("An unrecognised media type leaves the name as it is")
    func unknownMediaType() {
        #expect(attachmentFileName(name: "blob", mediaType: "application/x-not-a-type") == "blob")
        #expect(attachmentFileName(name: "blob", mediaType: "") == "blob")
    }

    @Test("Path separators are neutralised, and a name that is only dots is not a name")
    func sanitizesPathComponents() {
        #expect(attachmentFileName(name: "a/b\\c:d", mediaType: "") == "a_b_c_d")
        #expect(attachmentFileName(name: "..", mediaType: "application/pdf") == "attachment.pdf")
        #expect(attachmentFileName(name: ".", mediaType: "") == "attachment")
    }
}
