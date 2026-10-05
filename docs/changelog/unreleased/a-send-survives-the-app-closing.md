# A send survives the app closing, and a refused one is kept

Platforms: all
Bump: minor

> A send cut off by the app ending stayed "Sending" for good, and one the server refused left the
> Outbox and was gone. The engine now records the point where the server could act on a message:
> at start-up a send cut off before it is sent again, one cut off after it asks the user whether it
> arrived, and a refused send stays in the Outbox until the user acts. Every Outbox step is logged.

**English**

```
If the app closes while a message is being sent, it is no longer lost or sent twice: it is sent at the next start, or the Outbox asks whether it arrived. A message the server refused now stays in the Outbox, to send again, edit or discard.
```

**Nederlands**

```
Als de app sluit terwijl een bericht wordt verzonden, gaat het niet meer verloren en wordt het niet twee keer verzonden: het gaat bij de volgende start alsnog weg, of Postvak UIT vraagt of het is aangekomen. Een bericht dat de server weigerde, blijft nu in Postvak UIT staan om opnieuw te verzenden, te bewerken of weg te gooien.
```

**Deutsch**

```
Wird die App geschlossen, während eine Nachricht gesendet wird, geht sie nicht mehr verloren und wird nicht doppelt gesendet: Sie wird beim nächsten Start gesendet, oder der Postausgang fragt, ob sie angekommen ist. Eine vom Server abgelehnte Nachricht bleibt jetzt im Postausgang, um sie erneut zu senden, zu bearbeiten oder zu verwerfen.
```

**Français**

```
Si l'app se ferme pendant l'envoi d'un message, celui-ci n'est plus perdu ni envoyé deux fois : il part au prochain démarrage, ou la Boîte d'envoi demande s'il est arrivé. Un message refusé par le serveur reste désormais dans la Boîte d'envoi, pour le renvoyer, le modifier ou le supprimer.
```

**Español**

```
Si la app se cierra mientras se envía un mensaje, ya no se pierde ni se envía dos veces: se envía al volver a abrirla, o la Bandeja de salida pregunta si ha llegado. Un mensaje que el servidor rechazó se queda ahora en la Bandeja de salida para volver a enviarlo, editarlo o descartarlo.
```

**Italiano**

```
Se l'app si chiude mentre un messaggio viene inviato, non va più perso né viene inviato due volte: parte al prossimo avvio, oppure Posta in uscita chiede se è arrivato. Un messaggio rifiutato dal server ora resta in Posta in uscita, per inviarlo di nuovo, modificarlo o eliminarlo.
```

**Português**

```
Se a app fechar enquanto uma mensagem está a ser enviada, esta já não se perde nem é enviada duas vezes: é enviada no próximo arranque, ou a Caixa de saída pergunta se chegou. Uma mensagem recusada pelo servidor fica agora na Caixa de saída, para a enviar novamente, editar ou descartar.
```
