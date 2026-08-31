// The first-boot welcome screen, and the one place we ask about usage statistics.
//
// The rules it implements are legal conditions, not styling choices (docs/analytics.md):
//
//   * The switch starts **off**, and nothing is written to the device until the user leaves the
//     screen. Under ePrivacy Art. 5(3) the *act* of storing the install id is what needs consent,
//     so a pre-ticked switch would not merely be rude, it would be the violation itself.
//   * Refusing costs nothing. "Get started" is the only way forward and it is always enabled, so
//     the switch is genuinely optional rather than a toll gate.
//   * The consent is unbundled: this screen accepts no terms and creates no account. It asks one
//     question and takes one answer (GDPR Art. 7(2)).
//   * "See exactly what we send" renders the literal payload the core would put on the wire.
//
// Deliberately a pure composable, every value is a parameter, every write a lambda, so the JVM
// suite can drive it without loading the cdylib (see the Android notes in AGENTS.md).
package eu.allodia.mailcal

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp

/**
 * The welcome + consent screen.
 *
 * @param payloadPreview the literal JSON the core would send, pulled lazily.
 * @param onGetStarted the user's decision, taken exactly once when they leave the screen: `true`
 *   only if they deliberately moved the switch on. Recording the `false` case matters as much as
 *   the `true` one, it is what stops us asking again.
 */
@Composable
fun WelcomeScreen(
    payloadPreview: () -> String,
    onGetStarted: (Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    val uriHandler = LocalUriHandler.current
    // Default OFF. The whole feature hangs off this one line.
    var shareStats by remember { mutableStateOf(false) }

    // This screen renders outside the Scaffold (which would pad its content), so it must clear the
    // system bars itself, the app is edge-to-edge and has no say in it. Without this the button
    // below draws UNDER the navigation bar, which on a three-button device hides half of the only
    // way forward out of the first screen a new user ever sees.
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding().padding(24.dp)) {
        // The Box owns the free space; the scrolling column sizes to its content and is centred
        // inside it. Centring the scrolling column directly would do nothing, a `verticalScroll`
        // column measures against its *content*, so it has no free space to arrange within. This
        // way short content sits in the middle, and content that outgrows the screen (the payload
        // panel, a large font size) fills the Box and scrolls instead.
        Box(modifier = Modifier.weight(1f), contentAlignment = Alignment.Center) {
            Column(
                modifier = Modifier.verticalScroll(rememberScrollState()),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Image(
                    painter = painterResource(R.drawable.welcome_art),
                    contentDescription = L10n.a11y_welcome_art(ctx),
                    modifier = Modifier.size(140.dp),
                )
                Text(
                    L10n.welcome_title(ctx),
                    style = MaterialTheme.typography.headlineSmall,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 12.dp),
                )
                Text(
                    L10n.welcome_tagline(ctx),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.padding(top = 4.dp, bottom = 24.dp),
                )

                Card(modifier = Modifier.fillMaxWidth()) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        AnalyticsSwitchRow(
                            label = L10n.welcome_analytics_toggle(ctx),
                            checked = shareStats,
                            onCheckedChange = { shareStats = it },
                        )
                        Text(
                            L10n.welcome_analytics_body(ctx),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                        AnalyticsPayloadPanel(payloadPreview)
                    }
                }

                TextButton(
                    onClick = { uriHandler.openUri(L10n.welcome_privacy_url(ctx)) },
                    modifier = Modifier.padding(top = 8.dp),
                ) {
                    Text(L10n.welcome_privacy_policy(ctx))
                }
            }
        }

        // Outside the scrolling area, so it stays on screen however far the payload panel pushes
        // the content down, and enabled whichever way the switch is set.
        Button(
            onClick = { onGetStarted(shareStats) },
            modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
        ) {
            Text(L10n.welcome_get_started(ctx))
        }
    }
}
