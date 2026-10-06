# Read and flag changes show on Yahoo and similar servers

Platforms: all
Bump: patch

> Two engine fixes. Yahoo lists its inbox as `Inbox`, which the engine took as the folder's id
> while the account sync bound the reserved `INBOX`: two copies of one mailbox, and the folder
> pane showed the one nothing kept current. The engine now lists every inbox as `INBOX`, and its
> vanished-folder step drops the stale copy on the first sync after updating. Separately, a server
> without QRESYNC (Yahoo among them) only ever sent new arrivals, so a flag change or a move made
> after the first sync never came back; its delta now reads the flags of every held message and
> the set still present. A push-folder choice stored as `Inbox` keeps naming the inbox.

**English**

```
Marking a message as read, unread or flagged on a Yahoo account, or on a mail server like it, now shows in the app, and a message moved elsewhere no longer stays in its old folder.
```

**Nederlands**

```
Als je een bericht in een Yahoo-account, of bij een vergelijkbare mailserver, als gelezen of ongelezen markeert of een markering geeft, zie je dat nu ook in de app, en een elders verplaatst bericht blijft niet meer in de oude map staan.
```

**Deutsch**

```
Wenn Sie eine Nachricht in einem Yahoo-Konto oder bei einem vergleichbaren Mailserver als gelesen oder ungelesen markieren oder sie markieren, zeigt die App das jetzt an, und eine anderswo verschobene Nachricht bleibt nicht mehr im alten Ordner.
```

**Français**

```
Marquer un message comme lu ou non lu, ou le signaler, sur un compte Yahoo ou un serveur de messagerie similaire s'affiche désormais dans l'app, et un message déplacé ailleurs ne reste plus dans son ancien dossier.
```

**Español**

```
Marcar un mensaje como leído o no leído, o marcarlo, en una cuenta de Yahoo o en un servidor de correo similar ahora se refleja en la app, y un mensaje movido en otro sitio ya no se queda en su carpeta anterior.
```

**Italiano**

```
Segnare un messaggio come letto o non letto, o contrassegnarlo, in un account Yahoo o su un server di posta simile ora si vede nell'app, e un messaggio spostato altrove non resta più nella cartella di prima.
```

**Português**

```
Marcar uma mensagem como lida ou não lida, ou sinalizá-la, numa conta Yahoo ou num servidor de correio semelhante passa a ver-se na app, e uma mensagem movida noutro sítio já não fica na pasta antiga.
```
