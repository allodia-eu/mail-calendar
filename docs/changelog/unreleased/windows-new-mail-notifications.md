# New-mail notifications on Windows

Platforms: windows
Bump: minor

> The Windows client now raises a desktop notification when its live runtime commits inbound Inbox
> mail, through the Windows App SDK's `AppNotificationManager`, which registers in both build
> shapes (MSIX identity when packaged, an AppUserModelId for the running executable when not). It
> uses the same `collect_cached_new_mail` seam Linux does, called off the `MailboxList` signal, so
> there is no second network schedule and the cadence stays whatever the account's push/poll
> setting is. One notification per message, keyed by the stable message key, grouped per account;
> keying per account instead would let the next pass replace a message the user had not seen, after
> the marks had already moved past it. What a pass says is `NewMailNotices`, which is WinUI- and
> catalog-free and unit-tested; the toast itself is a thin adapter. Settings gains the
> Notifications category the taxonomy has always reserved for slot 7, with the shared catalog's
> copy. Clicking a notification brings the app forward; opening the message it names is a
> follow-up.

**English**

```
New mail now raises a desktop notification, showing who it is from, what it is about and how the message begins. Turn it off in Settings → Notifications.
```

**Nederlands**

```
Nieuwe e-mail geeft nu een melding op het bureaublad, met de afzender, het onderwerp en het begin van het bericht. Uitschakelen kan bij Instellingen → Meldingen.
```

**Deutsch**

```
Neue E-Mails erscheinen jetzt als Benachrichtigung auf dem Desktop, mit Absender, Betreff und dem Anfang der Nachricht. Sie können das unter Einstellungen → Benachrichtigungen abschalten.
```

**Français**

```
Les nouveaux messages donnent désormais lieu à une notification sur le bureau, indiquant l'expéditeur, l'objet et le début du message. Vous pouvez la désactiver dans Réglages → Notifications.
```

**Español**

```
El correo nuevo ahora muestra una notificación en el escritorio, con el remitente, el asunto y el comienzo del mensaje. Puede desactivarla en Ajustes → Notificaciones.
```

**Italiano**

```
La posta in arrivo ora mostra una notifica sul desktop, con il mittente, l'oggetto e l'inizio del messaggio. Puoi disattivarla in Impostazioni → Notifiche.
```

**Português**

```
As mensagens novas passam a mostrar uma notificação no ambiente de trabalho, com o remetente, o assunto e o início da mensagem. Pode desativá-la em Definições → Notificações.
```
