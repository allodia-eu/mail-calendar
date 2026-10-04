# A queued message appears in the Outbox at once

Platforms: all
Bump: patch

> The Outbox rides the mailbox-list snapshot, and after a send that queued, that snapshot was
> republished only by the sync that followed. A server that accepts the connection and then
> answers nothing holds that sync indefinitely, so the hint said "Waiting to send." with no Outbox
> to look in. The list is now republished from the store as soon as a send or a drain pass has an
> outcome. Each send attempt is also logged as it starts.

**English**

```
A message that cannot be sent yet now appears in the Outbox straight away, even while the mail server is not answering.
```

**Nederlands**

```
Een bericht dat nog niet verzonden kan worden, staat nu meteen in Postvak UIT, ook als de mailserver niet antwoordt.
```

**Deutsch**

```
Eine Nachricht, die noch nicht gesendet werden kann, erscheint jetzt sofort im Postausgang, auch wenn der Mailserver nicht antwortet.
```

**Français**

```
Un message qui ne peut pas encore être envoyé apparaît désormais aussitôt dans la Boîte d'envoi, même lorsque le serveur de messagerie ne répond pas.
```

**Español**

```
Un mensaje que todavía no se puede enviar aparece ahora de inmediato en la Bandeja de salida, aunque el servidor de correo no responda.
```

**Italiano**

```
Un messaggio che non può ancora essere inviato compare ora subito in Posta in uscita, anche quando il server di posta non risponde.
```

**Português**

```
Uma mensagem que ainda não pode ser enviada aparece agora de imediato na Caixa de saída, mesmo quando o servidor de correio não responde.
```
