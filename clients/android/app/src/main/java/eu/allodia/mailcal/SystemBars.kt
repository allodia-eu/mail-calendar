// Window chrome for the full-screen dialogs: the composer and the signature editor both draw
// behind the system bars, and both need their icons pointed at the current theme. Its own file
// because it belongs to neither of them.
package eu.allodia.mailcal

import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.window.DialogWindowProvider
import androidx.core.view.WindowCompat

// Point the enclosing dialog window's status/navigation-bar icons at the current theme, so they
// stay legible against the dialog drawing behind them. A no-op outside a dialog.
@Composable
internal fun SystemBarsMatchTheme() {
    val view = LocalView.current
    val lightBars = !LocalAppDark.current
    SideEffect {
        val window = (view.parent as? DialogWindowProvider)?.window ?: return@SideEffect
        WindowCompat.getInsetsController(window, view).apply {
            isAppearanceLightStatusBars = lightBars
            isAppearanceLightNavigationBars = lightBars
        }
    }
}
