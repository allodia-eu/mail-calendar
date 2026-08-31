// The bottom-navigation host shared by the mail/calendar/contacts tabs, split out of
// CalendarScreen.kt: it is not calendar-specific, just drawn beside it originally.
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Box
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource

// Which top-level screen the bottom bar is showing. An enum rather than the pair of booleans
// this started as: with three destinations, booleans admit states that cannot exist ("calendar
// and contacts at once") and every reader has to prove they don't happen.
internal enum class AppDestination { MAIL, CALENDAR, CONTACTS }

// The bottom-navigation host. The selected destination and the switch handler live in
// MainActivity (state stays put); selecting Calendar or Contacts also kicks a sync.
// [content] renders the active screen inside the scaffold's content padding.
//
// [home] is the destination the app opened on, and it is what makes system back mean one thing
// everywhere: back walks down through whatever is open, then across to [home], and only then does
// the app close. Written as ONE rule here rather than per tab, every destination, present and
// future, inherits it, and a tab that forgot to opt in would silently close the app instead of
// returning to the mailbox (which is exactly how Calendar and Contacts behaved).
@Composable
internal fun AppNavScaffold(
    destination: AppDestination,
    home: AppDestination,
    onSelect: (AppDestination) -> Unit,
    content: @Composable () -> Unit,
) {
    val ctx = LocalContext.current
    // Deliberately DISABLED on the home destination rather than calling finish(): an unhandled
    // press is what lets the platform run its own predictive-back close animation, and it is also
    // what makes "the last back closes the app" true without this screen having to know how.
    // Declared before `content`, so any handler a screen registers inside it is added later and
    // therefore wins, the innermost open thing unwinds first.
    BackHandler(enabled = destination != home) { onSelect(home) }
    Scaffold(
        bottomBar = {
            NavigationBar {
                NavigationBarItem(
                    selected = destination == AppDestination.MAIL,
                    onClick = { onSelect(AppDestination.MAIL) },
                    icon = { Icon(painterResource(R.drawable.ic_mail), contentDescription = null) },
                    label = { Text(L10n.nav_mail(ctx)) },
                )
                NavigationBarItem(
                    selected = destination == AppDestination.CALENDAR,
                    onClick = { onSelect(AppDestination.CALENDAR) },
                    icon = { Icon(painterResource(R.drawable.ic_calendar_month), contentDescription = null) },
                    label = { Text(L10n.nav_calendar(ctx)) },
                )
                NavigationBarItem(
                    selected = destination == AppDestination.CONTACTS,
                    onClick = { onSelect(AppDestination.CONTACTS) },
                    icon = { Icon(painterResource(R.drawable.ic_contacts), contentDescription = null) },
                    label = { Text(L10n.nav_contacts(ctx)) },
                )
            }
        },
    ) { innerPadding ->
        Box(modifier = Modifier.padding(innerPadding)) { content() }
    }
}
