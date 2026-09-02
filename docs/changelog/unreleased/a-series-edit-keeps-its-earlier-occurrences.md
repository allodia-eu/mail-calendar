# A series edit keeps the occurrences before the one you opened

Platforms: all
Bump: patch

> Editing a repeating event from one of its later occurrences and answering *All events* wrote that
> occurrence's clocks onto the series, so its start moved forward and every occurrence before it
> stopped existing. It needed no time change to happen: a rename was enough, and the repeat editor
> made it easy to reach because a rule change means the series without asking. An edit meant for
> the series now carries the *shift* the user made, applied to the series' own clock, which is what
> a drag on a series already does; `EventEdit::times_from_occurrence` names where the clocks were
> read from, and `calendar_series_shift_tests.rs` pins the four cases.

**English**

```
Editing a repeating event from a later occurrence no longer deletes the ones before it, and
changing the time there now moves the whole series by that much.
```

**Nederlands**

```
Een terugkerende afspraak bewerken vanaf een latere herhaling verwijdert de eerdere niet meer, en
de tijd daar aanpassen verschuift voortaan de hele reeks met datzelfde verschil.
```

**Deutsch**

```
Wer einen Serientermin von einem späteren Termin aus bearbeitet, verliert die früheren nicht mehr,
und eine dort geänderte Uhrzeit verschiebt jetzt die ganze Serie um denselben Betrag.
```

**Français**

```
Modifier un événement récurrent depuis une occurrence ultérieure ne supprime plus les précédentes,
et y changer l’heure décale désormais toute la série d’autant.
```

**Español**

```
Editar un evento periódico desde una repetición posterior ya no elimina las anteriores, y cambiar
la hora allí ahora desplaza toda la serie en la misma medida.
```

**Italiano**

```
Modificare un evento ricorrente da un'occorrenza successiva non elimina più quelle precedenti, e
cambiare l'ora lì ora sposta l'intera serie della stessa quantità.
```

**Português**

```
Editar um evento recorrente a partir de uma ocorrência posterior deixa de apagar as anteriores, e
alterar a hora aí passa a deslocar toda a série na mesma medida.
```
