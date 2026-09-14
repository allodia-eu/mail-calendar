# Calendar opens on the current hour, scrolls, and survives a resize

Platforms: linux
Bump: patch

> The grid's height follows its viewport, and GTK reports a new viewport from inside the viewport's
> own size allocation, where neither a height nor a scroll offset asked for there reaches the
> screen. Both are now asked for from an idle, after the frame. That leaves one pass where the
> scrolled window measures the old day against the new size, so the offset is held as the minute at
> the middle of the viewport and put back once the geometry lands, rather than followed through a
> clamp that is not the reader's. Widget tests put a grid on screen, assert the scroll range exists
> and the grid is drawn where its offset says, then resize the window and assert the hour came back.

**English**

```
The calendar now opens at the current hour and scrolls from the moment it appears, instead of standing at midnight and ignoring the wheel until you moved to another week, and resizing the window keeps the hours you were reading rather than dropping you back at the start of the day.
```

**Nederlands**

```
De agenda opent nu op het huidige uur en laat zich meteen scrollen, in plaats van op middernacht te blijven staan en het scrollwiel te negeren tot u naar een andere week ging, en bij het verslepen van de vensterrand blijven de uren staan die u aan het lezen was.
```

**Deutsch**

```
Der Kalender öffnet jetzt zur aktuellen Stunde und lässt sich sofort scrollen, statt auf Mitternacht stehen zu bleiben und das Mausrad zu ignorieren, bis Sie in eine andere Woche gewechselt sind, und beim Ändern der Fenstergröße bleiben die Stunden stehen, die Sie gerade gelesen haben.
```

**Français**

```
L'agenda s'ouvre désormais à l'heure actuelle et défile dès son apparition, au lieu de rester à minuit et d'ignorer la molette jusqu'à ce que vous changiez de semaine, et le redimensionnement de la fenêtre conserve les heures que vous lisiez au lieu de vous ramener au début de la journée.
```

**Español**

```
El calendario ahora se abre a la hora actual y se desplaza desde que aparece, en lugar de quedarse a medianoche e ignorar la rueda hasta que cambiabas de semana, y al cambiar el tamaño de la ventana se mantienen las horas que estabas leyendo.
```

**Italiano**

```
Il calendario ora si apre all'ora corrente e scorre fin da quando compare, invece di restare a mezzanotte e ignorare la rotellina finché non cambiavi settimana, e ridimensionare la finestra mantiene le ore che stavi leggendo.
```

**Português**

```
O calendário abre agora na hora atual e desliza assim que aparece, em vez de ficar à meia-noite e ignorar a roda do rato até mudar de semana, e redimensionar a janela mantém as horas que estava a ler.
```
