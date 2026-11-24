# Linux draws the writing area of every new message

Platforms: linux
Bump: patch

> WebKitGTK's hardware-accelerated renderer never paints a view whose page is laid out while the
> view has no height, and resizing it afterwards does not recover it (WebKit bug 332660). The
> composer's editor had no height until the column first measured its visible part, and from the
> second composer on the page loads before that, because the content filter it waits for is
> already compiled; a first composer did so about one time in four. The editor is now given its
> minimum height when the column takes it.

**English**

```
A new message no longer opens with an empty space where you write.
```

**Nederlands**

```
Een nieuw bericht opent niet meer met een lege ruimte waar je schrijft.
```

**Deutsch**

```
Eine neue Nachricht öffnet sich nicht mehr mit einer leeren Fläche anstelle des Schreibbereichs.
```

**Français**

```
Un nouveau message ne s’ouvre plus avec un espace vide à la place de la zone de rédaction.
```

**Español**

```
Un mensaje nuevo ya no se abre con un espacio vacío en lugar del área de escritura.
```

**Italiano**

```
Un nuovo messaggio non si apre più con uno spazio vuoto al posto dell’area di scrittura.
```

**Português**

```
Uma nova mensagem já não abre com um espaço vazio no lugar da área de escrita.
```
