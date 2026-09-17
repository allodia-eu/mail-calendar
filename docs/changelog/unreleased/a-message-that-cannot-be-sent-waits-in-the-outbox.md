# A message that cannot be sent waits in the Outbox

Platforms: all
Bump: minor

> A send whose failure was worth retrying used to be recorded as failed and forgotten: the hint
> cleared after two and a half seconds, nothing retried the message, and no copy of it survived
> anywhere, because nothing could read the durable outbox back. The engine now keeps a queue that
> can be listed, retried and withdrawn, and the app shows it as one Outbox row above the accounts,
> present only while something is in it. The queue drains when the device comes back online, at
> the end of every sync pass, and when the user asks; reconnecting also clears each message's
> backoff, since the outage it was waiting out has just ended. Apple and Windows ship the row, the
> list and the three actions; Android and Linux queue and drain but do not draw it yet
> (`docs/sending.md` → Known gaps).

**English**

```
A message that cannot be sent right away now waits in the Outbox and goes out by itself when the connection returns, instead of being lost; you can send it now, edit it, or cancel it.
```

**Nederlands**

```
Een bericht dat niet meteen verstuurd kan worden, wacht nu in Postvak UIT en gaat vanzelf weg zodra de verbinding terug is, in plaats van verloren te gaan; je kunt het nu versturen, bewerken of annuleren.
```

**Deutsch**

```
Eine Nachricht, die nicht sofort gesendet werden kann, wartet jetzt im Postausgang und geht von selbst raus, sobald die Verbindung wieder da ist, statt verloren zu gehen; Sie können sie jetzt senden, bearbeiten oder abbrechen.
```

**Français**

```
Un message qui ne peut pas partir tout de suite attend désormais dans la boîte d'envoi et s'envoie tout seul dès que la connexion revient, au lieu d'être perdu ; vous pouvez l'envoyer maintenant, le modifier ou l'annuler.
```

**Español**

```
Un mensaje que no se puede enviar de inmediato ahora espera en la bandeja de salida y sale solo cuando vuelve la conexión, en vez de perderse; puedes enviarlo ahora, editarlo o cancelarlo.
```

**Italiano**

```
Un messaggio che non può essere inviato subito ora attende nella posta in uscita e parte da solo quando la connessione torna, invece di andare perso; puoi inviarlo subito, modificarlo o annullarlo.
```

**Português**

```
Uma mensagem que não pode ser enviada de imediato fica agora na caixa de saída e segue sozinha quando a ligação voltar, em vez de se perder; pode enviá-la agora, editá-la ou cancelá-la.
```
