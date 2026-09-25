// Sender and event text drawn natively, with the web and mail addresses in it as links: a
// plain-text body, an event's notes, an invitation's description.
//
// The core finds the addresses (`linkedText`); this only draws them. Each run is appended as text,
// so nothing here parses markup or HTML (docs/rendering-security.md, Gate 8), and a tap opens only
// what `shouldOpenExternalLink` allows, through the handoff the reading view's links use.

package eu.allodia.mailcal

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withLink
import androidx.core.net.toUri
import uniffi.mailcal_bindings.LinkedText
import uniffi.mailcal_bindings.linkedText
import uniffi.mailcal_bindings.shouldOpenExternalLink

/** The runs as one string, each linked run carrying a clickable annotation that calls [onOpen]. */
internal fun linkedAnnotatedString(
    runs: List<LinkedText>,
    linkColor: Color,
    onOpen: (String) -> Unit,
): AnnotatedString = buildAnnotatedString {
    val style = TextLinkStyles(SpanStyle(color = linkColor, textDecoration = TextDecoration.Underline))
    for (run in runs) {
        val link = run.link
        if (link == null) {
            append(run.text)
        } else {
            // `Clickable`, not `Url`: a `Url` annotation opens through the platform's own
            // handler, around the scheme gate.
            withLink(LinkAnnotation.Clickable(tag = link, styles = style) { onOpen(link) }) {
                append(run.text)
            }
        }
    }
}

/** [text] with its addresses as links, otherwise as `Text` draws it. */
@Composable
internal fun LinkifiedText(
    text: String,
    style: TextStyle,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val context = LocalContext.current
    val linkColor = MaterialTheme.colorScheme.primary
    val annotated = remember(text, linkColor) {
        linkedAnnotatedString(linkedText(text), linkColor) { openExternalLink(context, it) }
    }
    Text(
        text = annotated,
        modifier = modifier,
        style = style,
        color = color,
        maxLines = maxLines,
        overflow = overflow,
    )
}

/**
 * Hands a link the user tapped to the system's handler, when the shared launch policy allows its
 * scheme. The one path out for a link on every surface, the reading view's included.
 */
internal fun openExternalLink(context: Context, url: String) {
    if (!shouldOpenExternalLink(url)) return
    try {
        context.startActivity(
            Intent(Intent.ACTION_VIEW, url.toUri()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    } catch (_: ActivityNotFoundException) {
        // No app handles this scheme: ignore rather than crash.
    }
}
