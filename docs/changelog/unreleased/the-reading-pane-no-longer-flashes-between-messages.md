# The reading pane no longer flashes between messages

Platforms: macos, ios, windows, android, linux
Bump: patch

> Two causes, both showing as black-to-white against a dark theme. The body area is the page a
> message is drawn on, and every client left it transparent until a body arrived, which punched a
> hole in that page for the length of the open (75–82 ms, measured). `MESSAGE_CANVAS` now sits
> beside the base stylesheet and is interpolated into it, crosses the FFI as `message_canvas`, and
> every client paints it behind the whole body area in a light appearance. A web view needs one
> thing more: WebKitGTK presents a black frame until the document's first paint (467 ms on a heavy
> message at full screen), so the Linux pane holds the canvas until the load reports finished.

**English**

```
Moving between messages no longer flashes a dark panel where the message should be; the page stays put and only the message on it changes.
```

**Nederlands**

```
Als je van bericht wisselt zie je niet langer kort een donker vlak waar het bericht hoort te staan; de pagina blijft staan en alleen het bericht erop verandert.
```

**Deutsch**

```
Beim Wechsel zwischen Nachrichten blitzt keine dunkle Fläche mehr dort auf, wo die Nachricht stehen sollte; die Seite bleibt stehen und nur die Nachricht darauf wechselt.
```

**Français**

```
Passer d'un message à l'autre ne fait plus apparaître brièvement un fond sombre à la place du message ; la page reste en place et seul le message qui s'y trouve change.
```

**Español**

```
Al pasar de un mensaje a otro ya no aparece un instante un panel oscuro donde debería estar el mensaje; la página se mantiene y solo cambia el mensaje que hay en ella.
```

**Italiano**

```
Passando da un messaggio all'altro non compare più per un istante un riquadro scuro al posto del messaggio; la pagina resta ferma e cambia solo il messaggio su di essa.
```

**Português**

```
Ao mudar de mensagem deixa de surgir por instantes uma área escura no lugar da mensagem; a página mantém-se e muda apenas a mensagem que está nela.
```
