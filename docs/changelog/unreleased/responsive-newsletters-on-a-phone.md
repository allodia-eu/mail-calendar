# Responsive newsletters on a phone

Platforms: all
Bump: patch

> The sanitiser kept `class` but dropped `id`, and a newsletter's mobile layout routinely hangs off
> `id` alone: the container table carries a fixed `width="600"` and only
> `table[id=templateContainer]{width:100% !important}`, inside a `max-width:480px` `@media` block,
> narrows it. With the attribute gone that rule matched nothing, so the message laid out 600px wide
> in a phone's ~384px viewport and every line of text ran off the right edge while images were cut
> in half. `id` is now kept, unprefixed, so the message's own responsive CSS resolves. The desktop
> reading pane is wide enough that those rules never applied there, which is why this only showed
> on a phone.

**English**

```
Newsletters that adapt to a small screen now do so in the reading view, instead of being cut off at the right edge on a phone.
```

**Nederlands**

```
Nieuwsbrieven die zich aanpassen aan een klein scherm doen dat nu ook in de leesweergave, in plaats van op een telefoon rechts weg te vallen.
```

**Deutsch**

```
Newsletter, die sich an kleine Bildschirme anpassen, tun dies jetzt auch in der Leseansicht, statt auf dem Telefon am rechten Rand abgeschnitten zu werden.
```

**Français**

```
Les lettres d'information qui s'adaptent aux petits écrans le font désormais dans la vue de lecture, au lieu d'être coupées à droite sur un téléphone.
```

**Español**

```
Los boletines que se adaptan a una pantalla pequeña ahora lo hacen en la vista de lectura, en lugar de quedar cortados por la derecha en el teléfono.
```

**Italiano**

```
Le newsletter che si adattano agli schermi piccoli ora lo fanno anche nella vista di lettura, invece di essere tagliate a destra sul telefono.
```

**Português**

```
As newsletters que se adaptam a ecrãs pequenos passam a fazê-lo na vista de leitura, em vez de ficarem cortadas à direita no telemóvel.
```
