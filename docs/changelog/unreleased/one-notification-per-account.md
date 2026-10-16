# Two accounts' new mail no longer shares one notification

Platforms: android, linux
Bump: patch

> A notification was named by the message's key alone, and a key is unique within its account and
> never across two, so two IMAP accounts on the same server software could hand out the same key and
> the second notification replaced the first. On Android the tap intents also told messages apart by
> a hash of the key, so a tap could open a different message from the one announced. Both platforms
> now name a notification by account and key together.

**English**

```
New mail in two accounts no longer replaces one account's notification with the other's.
```

**Nederlands**

```
Nieuwe e-mail in twee accounts vervangt niet langer de melding van het ene account door die van het andere.
```

**Deutsch**

```
Neue E-Mails in zwei Konten ersetzen nicht mehr die Mitteilung des einen Kontos durch die des anderen.
```

**Français**

```
Un nouveau message dans deux comptes ne remplace plus la notification d'un compte par celle de l'autre.
```

**Español**

```
El correo nuevo en dos cuentas ya no sustituye la notificación de una cuenta por la de la otra.
```

**Italiano**

```
La nuova posta in due account non sostituisce più la notifica di un account con quella dell'altro.
```

**Português**

```
O correio novo em duas contas já não substitui a notificação de uma conta pela da outra.
```
