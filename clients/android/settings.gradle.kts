// The Android client: a Jetpack Compose app rendering the mailbox-list snapshot the
// Rust core (mailcal-bindings, via the UniFFI `MailcalApp`) drives. Mirrors the macOS
// SwiftUI spike in ../macos. The eventual home is the allodia-clients repo; this lives
// here while the client lives in this repo.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
plugins {
    id("org.gradle.toolchains.foojay-resolver-convention") version "1.0.0"
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "Mailcal"
include(":app")
