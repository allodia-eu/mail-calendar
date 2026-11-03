// Root build script: declares the plugin versions, applied per-module in app/build.gradle.kts.
//
// Kotlin is compiled by AGP's *built-in* Kotlin (android.builtInKotlin=true, required by
// android.newDsl=true: the standalone org.jetbrains.kotlin.android plugin casts the extension to
// the removed BaseExtension and can't be applied), so no org.jetbrains.kotlin.android plugin is
// declared here.
plugins {
    id("com.android.application") version "9.5.0" apply false
    // The Compose compiler is a plugin of its own, and its version is the Kotlin version: it puts
    // its own kotlin-gradle-plugin on the build classpath, which outranks the older one AGP depends
    // on (`./gradlew buildEnvironment` shows the resolution).
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21" apply false
}
