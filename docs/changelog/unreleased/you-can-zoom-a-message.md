# You can zoom a message

Platforms: all
Bump: minor

> The zoom half of `docs/reading-zoom.md`, whose sizing half is now
> `newsletters-reflow-to-the-reading-pane.md`. The shared reading document declared
> `initial-scale=1`, which pins the page at 1:1 and is the condition under which Blink declines to
> shrink an over-wide message; it now names a width and no scale, and Android honours the viewport
> at all (`useWideViewPort` is false by default, which ignores the tag outright). Zoom itself is now
> on every client, from each message's own scale up to 5×, starting again at that scale on every
> open, so every host whose zoom is the *view's* rather than the *page's* resets it. On iPhone and
> iPad the range starts at 1 rather than 0.25, which is WebKit's and not ours: it will not scale a
> page below the width at which its layout viewport fits.

**English**

```
You can now pinch to zoom a message, and every message opens at its own scale again.
```

**Nederlands**

```
U kunt nu met een knijpbeweging in- en uitzoomen op een bericht, en elk bericht opent weer op zijn eigen schaal.
```

**Deutsch**

```
Sie können eine Nachricht jetzt per Fingergeste zoomen, und jede Nachricht öffnet wieder in ihrem eigenen Maßstab.
```

**Français**

```
Vous pouvez désormais zoomer sur un message par pincement, et chaque message s'ouvre de nouveau à sa propre échelle.
```

**Español**

```
Ahora puede acercar o alejar un mensaje pellizcando, y cada mensaje vuelve a abrirse a su propia escala.
```

**Italiano**

```
Ora si può ingrandire un messaggio con un pizzico, e ogni messaggio torna ad aprirsi alla propria scala.
```

**Português**

```
Agora pode ampliar uma mensagem com dois dedos, e cada mensagem volta a abrir à sua própria escala.
```
