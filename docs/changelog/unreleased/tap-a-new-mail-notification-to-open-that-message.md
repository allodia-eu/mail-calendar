# Tap a new-mail notification to open that message

Platforms: ios
Bump: minor

> Two shortfalls the phone carried since notifications landed there. A tap only brought the app
> forward, and the messages past the core's preview cap went unmentioned rather than merely
> unnamed, because the phone stacks an account's notifications behind one another. Both now match
> what Android already did: one notification per message carrying the `(account, message key)` pair
> the core keys it on, and a "+N more" summary for the remainder. The delegate that answers the tap
> is installed at every launch rather than only in a debug build, which is what it was before, so a
> release build could not have answered one at all. The reading pane belongs to a scene and the
> delegate is process-wide, so the target waits in a box the shell drains, the shape the Share
> Extension's drop box already has. A summary names no message and opens the app alone; a message
> the list has not loaded yet is waited for over one account switch and then dropped, rather than
> dragging the view back to the mailbox on every later snapshot.

**English**

```
Tapping a new-mail notification now opens that message instead of just opening the app, and when several arrive at once a short "+N more" tells you how many are waiting.
```

**Nederlands**

```
Tik je op een melding over nieuwe e-mail, dan opent nu dat bericht in plaats van alleen de app, en als er meerdere tegelijk binnenkomen laat "+N meer" zien hoeveel er nog wachten.
```

**Deutsch**

```
Beim Tippen auf eine Mitteilung über neue E-Mail öffnet sich jetzt die betreffende Nachricht statt nur der App, und wenn mehrere zugleich eintreffen, zeigt "+N weitere", wie viele noch warten.
```

**Français**

```
Toucher une notification de nouveau message ouvre désormais ce message et non plus seulement l'application, et lorsque plusieurs arrivent en même temps, un « +N de plus » indique combien attendent encore.
```

**Español**

```
Al tocar una notificación de correo nuevo ahora se abre ese mensaje en lugar de solo la aplicación, y cuando llegan varios a la vez un "+N más" indica cuántos quedan esperando.
```

**Italiano**

```
Toccando una notifica di nuova posta ora si apre quel messaggio anziché solo l'app, e quando ne arrivano diverse insieme un "+N altri" indica quanti sono ancora in attesa.
```

**Português**

```
Tocar numa notificação de correio novo passa a abrir essa mensagem em vez de apenas a aplicação e, quando chegam várias ao mesmo tempo, um "+N mais" indica quantas ainda esperam.
```
