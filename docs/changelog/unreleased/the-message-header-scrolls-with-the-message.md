# The message header scrolls with the message

Platforms: ios
Bump: minor

> Who sent a message, who else got it and what it carries belong to the mail rather than to the
> pane, so on iPhone and iPad they now go up with the body and only the action row stays
> ([`docs/reading-actions.md`](../../reading-actions.md)). The reading view is one `ScrollView`
> rather than a header over a body that scrolls itself: the web view is laid out at the whole
> message's height, which WebKit reports, so it has nothing left to scroll and the one scroll moves
> both. A header drawn *over* the body looks identical until a finger lands on it, because that
> view takes the drag, and a message whose header fills the screen (an invitation card, or twenty
> attachments) could then not be scrolled at all. Laying the web view out at the message's height
> is also what lets WebKit apply its own fit to an over-wide message
> ([`docs/reading-zoom.md`](../../reading-zoom.md)); held to the pane's height it does not.
> `ReadingHeaderScrollTests` holds it, on the invitation, whose card is the one header in the
> showcase dataset guaranteed to outgrow a large phone.

**English**

```
On iPhone and iPad the sender, recipients and attachments now scroll away with the message, leaving reply, archive and delete in place at the top.
```

**Nederlands**

```
Op de iPhone en iPad schuiven de afzender, de geadresseerden en de bijlagen nu mee met het bericht, terwijl beantwoorden, archiveren en verwijderen bovenaan blijven staan.
```

**Deutsch**

```
Auf iPhone und iPad scrollen Absender, Empfänger und Anhänge jetzt mit der Nachricht mit, während Antworten, Archivieren und Löschen oben stehen bleiben.
```

**Français**

```
Sur iPhone et iPad, l'expéditeur, les destinataires et les pièces jointes défilent désormais avec le message, tandis que répondre, archiver et supprimer restent en haut.
```

**Español**

```
En el iPhone y el iPad, el remitente, los destinatarios y los archivos adjuntos ahora se desplazan con el mensaje, mientras que responder, archivar y eliminar se quedan arriba.
```

**Italiano**

```
Su iPhone e iPad il mittente, i destinatari e gli allegati ora scorrono insieme al messaggio, mentre rispondi, archivia ed elimina restano in alto.
```

**Português**

```
No iPhone e no iPad, o remetente, os destinatários e os anexos passam a deslocar-se com a mensagem, enquanto responder, arquivar e eliminar ficam no topo.
```
