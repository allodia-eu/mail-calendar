# An empty folder says why it is empty

Platforms: all
Bump: minor

> The badge beside a folder counts what the server holds, over all time, while the list can only
> show what sync depth kept (`docs/folder-pane.md`, rules 5 and 20). A folder whose mail predates
> the depth therefore badged its unread above no rows at all, and nothing on screen reconciled the
> two numbers: unqualified, the empty list claimed the folder was empty, which is the one reading
> the user cannot act on. The list now states which of the two it is, from
> `MailboxListSnapshot::empty_reason` over the narrowest depth of the accounts in view, and offers
> the sync-depth setting only where widening would actually find something. The setting's own
> description said older messages download when a folder is opened, which is not what an on-demand
> open does: it applies the same depth. That sentence is corrected in every locale.

**English**

```
A folder that looks empty now says whether it really is empty or whether older mail is still on the server, with a link to change how far back this account syncs.
```

**Nederlands**

```
Een map die leeg lijkt, geeft nu aan of hij echt leeg is of dat er nog oudere e-mail op de server staat, met een link om in te stellen hoe ver terug dit account synchroniseert.
```

**Deutsch**

```
Ein Ordner, der leer aussieht, gibt jetzt an, ob er wirklich leer ist oder ob ältere Nachrichten noch auf dem Server liegen, mit einem Link zur Einstellung, wie weit dieses Konto zurück synchronisiert.
```

**Français**

```
Un dossier qui semble vide indique désormais s'il est réellement vide ou si des messages plus anciens sont encore sur le serveur, avec un lien pour régler jusqu'où ce compte se synchronise.
```

**Español**

```
Una carpeta que parece vacía ahora indica si está realmente vacía o si aún hay mensajes más antiguos en el servidor, con un enlace para ajustar hasta dónde sincroniza esta cuenta.
```

**Italiano**

```
Una cartella che sembra vuota ora indica se è davvero vuota o se sul server ci sono ancora messaggi più vecchi, con un collegamento per regolare fino a quando sincronizza questo account.
```

**Português**

```
Uma pasta que parece vazia passa a indicar se está mesmo vazia ou se ainda há mensagens mais antigas no servidor, com uma ligação para definir até quando esta conta sincroniza.
```
