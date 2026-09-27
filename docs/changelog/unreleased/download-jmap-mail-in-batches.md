# Download a JMAP account's messages in batches where the server offers it

Platforms: all
Bump: patch

> On a JMAP server that offers `Blob/get` (RFC 9404), such as Stalwart and Thundermail, the
> body warm now reads 25 messages per request instead of one. Stalwart also limits an account to
> about 1,000 requests a minute, which one request per message reached on any sizeable mailbox.
> When a server does refuse for now, the warm waits (its stated time, or a minute where it names
> none) and carries on in the same pass, rather than leaving those messages for the next sync.

**English**

```
The first sync of a JMAP account downloads its messages faster on servers that allow many to be fetched at once.
```

**Nederlands**

```
De eerste synchronisatie van een JMAP-account downloadt berichten sneller op servers die er veel tegelijk laten ophalen.
```

**Deutsch**

```
Die erste Synchronisierung eines JMAP-Kontos lädt Nachrichten schneller herunter, wenn der Server viele auf einmal ausliefert.
```

**Français**

```
La première synchronisation d'un compte JMAP télécharge les messages plus rapidement sur les serveurs qui en fournissent plusieurs à la fois.
```

**Español**

```
La primera sincronización de una cuenta JMAP descarga los mensajes más rápido en los servidores que permiten obtener muchos a la vez.
```

**Italiano**

```
La prima sincronizzazione di un account JMAP scarica i messaggi più velocemente sui server che ne consentono molti alla volta.
```

**Português**

```
A primeira sincronização de uma conta JMAP descarrega as mensagens mais depressa nos servidores que permitem obter muitas de uma só vez.
```
