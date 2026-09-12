# Pinch the calendar on iPhone and iPad

Platforms: ios
Bump: patch

> The grid's pinch recognizer sat on the SwiftUI overlay that carries it. An overlay is a sibling of
> the content rather than its ancestor, and UIKit offers a touch only to the recognizers of the
> hit-test view and the views above it in that chain, so the handler was never called: the view was
> built, the recognizer existed, and the grid drew perfectly while ignoring every pinch. The scroll
> catcher beside it already hangs its recognizer on the window for exactly this reason; the zoom
> catcher now does the same, narrowed to the grid in `gestureRecognizerShouldBegin` so a pinch on
> the mailbox list still belongs to the mailbox list. Nothing but a real gesture can see this
> failure, which is why it survived every gate the repository had: `CalendarPinchTests` in the new
> Apple UI suite is the one that now holds it.

**English**

```
Pinching the calendar on iPhone and iPad zooms it again, both the hours and the days.
```

**Nederlands**

```
Knijpen in de agenda op iPhone en iPad zoomt weer in en uit, zowel op de uren als op de dagen.
```

**Deutsch**

```
Das Zusammenziehen im Kalender auf iPhone und iPad zoomt wieder, sowohl die Stunden als auch die Tage.
```

**Français**

```
Le pincement sur le calendrier de l'iPhone et de l'iPad zoome de nouveau, aussi bien les heures que les jours.
```

**Español**

```
Pellizcar el calendario en el iPhone y el iPad vuelve a ampliarlo, tanto las horas como los días.
```

**Italiano**

```
Il pizzico sul calendario di iPhone e iPad torna a ingrandirlo, sia le ore sia i giorni.
```

**Português**

```
Juntar os dedos no calendário do iPhone e do iPad volta a ampliá-lo, tanto as horas como os dias.
```
