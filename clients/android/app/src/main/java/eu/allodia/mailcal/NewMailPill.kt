// The floating "new mail" pill the mailbox shows when mail arrives above the scroll position.
// Its own file because it is a widget rather than part of the screen's logic, and MailboxScreen.kt
// is at the line limit.
package eu.allodia.mailcal

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp

// A rounded, elevated primary-coloured chip with an up-arrow. Kept as its own composable so
// `AnimatedVisibility` binds to the plain top-level overload rather than the caller's Box/Column
// scope, which would otherwise reject the implicit receiver.
@Composable
internal fun NewMailPill(visible: Boolean, label: String, onClick: () -> Unit) {
    AnimatedVisibility(visible = visible) {
        Surface(
            onClick = onClick,
            shape = RoundedCornerShape(50),
            color = MaterialTheme.colorScheme.primary,
            contentColor = MaterialTheme.colorScheme.onPrimary,
            shadowElevation = 4.dp,
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Icon(painterResource(R.drawable.ic_keyboard_arrow_up), contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(label)
            }
        }
    }
}
