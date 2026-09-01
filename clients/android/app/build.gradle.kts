import com.android.build.api.variant.HostTestBuilder
import java.util.Properties

plugins {
    // Kotlin is compiled by AGP's built-in Kotlin (see the root build script + gradle.properties);
    // no org.jetbrains.kotlin.android here. Only the Compose compiler is a separate plugin.
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

// The marketing version, single source of truth in the top-level /VERSION file (docs/versioning.md).
// The user-visible versionName IS that string; the Play versionCode is derived from it by the shared
// formula so both are fully computed and nothing here can drift from the other clients. rootProject is
// clients/android, so ../../VERSION resolves to the repo root. Bump with scripts/dev/bump-version.sh.
val marketingVersion = rootProject.file("../../VERSION").readText().trim()
val (verMajor, verMinor, verPatch) = marketingVersion.split(".").map(String::toInt)
// major*10^7 + minor*10^5 + patch*10^3 (+ a 0-999 build slot, unused here): 0.2.0 -> 200000,
// strictly above the old hardcoded 2 and far under Play's 2_100_000_000 ceiling (holds for major <= 209).
val derivedVersionCode = verMajor * 10_000_000 + verMinor * 100_000 + verPatch * 1_000

// `KEY=value` lines out of one of the repo's env-style files. Comments, blanks, `export ` and one
// pair of quotes are tolerated; nothing is interpolated. The twin of `parse_env_file` in
// scripts/dev/envfile.py and of the sed in scripts/dev/brand.sh.
fun readEnvFile(file: File): Map<String, String> =
    file.readLines()
        .map { it.trim().removePrefix("export ") }
        .filterNot { it.isEmpty() || it.startsWith("#") }
        .mapNotNull { line -> line.split("=", limit = 2).takeIf { it.size == 2 } }
        .associate { (name, value) -> name.trim() to value.trim().trim('"', '\'') }
        .filterValues { it.isNotEmpty() }

// One credential value, from the environment first and the repo's gitignored `.env` second, the
// same order and the same file the Rust build script reads, so Gradle and cargo are given the same
// client id. Blank counts as absent, because a CI run without access to the secrets sets the empty
// string rather than leaving the name unbound.
fun credentialValue(key: String): String? {
    System.getenv(key)?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
    val envFile = rootProject.file("../../.env").takeIf { it.exists() } ?: return null
    return readEnvFile(envFile)[key]
}

// One brand value, the app's name or its application id (docs/branding.md). The environment
// first, then Allodia's file when the checkout has one, then the neutral default that every
// checkout has. There is no "absent" case to handle: `branding/default.env` is the floor, so a
// build always has an identity, and in a checkout without `allodia.env` that identity is the
// unbranded one.
fun brandValue(key: String): String {
    System.getenv(key)?.trim()?.takeIf { it.isNotEmpty() }?.let { return it }
    for (name in listOf("allodia.env", "default.env")) {
        val file = rootProject.file("../../branding/$name").takeIf { it.exists() } ?: continue
        readEnvFile(file)[key]?.let { return it }
    }
    error("branding/default.env gives no $key, every build needs one, so it is not optional there.")
}

// The custom scheme the Google OAuth redirect comes back to. Google matches an Android redirect on
// the scheme alone, and the scheme IS the client id with its dotted components reversed:
// including the numeric project-number prefix, without which Google answers redirect_uri_mismatch.
//
// The core derives the same string from the same client id, so the manifest filter and the URI the
// browser is sent to cannot drift; keep this in step with `reversed_client_id_redirect` in
// crates/mailcal-oauth/src/credentials.rs. It has to be computed here as well because a manifest
// is not code: `android:scheme` takes a literal, and only a placeholder can fill one in at build
// time.
//
// A build carrying no Google registration, or one handed something that is not a Google client id
// gets the inert value below. The filter is then dead, which is exactly right: such a build never
// offers Google sign-in, and an *empty* placeholder would fail the manifest merger rather than
// build.
val googleRedirectScheme: String = credentialValue("MAILCAL_GOOGLE_ANDROID_CLIENT_ID")
    ?.takeIf { it.endsWith(".apps.googleusercontent.com") }
    ?.removeSuffix(".apps.googleusercontent.com")
    ?.takeIf { it.isNotEmpty() }
    ?.let { "com.googleusercontent.apps.$it" }
    ?: "com.googleusercontent.apps.unconfigured"

android {
    namespace = "eu.allodia.mailcal"
    compileSdk = 37

    defaultConfig {
        // The identity, injected rather than written here (docs/branding.md). `namespace` above is
        // the *source* package and deliberately does not follow it: it names the R class and the
        // generated BuildConfig, is invisible to the OS and to the user, and moving it would
        // rewrite every file in the client for a string nothing reads.
        applicationId = brandValue("MAILCAL_APP_ID")
        // minSdk 31 (Android 12): we deliberately support only modern Android, no legacy
        // devices. The cross-compiled cdylib targets a lower NDK platform, which stays
        // forward-compatible at a higher minSdk.
        minSdk = 31
        targetSdk = 37
        // Both derived from /VERSION above, never edit these literals (the version-sync check fails
        // if a hardcoded versionName creeps back). Change the version in /VERSION.
        versionCode = derivedVersionCode
        versionName = marketingVersion

        // Fills in the Google OAuth redirect filter in AndroidManifest.xml, see the derivation
        // above for why it cannot simply be written there.
        manifestPlaceholders["googleRedirectScheme"] = googleRedirectScheme

        // The launcher label, for the same reason: `android:label` takes a literal. The merger
        // substitutes into the parsed document, so the ampersand in the product name is escaped on
        // the way out and must not be escaped here.
        manifestPlaceholders["appName"] = brandValue("MAILCAL_APP_NAME")

        // Ship exactly the ABIs we cross-compile the cdylib for, 64-bit arm (every phone) and
        // 64-bit x86 (emulators, Chromebooks). Without this filter AGP packages whatever ABIs a
        // dependency's AAR happens to carry, and that is not cosmetic in either direction:
        //
        //  1. An ABI without `libmailcal_bindings.so` is a crash, not a fallback. JNA and Compose
        //     ship `.so`s for `armeabi-v7a` and `x86` too; packaging those produced an APK that
        //     installed on a 32-bit device and died at its first `System.loadLibrary`. minSdk is
        //     31 and the Play 64-bit requirement has been in force for years, so no reach is lost.
        //  2. Play rejects an upload whose native libraries are not 16 KB page aligned, and
        //     alignment is a property of each `.so`'s ELF LOAD segments, per-ABI, so one
        //     dependency can be aligned on arm64 and not on x86_64. That is exactly what bit us:
        //     JNA 5.14's `x86_64/libjnidispatch.so` aligned to 4 KB. JNA 5.19.1 aligns all of
        //     them to 16 KB, which is why the version below is a floor, not a preference, do
        //     not lower it.
        //
        // Asserted before every release by scripts/dev/check-android-native-libs.sh, which
        // build-release.sh runs on the packaged artifact: it fails on an unexpected ABI, on an ABI
        // missing our own cdylib, and on any LOAD segment under 16 KB. Play is a remote gate, and
        // a rejection costs a build number and a slow round-trip (AGENTS.md).
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    // The upload key, read from a gitignored `keystore.properties` beside this file:
    //
    //     storeFile=/absolute/path/to/upload.jks
    //     storePassword=…
    //     keyAlias=…
    //     keyPassword=…
    //
    // Absent, on a fresh checkout, and in CI, the release build still *builds*, unsigned, so the
    // shrinker and its keep rules are exercised on every machine. That matters more than it sounds:
    // R8 breakage in the JNA bindings is invisible until the release build runs (see
    // proguard-rules.pro), so a config only a release engineer can compile is a config nobody tests.
    val keystoreProperties = rootProject.file("app/keystore.properties").takeIf { it.exists() }
        ?.let { file -> Properties().apply { file.inputStream().use(::load) } }

    signingConfigs {
        if (keystoreProperties != null) {
            create("release") {
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        debug {
            isMinifyEnabled = false
        }
        release {
            // R8: shrink, optimise, obfuscate. This is not a size tweak, an unminified Compose
            // build is *several times slower* than a minified one, which is why the calendar's pinch
            // was being judged against Samsung's release build while running as a debug one.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            signingConfig = signingConfigs.findByName("release")
        }
    }

    // Give `stripReleaseDebugSymbols` an NDK it can resolve, so it actually runs.
    //
    // Without this it finds no strip tool and copies every library through unchanged, logging that
    // only at `verbose`, a task that reports success having done nothing. It is not the only such
    // path: AGP also copies through when the strip exits non-zero, and there is no setting that
    // turns either into a build failure, so the shipped symbol content has to be asserted rather
    // than assumed (build-release.sh does that).
    //
    // Pinning the version rather than taking whatever a machine happens to have is what makes two
    // builds of one commit agree. A machine without it downloads it once.
    ndkVersion = "30.0.14904198"

    // Our own cdylib is exempted, and only ours: build-release.sh decides its symbol content.
    //
    // AGP's strip is `--strip-unneeded`, which takes the symbol table with the DWARF and leaves
    // only the 438 dynamic symbols the loader needs, measured on a device, that turns every frame
    // of a Rust backtrace into `<unknown>`. We want the middle outcome AGP has no setting for:
    // DWARF gone, symbol table kept, which build-release.sh gets by building the library without
    // debug info at all. docs/logging.md → "A shipped Android stack names functions but not lines"
    // has the full reasoning and the numbers.
    //
    // `keepDebugSymbols` means "do not strip this file at all", it is checked before AGP even
    // looks for a strip tool, so the glob must name our library and nothing else. Widened to
    // `**/*.so` it would exempt JNA and androidx too, and then the NDK above would buy nothing.
    packaging {
        jniLibs {
            keepDebugSymbols += "**/libmailcal_bindings.so"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // Pin the JVM the unit tests RUN on, not just the bytecode they compile to.
    //
    // Without this, Gradle runs them on whatever JDK the daemon happens to be, 21 on a machine with
    // 21 installed, while CI's `setup-java` gives 17, and that is not a cosmetic difference. Under
    // Robolectric, `java.time` reads the locale data of the **host JDK**, so a Dutch abbreviated month
    // is `jul` on 21 and `jul.` on 17 (a newer CLDR dropped the full stop). A localisation test then
    // passes locally and fails in CI for no reason a reader could ever guess.
    //
    // Same principle as the pinned Rust toolchain (AGENTS.md): local and CI run the same thing, or
    // "green on my machine" means nothing.
    kotlin {
        jvmToolchain(17)
    }

    buildFeatures {
        compose = true
        // BuildConfig.VERSION_NAME / VERSION_CODE, both already derived from /VERSION above, and
        // the only way FileLog can stamp them without a Context (docs/logging.md).
        buildConfig = true
    }

    testOptions {
        unitTests {
            // Robolectric needs the merged resources + manifest to inflate a Context and resolve
            // the generated R.string catalog the L10n accessor reads.
            isIncludeAndroidResources = true
        }
    }

    sourceSets {
        getByName("main") {
            assets.directories.add("../../composer/dist")
        }
    }

    // The generated UniFFI binding (uniffi/mailcal_bindings/mailcal_bindings.kt) and the
    // cross-compiled cdylib (jniLibs/<abi>/libmailcal_bindings.so) both live under
    // src/main/, so the default source sets already pick them up, no extra wiring needed.
}

// ---- Generated sources ------------------------------------------------------------------------
//
// Two of this module's source files are *generated* from the Rust workspace and gitignored: the
// UniFFI binding (`src/main/java/uniffi/…`) and the typed L10n accessor + string resources, built
// from the shared inlang catalog. Both go stale the moment a Rust enum, an FFI record, or a
// message changes, and a stale one fails the Kotlin build with an error that looks nothing like
// its cause (or, worse, compiles against a name that has quietly changed meaning).
//
// So Gradle generates them itself, rather than every entry point being expected to remember:
// `./gradlew :app:test`, Android Studio, `build-and-run.sh`, `build-release.sh`, and CI now all
// regenerate by construction. That is the "a gate you have to remember is not a gate" rule from
// AGENTS.md applied to the one input the gate reads.
//
// Cheap when nothing changed: the tasks declare their Rust inputs and generated outputs, so Gradle
// skips them as UP-TO-DATE and no `cargo` process starts at all.
val repoRoot: File = rootProject.file("../..")
val hostCdylib: File = repoRoot.resolve(
    "target/debug/" + when {
        // uniffi-bindgen reads the exported metadata of the **host** library, not the
        // cross-compiled Android .so, which a JVM test never loads anyway.
        System.getProperty("os.name").startsWith("Mac") -> "libmailcal_bindings.dylib"
        System.getProperty("os.name").startsWith("Windows") -> "mailcal_bindings.dll"
        else -> "libmailcal_bindings.so"
    },
)

// Prefer rustup's own bin over bare `cargo`: Android Studio launches its Gradle daemon with a
// login-shell-free environment, where ~/.cargo/bin is routinely absent from PATH.
val cargoExecutable: String = File(System.getProperty("user.home"), ".cargo/bin/cargo")
    .takeIf { it.canExecute() }?.absolutePath ?: "cargo"

val cargoBuildBindings = tasks.register<Exec>("cargoBuildBindings") {
    description = "Builds the host cdylib that the UniFFI binding is generated from."
    workingDir = repoRoot
    // The Allodia sign-in, when this build was given the registration that turns it on. Derived
    // from that registration rather than a switch of its own, so the two halves cannot disagree:
    // the code it turns on is source-available rather than GPL and the open tree must build
    // without it, so it is an optional off-by-default dependency (BUILDING.md).
    commandLine(
        buildList {
            addAll(listOf(cargoExecutable, "build", "--quiet", "-p", "mailcal-bindings"))
            if (credentialValue("MAILCAL_ALLODIA_CLIENT_ID") != null) {
                addAll(listOf("--features", "allodia-license"))
            }
        },
    )
    inputs.files(
        fileTree(repoRoot.resolve("crates")) { include("**/*.rs") },
        repoRoot.resolve("Cargo.lock"),
    )
    outputs.file(hostCdylib)
}

val generateUniffiBindings = tasks.register<Exec>("generateUniffiBindings") {
    description = "Generates the Kotlin UniFFI binding from the host cdylib."
    dependsOn(cargoBuildBindings)
    workingDir = repoRoot
    commandLine(
        cargoExecutable, "run", "--quiet", "--bin", "uniffi-bindgen", "--",
        "generate", "--library", hostCdylib.absolutePath,
        "--language", "kotlin", "--out-dir", "clients/android/app/src/main/java",
    )
    inputs.file(hostCdylib)
    // Only the generated package is an output, the rest of src/main/java is hand-written.
    outputs.dir(layout.projectDirectory.dir("src/main/java/uniffi"))
}

val generateL10n = tasks.register<Exec>("generateL10n") {
    description = "Generates the typed L10n accessor + string resources from the inlang catalog."
    workingDir = repoRoot
    commandLine(
        cargoExecutable, "run", "--quiet", "-p", "mailcal-l10n", "--",
        "generate", "--target", "kotlin", "--root", repoRoot.absolutePath,
        "--out", "clients/android/app/src/main",
    )
    inputs.files(
        fileTree(repoRoot.resolve("messages")) { include("**/*.json") },
        fileTree(repoRoot.resolve("crates/mailcal-l10n/src")) { include("**/*.rs") },
        // The app's name is substituted into the catalog at codegen time (docs/branding.md), so a
        // brand change *is* a catalog change. Without these the generated strings keep the name
        // the last build used while the launcher label, recomputed every time, below, moves,
        // and the app disagrees with itself about what it is called.
        fileTree(repoRoot.resolve("branding")) { include("*.env") },
    )
    inputs.property("appName", brandValue("MAILCAL_APP_NAME"))
    outputs.files(
        layout.projectDirectory.file("src/main/java/eu/allodia/mailcal/L10n.kt"),
        layout.projectDirectory.file("src/main/res/values/strings.xml"),
        layout.projectDirectory.file("src/main/res/values-nl/strings.xml"),
    )
}

// `preBuild` is the ancestor of every variant task, assemble, bundle, and the unit tests alike:
// so hooking here covers each way this module can be built.
tasks.named("preBuild") {
    dependsOn(generateUniffiBindings, generateL10n)
}

// The unit tests run against the debug variant only. Compose's test rule hosts its content in the
// empty ComponentActivity that `ui-test-manifest` contributes, and that artifact is a
// `debugImplementation`, adding it to release would merge a test activity into the shipped
// manifest. Running the same sources twice buys nothing, so the release unit-test variant is
// switched off and `./gradlew test` means `testDebugUnitTest`.
androidComponents {
    beforeVariants(selector().withBuildType("release")) { variant ->
        variant.hostTests[HostTestBuilder.UNIT_TEST_TYPE]?.enable = false
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2026.08.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.material3:material3")

    // NOTE: there is deliberately no `material-icons-core` dependency.
    //
    // material3 1.4.0 dropped it and the BOM stopped managing its version; the library is frozen
    // at 1.7.8 and no longer published. Rather than pin a dead artifact, every icon is a Material
    // Symbols vector drawable vendored into res/drawable (see any `ic_*.xml` for provenance) and
    // drawn with `painterResource`, which is what Google recommends in its place. The old subset
    // also dictated some glyphs: the Archive folder used to draw a *calendar* because the subset
    // had no archive icon. Adding an icon now means adding one XML, not a dependency.
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.core:core-ktx:1.19.0")

    // AppCompat backports the per-app language override (AppCompatDelegate.setApplicationLocales)
    // below API 33; MainActivity extends AppCompatActivity so the picker can apply locales.
    implementation("androidx.appcompat:appcompat:1.8.0")

    // The account config (endpoints + credentials) is held in the OS secure store
    // (EncryptedSharedPreferences over an AES256-GCM master key), not a plaintext file.
    implementation("androidx.security:security-crypto:1.1.0")

    // UniFFI's generated Kotlin bindings call into the cdylib through JNA. The @aar
    // variant bundles JNA's own native libraries for Android (libjnidispatch.so per ABI).
    // 5.19.1 is a FLOOR: it is the first release whose `.so`s are all 16 KB page aligned on
    // every ABI (5.14's x86_64 build aligned to 4 KB, which Google Play rejects outright).
    // See the `abiFilters` note above; scripts/dev/check-android-native-libs.sh enforces it.
    implementation("net.java.dev.jna:jna:5.19.1@aar")

    // Chrome Custom Tabs for the Microsoft OAuth sign-in: opens the authorization URL in the
    // user's browser (reusing its logged-in Microsoft session) rather than an in-app WebView,
    // and returns via a custom-scheme redirect the manifest intent-filter catches.
    implementation("androidx.browser:browser:1.10.0")

    // ---- Tests -------------------------------------------------------------------------------
    // The client's tests run on the JVM (`./gradlew :app:test`), never on a device: Robolectric
    // supplies the Android framework and the merged resources, and Compose's test rule drives the
    // composables. Nothing here loads the cdylib, the generated binding's data classes and enums
    // are plain Kotlin, and no test touches `MailcalApp` (that surface is covered by the core's own
    // Rust tests). Keeping the suite emulator-free is what lets it gate every PR in CI.
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.robolectric:robolectric:4.16.1")
    testImplementation("androidx.compose.ui:ui-test-junit4")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
    // Supplies the empty ComponentActivity that `createComposeRule()` hosts its content in.
    debugImplementation("androidx.compose.ui:ui-test-manifest")

    // WorkManager drives the periodic background mail sync (docs/background-sync.md): a ~15-min
    // CoroutineWorker that runs the core's one-shot `run_background_sync` while the app is
    // backgrounded/killed, then raises new-mail notifications. Battery/Play-policy friendly and
    // it self-reschedules across reboots; pulls kotlinx-coroutines transitively.
    implementation("androidx.work:work-runtime-ktx:2.11.2")
}
