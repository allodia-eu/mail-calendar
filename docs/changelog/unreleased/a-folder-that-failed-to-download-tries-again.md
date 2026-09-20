# A folder that failed to download tries again when you open it

Platforms: all
Bump: patch

> Opening a folder the account did not bind at sign-in downloads it once, and the open threw the
> engine's report away. So it could not tell a folder that had fetched two thousand messages from
> one that had failed outright: the log said the same sentence either way, and either way the open
> counted as done. A folder is attempted once per session, so a download that failed left it empty
> until the app was restarted. The report is now read, through the same logging the push path
> already used, so the log carries the engine's own counts or its error; a failed download is
> forgotten, so the next open runs it again; and the open answers honestly whether anything landed,
> so one that downloaded nothing no longer repaints the list for an identical snapshot.

**English**

```
A folder whose mail could not be downloaded when you opened it now tries again the next time you open it, instead of staying empty until you restart the app.
```

**Nederlands**

```
Een map waarvan de e-mail niet kon worden opgehaald toen u hem opende, probeert het nu opnieuw wanneer u hem weer opent, in plaats van leeg te blijven tot u de app herstart.
```

**Deutsch**

```
Ein Ordner, dessen Nachrichten sich beim Öffnen nicht laden ließen, versucht es jetzt beim nächsten Öffnen erneut, statt bis zum Neustart der App leer zu bleiben.
```

**Français**

```
Un dossier dont les messages n'ont pas pu être téléchargés à l'ouverture réessaie désormais à la prochaine ouverture, au lieu de rester vide jusqu'au redémarrage de l'application.
```

**Español**

```
Una carpeta cuyos mensajes no se pudieron descargar al abrirla ahora vuelve a intentarlo la próxima vez que la abra, en vez de quedarse vacía hasta reiniciar la aplicación.
```

**Italiano**

```
Una cartella i cui messaggi non è stato possibile scaricare all'apertura ora riprova alla prossima apertura, invece di restare vuota fino al riavvio dell'app.
```

**Português**

```
Uma pasta cujas mensagens não foi possível transferir ao abri-la passa a tentar de novo na próxima vez que a abrir, em vez de ficar vazia até reiniciar a aplicação.
```
