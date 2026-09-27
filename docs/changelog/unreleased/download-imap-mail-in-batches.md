# Download an IMAP account's messages in batches, over several connections

Platforms: all
Bump: minor

> The body warm after an IMAP sync asked for one message at a time, each paying two round
> trips, so a first sync of a real mailbox took minutes on any connection. It now asks for
> batches of messages in one request per folder, several at once over the account's
> connections, and keeps going through dropped connections and a server that allows fewer
> connections than we asked for.

**English**

```
The first sync of an IMAP account now downloads its messages several times faster, and carries on through a dropped connection.
```

**Nederlands**

```
De eerste synchronisatie van een IMAP-account downloadt berichten nu een stuk sneller en gaat door na een verbroken verbinding.
```

**Deutsch**

```
Die erste Synchronisierung eines IMAP-Kontos lädt Nachrichten jetzt um ein Vielfaches schneller herunter und setzt nach einer unterbrochenen Verbindung fort.
```

**Français**

```
La première synchronisation d'un compte IMAP télécharge désormais les messages bien plus rapidement et se poursuit après une coupure de connexion.
```

**Español**

```
La primera sincronización de una cuenta IMAP descarga ahora los mensajes mucho más rápido y continúa tras una conexión interrumpida.
```

**Italiano**

```
La prima sincronizzazione di un account IMAP ora scarica i messaggi molto più velocemente e prosegue dopo una connessione interrotta.
```

**Português**

```
A primeira sincronização de uma conta IMAP descarrega agora as mensagens muito mais depressa e continua após uma ligação interrompida.
```
