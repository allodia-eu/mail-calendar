// swift-tools-version:6.0
import PackageDescription

// The shared Apple layer. The Rust core ships as a prebuilt XCFramework (binary target); the
// generated UniFFI bindings, the view models, and the shared SwiftUI views layer on top. The
// app target (managed by XcodeGen) depends only on MailcalUI.
//
// Run Scripts/build-core.sh before building, it produces artifacts/Mailcal.xcframework and the
// generated sources under Sources/MailcalBindings (both git-ignored, rebuilt from Rust).
let package = Package(
    name: "MailcalKit",
    platforms: [.macOS(.v15), .iOS(.v18)],
    products: [
        .library(name: "MailcalUI", targets: ["MailcalUI"]),
        // Exposed so the headless MailcalVerify tool can drive the FFI without the UI layer.
        .library(name: "MailcalBindings", targets: ["MailcalBindings"]),
    ],
    targets: [
        // The Rust core, all Apple slices. Vends the `mailcal_bindingsFFI` C module.
        .binaryTarget(name: "MailcalFFI", path: "artifacts/Mailcal.xcframework"),
        // Generated UniFFI Swift + L10n (git-ignored; produced by build-core.sh).
        .target(name: "MailcalBindings", dependencies: ["MailcalFFI"]),
        // libresolv, wrapped so SystemMxResolver can send an MX query via the system resolver
        // for the autodetect MX fallback (the raw answer is parsed in Swift by DnsMessage).
        .systemLibrary(name: "CResolv", path: "Sources/CResolv"),
        // The shared client: view models + SwiftUI views + the Platform* shims (this is where
        // the `#if os()` divergences live). Model and views stay in one module, they are
        // tightly coupled through @Published state, so a Core/UI split would only add churn.
        .target(
            name: "MailcalUI",
            dependencies: ["MailcalBindings", "CResolv"],
            resources: [
                // The rich-composer editor bundle (copied from clients/composer by
                // build-core.sh), loaded via Bundle.module.
                .copy("composer"),
                // The welcome screen's art. `.process` (not `.copy`) so actool compiles the
                // catalog and the @1x/@2x/@3x variants resolve; it is reached as
                // `Image("WelcomeArt", bundle: .module)`, a bare `Image("…")` inside this
                // module would look in the *main* bundle and render blank.
                .process("Assets.xcassets"),
            ]
        ),
        // The client's own tests, on the JVM-equivalent of the Android suite: plain logic, no UI, no
        // simulator. What is tested here is what the CLIENT decides, the page↔date mapping, the
        // zoom clamps, the all-day overflow rule, the localised copy. The core's layout has its own
        // Rust tests; this is the multiplication and the arithmetic on top of it.
        .testTarget(name: "MailcalUITests", dependencies: ["MailcalUI"]),
    ],
    // The ported client is Swift-5 code (as it was under swiftc); compile the whole package in
    // Swift 5 language mode so Swift 6 strict-concurrency doesn't reject it wholesale.
    swiftLanguageModes: [.v5]
)
