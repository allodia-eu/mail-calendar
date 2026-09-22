# New-mail notifications on macOS

Platforms: macos
Bump: minor

> macOS was the last client without them. It needed no schedule of its own: the desktop's live
> runtime already delivers, so a `MailboxList` signal is the moment it committed something, and the
> client reads that settled cache through `collect_cached_new_mail`, the same seam Windows and Linux
> use. A burst of signals collapses into one follow-up scan, and the scan runs off the main actor
> because the core blocks its thread. What a pass says is a pure projection (`NewMailNotices`,
> pinned by `NewMailNoticesTests`) and `MailNotifier` only hands each notice to
> `UNUserNotificationCenter`, one per message keyed by the stable message key, grouped per account,
> with a summary for whatever the core's preview cap left out. No dock badge: the pass total is
> what has just arrived, not what is unread. Settings gains the Notifications category it has been
> holding a slot for, and the permission is asked for on the same terms as iOS, once an account
> exists and the usage-statistics question is answered, so two system prompts never stack.
>
> Clicking one opens the message it names, which Android already did and the other three clients
> still do not. The click is answered by a process-wide delegate while the reading pane belongs to
> a scene, so the `(account, message key)` pair waits in a box the shell drains, the shape the
> Share Extension's drop box already has. A summary names no message and so opens the app alone,
> and a message the list does not hold yet is waited for over one account switch rather than
> opened against whatever is on screen, then dropped instead of dragging the view back to the
> mailbox on every later snapshot.

**English**

```
Mail arriving while Allodia Mail & Calendar is open now raises a notification on macOS, saying who it is from, what it is about and how it begins. Clicking one opens that message. Turn it off in Settings → Notifications.
```

**Nederlands**

```
E-mail die binnenkomt terwijl Allodia Mail & Calendar openstaat, geeft nu een melding op macOS: van wie het is, waar het over gaat en hoe het begint. Klik erop om dat bericht te openen. Uit te zetten in Instellingen → Meldingen.
```

**Deutsch**

```
E-Mails, die eintreffen, während Allodia Mail & Calendar geöffnet ist, erscheinen unter macOS jetzt als Mitteilung: von wem sie sind, worum es geht und wie sie beginnen. Ein Klick darauf öffnet die Nachricht. Abschaltbar unter Einstellungen → Benachrichtigungen.
```

**Français**

```
Les messages qui arrivent pendant qu'Allodia Mail & Calendar est ouvert donnent désormais lieu à une notification sur macOS : de qui ils viennent, de quoi ils parlent et comment ils commencent. Un clic ouvre le message concerné. Désactivable dans Réglages → Notifications.
```

**Español**

```
El correo que llega mientras Allodia Mail & Calendar está abierto ahora genera una notificación en macOS: de quién es, de qué trata y cómo empieza. Al hacer clic se abre ese mensaje. Se desactiva en Ajustes → Notificaciones.
```

**Italiano**

```
La posta che arriva mentre Allodia Mail & Calendar è aperto ora genera una notifica su macOS: da chi arriva, di cosa parla e come inizia. Facendo clic si apre quel messaggio. Si disattiva in Impostazioni → Notifiche.
```

**Português**

```
As mensagens que chegam enquanto o Allodia Mail & Calendar está aberto passam a gerar uma notificação no macOS: de quem são, do que tratam e como começam. Ao clicar, abre essa mensagem. Pode desativar em Definições → Notificações.
```
