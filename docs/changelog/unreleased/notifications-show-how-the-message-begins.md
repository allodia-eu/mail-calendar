# Notifications show how the message begins

Platforms: ios, android, linux
Bump: minor

> `NewMailPreview` gains the body snippet the list row already carries, so a notification and its
> row cannot quote the message differently and no second extraction exists to disagree with the
> first. It is empty until the body sync has run (an IMAP account commits headers first), and each
> client drops the line rather than drawing a blank one, which is the two-line notification they
> all had before. The slots differ per platform and the order does not: Windows has three text
> elements, Android puts the snippet on the expanded `BigTextStyle`, iOS and the Linux portal give
> one body that the subject and the snippet share. A per-account overflow summary names no single
> message, so it quotes none. `docs/privacy-policy.md` moves with it, in both locales: it
> enumerates what a lock-screen preview contains, and that enumeration is now longer. Windows is
> not on the `Platforms:` line because it gains notifications in this same release: its own note
> already describes all three lines, and a second bullet repeating one of them would read as two
> changes to a store's reader.

**English**

```
New-mail notifications now show how the message begins, after the sender and the subject.
```

**Nederlands**

```
Meldingen van nieuwe e-mail tonen nu ook hoe het bericht begint, na de afzender en het onderwerp.
```

**Deutsch**

```
Benachrichtigungen über neue E-Mails zeigen jetzt auch den Anfang der Nachricht, nach Absender und Betreff.
```

**Français**

```
Les notifications de nouveaux messages montrent désormais le début du message, après l'expéditeur et l'objet.
```

**Español**

```
Las notificaciones de correo nuevo ahora muestran cómo empieza el mensaje, después del remitente y el asunto.
```

**Italiano**

```
Le notifiche di posta in arrivo ora mostrano anche l'inizio del messaggio, dopo il mittente e l'oggetto.
```

**Português**

```
As notificações de correio novo passam a mostrar o início da mensagem, depois do remetente e do assunto.
```
