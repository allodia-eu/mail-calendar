# Scroll the calendar straight past the weekend

Platforms: macos, ios
Bump: minor

> The Mac and the phone stop paging the calendar by the week and draw one continuous strip of days
> instead, the model Windows already ships and Apple's own Calendar uses on both. The weeks are laid
> end to end with the hour ruler pinned beside them, so a grid resting on Sunday-and-Monday is a
> coherent frame rather than half of each of two pages, and the strip comes to rest on a **day** at
> every zoom and for every input. That needs no threshold and no judgement about whether a drag
> travelled far enough, which is the machinery that rubber-bands a slow pan back home.
> `CalendarStrip` is a plain Swift value type, so where the grid rests, which weeks it is showing and
> what a pan does to it are pinned without a viewport. Deliberately left out: the iPad's trackpad is
> wired but unverified, and the frame budget has not been measured on a release build. Both are under
> `docs/calendar.md` → "Known gaps".

**English**

```
Scroll the calendar sideways with a trackpad, a mouse wheel or a finger, and it now runs straight
across the week boundary instead of stopping at Sunday, coming to rest on a whole day.
```

**Nederlands**

```
Scroll de agenda zijwaarts met een trackpad, muiswiel of vinger: hij loopt nu gewoon door over de
weekgrens heen in plaats van bij zondag te stoppen, en komt op een hele dag tot stilstand.
```

**Deutsch**

```
Scrollen Sie den Kalender mit Trackpad, Mausrad oder Finger zur Seite: Er läuft jetzt über die
Wochengrenze hinweg, statt am Sonntag zu stoppen, und kommt auf einem ganzen Tag zur Ruhe.
```

**Français**

```
Faites défiler le calendrier latéralement avec un pavé tactile, une molette ou un doigt : il passe
désormais la limite de la semaine au lieu de s’arrêter au dimanche, et s’arrête sur un jour entier.
```

**Español**

```
Desplaza el calendario en horizontal con el trackpad, la rueda del ratón o el dedo: ahora cruza el
límite de la semana en vez de pararse en domingo, y se detiene en un día completo.
```

**Italiano**

```
Scorri il calendario in orizzontale con trackpad, rotellina o dito: ora prosegue oltre il confine
della settimana invece di fermarsi alla domenica, e si ferma su un giorno intero.
```

**Português**

```
Percorra o calendário na horizontal com o trackpad, a roda do rato ou o dedo: agora atravessa o
limite da semana em vez de parar no domingo, e para num dia inteiro.
```
