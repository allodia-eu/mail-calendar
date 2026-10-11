# Keep a first JMAP sync whole while another device changes the account

Platforms: all
Bump: patch

> A first sync of a large JMAP mailbox, calendar or address book reads it in pages. A message
> deleted on another device meanwhile moved every later one up a place, so one was skipped and
> never fetched, and a change made to a message already read was lost. The engine now pages by
> the last message it has and catches up on what changed during the sync.

**English**

```
A first sync of a JMAP account no longer misses messages when another device changes the mailbox while it runs.
```

**Nederlands**

```
Een eerste synchronisatie van een JMAP-account mist geen berichten meer wanneer een ander apparaat de mailbox ondertussen wijzigt.
```

**Deutsch**

```
Die erste Synchronisierung eines JMAP-Kontos lässt keine Nachrichten mehr aus, wenn ein anderes Gerät das Postfach währenddessen ändert.
```

**Français**

```
La première synchronisation d’un compte JMAP ne manque plus de messages lorsqu’un autre appareil modifie la boîte aux lettres pendant ce temps.
```

**Español**

```
La primera sincronización de una cuenta JMAP ya no omite mensajes cuando otro dispositivo cambia el buzón mientras se realiza.
```

**Italiano**

```
La prima sincronizzazione di un account JMAP non salta più messaggi quando un altro dispositivo modifica la casella nel frattempo.
```

**Português**

```
A primeira sincronização de uma conta JMAP já não omite mensagens quando outro dispositivo altera a caixa de correio entretanto.
```
