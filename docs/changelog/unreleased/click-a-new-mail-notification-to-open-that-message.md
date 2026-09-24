# Click a new-mail notification to open that message

Platforms: windows, linux
Bump: minor

> The last two clients that only brought the app forward. Both now carry the
> `(account, message key)` pair the core keys mail on inside the notification itself, which they
> have to: a click can also start the app, and then nothing the process remembers is left to look
> the message up in. Windows puts it in the toast's arguments, and reads it back from a live
> `NotificationInvoked` or, for a click that launched the app, from the activation's own arguments;
> Linux puts it in the portal's default-action target, under one `ActionInvoked` subscription taken
> at startup rather than one per pass. Both resolve it the way macOS does: the list on screen first,
> so a message in view opens where it is; otherwise the mailbox moves to that message's account for
> exactly one snapshot and the click is then dropped, rather than dragging the view back on every
> later snapshot. A summary names no message and opens the app alone. A message inside a
> conversation opens that message and discloses its thread, never the thread's representative,
> which on a busy conversation is not the one that was announced.

**English**

```
Clicking a new-mail notification now opens that message instead of just bringing the app forward.
```

**Nederlands**

```
Klik je op een melding over nieuwe e-mail, dan opent nu dat bericht in plaats van alleen de app naar voren te halen.
```

**Deutsch**

```
Ein Klick auf eine Mitteilung über neue E-Mail öffnet jetzt die betreffende Nachricht, statt nur die App in den Vordergrund zu holen.
```

**Français**

```
Cliquer sur une notification de nouveau message ouvre désormais ce message au lieu de seulement ramener l'application au premier plan.
```

**Español**

```
Al hacer clic en una notificación de correo nuevo ahora se abre ese mensaje en lugar de solo traer la aplicación al primer plano.
```

**Italiano**

```
Facendo clic su una notifica di nuova posta ora si apre quel messaggio anziché limitarsi a portare l'app in primo piano.
```

**Português**

```
Clicar numa notificação de correio novo passa a abrir essa mensagem em vez de apenas trazer a aplicação para a frente.
```
