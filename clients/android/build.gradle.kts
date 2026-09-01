// Root build script: declares the plugin versions, applied per-module in app/build.gradle.kts.
// AGP 9.3.0 + Kotlin 2.2.20 are the versions verified against the installed Gradle 9.5.
//
// Kotlin is compiled by AGP's *built-in* Kotlin (android.builtInKotlin=true, required by
// android.newDsl=true, the standalone org.jetbrains.kotlin.android plugin casts the extension to
// the removed BaseExtension and can't be applied). AGP 9.3.0 bundles the Kotlin toolchain, so no
// org.jetbrains.kotlin.android plugin is declared here; only the Compose compiler stays a separate,
// version-locked plugin.
plugins {
    id("com.android.application") version "9.3.2" apply false
    // Kotlin 2.x splits the Compose compiler into its own plugin, version-locked to Kotlin.
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.10" apply false
}
