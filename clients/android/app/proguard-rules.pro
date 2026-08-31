# R8 rules for the release build.
#
# The whole file exists for one reason: the Rust core is reached through **JNA**, and JNA works by
# reflection. R8 cannot see any of it. Shrink without these rules and the app still builds, still
# installs, and then dies the moment it touches the core, which is a failure that only ever appears
# in the build you ship.

# ---------------------------------------------------------------------------------------------
# JNA itself.
# ---------------------------------------------------------------------------------------------
# JNA is written against desktop Java and references AWT, which does not exist on Android. Nothing
# reachable from our code touches it, so the references are dead, but R8 still warns about them.
-dontwarn java.awt.**
-dontwarn com.sun.jna.internal.Cleaner
-dontwarn com.sun.jna.Structure$FFIType*

-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }

# ---------------------------------------------------------------------------------------------
# Tink, under androidx.security-crypto, the encrypted store the account credentials live in.
# ---------------------------------------------------------------------------------------------
# These are compile-time-only annotations. They are not on the runtime classpath by design, and R8
# refuses to shrink until it is told they are meant to be absent.
-dontwarn com.google.errorprone.annotations.**
-dontwarn javax.annotation.**

# ---------------------------------------------------------------------------------------------
# The generated UniFFI bindings.
# ---------------------------------------------------------------------------------------------
# `Native.load` binds the FFI functions to a Kotlin interface **by method name**, so obfuscating
# those names silently unbinds every call into Rust.
-keep interface uniffi.mailcal_bindings.UniffiLib { *; }
-keep class uniffi.mailcal_bindings.** { *; }

# A JNA `Structure` is mapped field-by-field, by name and in declaration order (`@FieldOrder`).
# Renaming or reordering the fields, or dropping one R8 thinks is unused, corrupts the memory
# layout the Rust side reads back, which does not throw. It returns garbage.
-keep class * extends com.sun.jna.Structure { *; }
-keepclassmembers class * extends com.sun.jna.Structure {
    public <fields>;
}

# Callbacks are how Rust calls US: the observer that signals a surface changed, the logger, the
# credential store. They are invoked from a Rust thread with no Kotlin caller to keep them alive, so
# R8 sees them as unreachable and strips them.
-keep class * implements com.sun.jna.Callback { *; }
-keepclassmembers class * implements com.sun.jna.Callback {
    <methods>;
}

# ---------------------------------------------------------------------------------------------
# Room databases (WorkManager's WorkDatabase drives the background-sync worker).
# ---------------------------------------------------------------------------------------------
# Room's generated `*_Impl` is instantiated reflectively via its no-arg constructor (WorkManager
# does this at startup through androidx.startup). Under R8 *strict* full mode
# (android.r8.strictFullModeForKeepRules, the AGP-9 default) the loose consumer rules no longer
# retain that constructor, so the app dies on launch with NoSuchMethodException WorkDatabase_Impl.
# This only ever surfaces in the release build. Keep the constructor on every Room database.
-keep class * extends androidx.room.RoomDatabase { <init>(); }
