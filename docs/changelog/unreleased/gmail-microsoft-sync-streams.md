# Gmail and Microsoft mail appears as it downloads, and stays when a sync stops partway

Platforms: all
Bump: patch

> The wrappers that refresh a Google or Microsoft token collected a whole pass before passing any
> of it on, so nothing was stored until the last message had arrived, and a pass the server
> stopped partway stored nothing at all. Gmail stops a large pass once its per-minute quota is
> spent, so widening a Gmail account's sync depth to all mail never stored a message. The three
> reconnecting wrappers now share one stream that hands each batch on as it arrives.

**English**

```
Gmail and Microsoft accounts now show messages as they download, and keep them if a sync is stopped partway.
```

**Nederlands**

```
Gmail- en Microsoft-accounts tonen berichten nu tijdens het downloaden en bewaren ze als een synchronisatie halverwege stopt.
```

**Deutsch**

```
Gmail- und Microsoft-Konten zeigen Nachrichten jetzt schon während des Herunterladens und behalten sie, wenn eine Synchronisierung abbricht.
```

**Français**

```
Les comptes Gmail et Microsoft affichent désormais les messages au fil du téléchargement et les conservent si une synchronisation s’interrompt.
```

**Español**

```
Las cuentas de Gmail y Microsoft muestran ahora los mensajes mientras se descargan y los conservan si una sincronización se detiene a medias.
```

**Italiano**

```
Gli account Gmail e Microsoft ora mostrano i messaggi durante il download e li conservano se una sincronizzazione si interrompe.
```

**Português**

```
As contas Gmail e Microsoft mostram agora as mensagens à medida que são transferidas e mantêm-nas se uma sincronização parar a meio.
```
