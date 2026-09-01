# The date in a quoted reply

Platforms: macos, ios, android, linux
Bump: patch

> The attribution line and the quoted `Sent:` header were handed the core's raw UTC instant, so a
> reply quoted the original as `2026-08-31T05:01:00Z`. Windows already formatted it; the other
> three clients had the formatter and never called it. `docs/timestamps.md` scoped itself to the
> mail list and the reading header, so no rule said they were wrong. It now covers the quoted
> original too, and says why this surface matters most: it is the one timestamp a stranger reads.

**English**

```
The date on a quoted original now reads as a normal local date, in replies and forwards alike.
```

**Nederlands**

```
De datum boven een geciteerd bericht is nu een gewone lokale datum, zowel bij beantwoorden als
bij doorsturen.
```

**Deutsch**

```
Das Datum über einem zitierten Original erscheint jetzt als normales lokales Datum, beim
Antworten wie beim Weiterleiten.
```

**Français**

```
La date au-dessus d'un message cité s'affiche désormais comme une date locale normale, en réponse
comme en transfert.
```

**Español**

```
La fecha sobre un mensaje citado ahora aparece como una fecha local normal, tanto al responder
como al reenviar.
```

**Italiano**

```
La data sopra un messaggio citato ora appare come una normale data locale, sia nelle risposte sia
negli inoltri.
```

**Português**

```
A data acima de uma mensagem citada passa a surgir como uma data local normal, tanto ao responder
como ao reencaminhar.
```
