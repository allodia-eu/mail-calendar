# Paging the calendar no longer pauses

Platforms: macos, ios, android, windows, linux
Bump: patch

> The core laid out a month page by placing every event on the day axis once for each of the 42
> cells, a time-zone conversion every time. On a calendar of 1,600 occurrences that took half a
> second a page, and Linux paid it on every week it paged, because it keeps the month current
> beside the week. Each occurrence is now placed once, and only those inside the six weeks are kept
> (`crates/mailcal-viewmodel/src/calendar/month.rs`): measured on Linux, a week step went from
> 500 to 830 ms to 30 to 50 ms in a debug build.

**English**

```
Moving to the next or previous week or month in the calendar is instant again.
```

**Nederlands**

```
Naar de volgende of vorige week of maand gaan in de agenda gaat weer direct.
```

**Deutsch**

```
Der Wechsel zur nächsten oder vorherigen Woche oder zum nächsten oder vorherigen Monat im Kalender geht wieder sofort.
```

**Français**

```
Passer à la semaine ou au mois suivant ou précédent dans le calendrier est de nouveau instantané.
```

**Español**

```
Pasar a la semana o al mes siguiente o anterior en el calendario vuelve a ser inmediato.
```

**Italiano**

```
Passare alla settimana o al mese successivo o precedente nel calendario è di nuovo immediato.
```

**Português**

```
Passar para a semana ou o mês seguinte ou anterior no calendário volta a ser imediato.
```
