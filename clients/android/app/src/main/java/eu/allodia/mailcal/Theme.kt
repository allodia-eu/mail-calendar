// The app's Material 3 colour scheme. The default `MaterialTheme` ships the Material 3 baseline
// palette, whose neutral surfaces carry a lavender tint (surface ≈ #FEF7FF), that's the "pink"
// background. Here we flatten the neutral roles to plain white (light) / near-black (dark) while
// keeping the purple primary as the accent. Which of the two applies is the core's persisted
// Appearance setting, which defaults to following the OS. Kept in its own file so MainActivity
// stays under the 500-line limit.
package eu.allodia.mailcal

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat
import uniffi.mailcal_bindings.Appearance

// Light: white window/surfaces with a subtle grey ladder for the container roles (search field,
// segmented button, FAB tint, chips). Only the neutral roles are overridden, primary/secondary/
// tertiary and their containers keep the Material 3 baseline purple accents.
private val LightColors = lightColorScheme(
    background = Color(0xFFFFFFFF),
    surface = Color(0xFFFFFFFF),
    surfaceVariant = Color(0xFFECECEE),
    surfaceContainerLowest = Color(0xFFFFFFFF),
    surfaceContainerLow = Color(0xFFF7F7F8),
    surfaceContainer = Color(0xFFF2F2F4),
    surfaceContainerHigh = Color(0xFFECECEE),
    surfaceContainerHighest = Color(0xFFE6E6E9),
    surfaceBright = Color(0xFFFFFFFF),
    surfaceDim = Color(0xFFDBDBDE),
)

// Dark: near-black window/surfaces with a lighter grey ladder. darkColorScheme's baseline keeps
// the lavender-black surfaces, so we flatten them the same way; the light-purple primary reads
// well on the neutral dark background.
private val DarkColors = darkColorScheme(
    background = Color(0xFF121212),
    surface = Color(0xFF121212),
    surfaceVariant = Color(0xFF2C2C2E),
    surfaceContainerLowest = Color(0xFF0D0D0D),
    surfaceContainerLow = Color(0xFF1A1A1B),
    surfaceContainer = Color(0xFF1E1E1F),
    surfaceContainerHigh = Color(0xFF282829),
    surfaceContainerHighest = Color(0xFF333335),
    surfaceBright = Color(0xFF39393B),
    surfaceDim = Color(0xFF121212),
)

/**
 * Whether the app is currently painted dark.
 *
 * Every surface that picks its own colours from a light/dark pair, the calendar swatches, the
 * month chips, the invitation preview, the composer's system bars, reads this rather than
 * `isSystemInDarkTheme()`, which answers what the DEVICE is set to and so contradicts an app-level
 * Appearance of Light or Dark. Defaults to light for a composable rendered outside [AppTheme].
 */
internal val LocalAppDark = staticCompositionLocalOf { false }

// The app theme: picks the neutral light/dark scheme from the app's Appearance setting, falling
// back to the OS's own when it says to, and keeps the status-bar icon appearance readable against
// it (dark icons on the white light theme, light icons on dark).
@Composable
internal fun AppTheme(
    appearance: Appearance = Appearance.SYSTEM,
    content: @Composable () -> Unit,
) {
    val darkTheme = when (appearance) {
        Appearance.LIGHT -> false
        Appearance.DARK -> true
        // Read inside the composable, so a device switching scheme mid-session still reaches the app.
        Appearance.SYSTEM -> isSystemInDarkTheme()
    }
    val colorScheme = if (darkTheme) DarkColors else LightColors
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !darkTheme
        }
    }
    CompositionLocalProvider(LocalAppDark provides darkTheme) {
        MaterialTheme(colorScheme = colorScheme, content = content)
    }
}
