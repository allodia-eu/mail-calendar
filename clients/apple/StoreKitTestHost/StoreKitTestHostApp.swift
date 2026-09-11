// An empty application, whose only job is to be a container for the StoreKit tests beside it.
//
// StoreKit resolves products, transactions and its own test session against an **application**:
// run from a bare `xctest` process it refuses every call with `SKInternalErrorDomain Code=3`, and
// hosted by the real mail app the runner never connects, because that app boots the whole core on
// launch and a test bundle waits on it. So the tests get an app that does nothing at all.
//
// It ships nowhere. Nothing depends on it but `StoreKitTests`, and `package.sh` builds the app
// target by name.

import SwiftUI

@main
struct StoreKitTestHostApp: App {
    var body: some Scene {
        // A `Settings` scene rather than a `WindowGroup`: it opens no window on launch, so a test
        // run neither draws anything nor steals focus from whoever is working.
        Settings {
            EmptyView()
        }
    }
}
