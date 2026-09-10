# A message always fits, and you can zoom it

Platforms: all
Bump: minor

> Two halves of one contract, [`docs/reading-zoom.md`](../../reading-zoom.md). The shared reading
> document declared `initial-scale=1`, which pins the page at 1:1, and that is exactly the condition
> under which both touch engines decline to shrink an over-wide message: mail with no `@media` rules
> at all, a table pinned to 600px in the markup and again inline, ran off the right edge of a phone
> with nothing the reader could do about it. The declaration now names a width and no scale, Android
> honours it at all (`useWideViewPort` is false by default, which ignores the tag outright) and
> shrinks a too-wide message to fit (`loadWithOverviewMode`). The desktop hosts have no such setting
> and the only way to compute the scale would be to measure the document from inside it, which needs
> script in the message: they scroll sideways and the reader zooms out instead, which is what
> Thunderbird does on a desktop too. Zoom itself is now on every client, 0.25× to 5×, and resets to
> each message's own fit, so the three hosts whose zoom is the *view's* rather than the *page's*
> (macOS, Windows, Linux) reset it explicitly on open.

**English**

```
Messages too wide to reflow are now scaled to fit the reading pane instead of running off the edge, and you can pinch to zoom any message.
```

**Nederlands**

```
Berichten die te breed zijn om mee te schalen passen nu in het leesvenster in plaats van weg te vallen aan de rand, en u kunt elk bericht in- en uitzoomen met een knijpbeweging.
```

**Deutsch**

```
Nachrichten, die zu breit sind, um umzubrechen, werden jetzt auf die Lesebreite verkleinert, statt am Rand abgeschnitten zu werden, und jede Nachricht lässt sich per Fingergeste zoomen.
```

**Français**

```
Les messages trop larges pour se réajuster sont désormais réduits à la taille du volet de lecture au lieu de déborder, et vous pouvez zoomer sur n'importe quel message par pincement.
```

**Español**

```
Los mensajes demasiado anchos para readaptarse ahora se reducen al tamaño del panel de lectura en lugar de salirse del borde, y puede acercar o alejar cualquier mensaje pellizcando.
```

**Italiano**

```
I messaggi troppo larghi per riadattarsi vengono ora ridotti alla larghezza del riquadro di lettura invece di uscire dal bordo, e ogni messaggio si può ingrandire con un pizzico.
```

**Português**

```
As mensagens demasiado largas para se reajustarem passam a ser reduzidas à largura do painel de leitura em vez de saírem pela margem, e pode ampliar qualquer mensagem com dois dedos.
```
