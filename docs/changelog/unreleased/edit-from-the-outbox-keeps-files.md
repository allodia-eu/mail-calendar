# Editing a message in the Outbox keeps its files

Platforms: all
Bump: patch

> Edit on a queued send handed the composer the recipients, subject and text only, so files and a
> reply's threading were dropped. It now moves the message back into Drafts first and opens the
> composer on that draft holding every file. A sent message also no longer leaves a copy in Drafts
> when the server refuses it: the Outbox keeps it. Sending again a message whose delivery was not
> confirmed now asks first.

**English**

```
Editing a message in the Outbox now moves it back to Drafts with its attachments, and sending again a message that may already have arrived asks first.
```

**Nederlands**

```
Een bericht in Postvak UIT bewerken zet het nu terug in Concepten, met de bijlagen, en een bericht dat misschien al is aangekomen opnieuw verzenden vraagt eerst om bevestiging.
```

**Deutsch**

```
Eine Nachricht im Postausgang zu bearbeiten, verschiebt sie jetzt mit ihren Anhängen zurück in die Entwürfe, und eine möglicherweise bereits zugestellte Nachricht erneut zu senden, fragt zuerst nach.
```

**Français**

```
Modifier un message de la Boîte d'envoi le replace désormais dans les Brouillons avec ses pièces jointes, et renvoyer un message peut-être déjà arrivé demande d'abord confirmation.
```

**Español**

```
Editar un mensaje de la Bandeja de salida ahora lo devuelve a Borradores con sus adjuntos, y volver a enviar un mensaje que quizá ya llegó pide confirmación antes.
```

**Italiano**

```
Modificare un messaggio in Posta in uscita ora lo riporta in Bozze con i suoi allegati, e inviare di nuovo un messaggio forse già arrivato chiede prima conferma.
```

**Português**

```
Editar uma mensagem na Caixa de saída passa a devolvê-la aos Rascunhos com os anexos, e enviar novamente uma mensagem que talvez já tenha chegado pede confirmação primeiro.
```
