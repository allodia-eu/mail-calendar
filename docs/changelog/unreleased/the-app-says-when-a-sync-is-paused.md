# The app says when a sync is paused, and when it continues

Platforms: all
Bump: minor

> A rate limit had become invisible. The previous change stopped it reading as an outage, which
> was right, but left the account looking entirely healthy while no mail arrived for it, and the
> user with no way to tell a paused sync from a quiet mailbox. The status line the footer already
> draws now says "Sync paused" in place of the background-sync note, with the account and when
> syncing continues behind a hover on a computer, and spelled out in the line itself on a phone,
> which has no hover. The wait is the server's own and is stated as an approximation, rounded up
> to whole minutes so it is never promised early; about two Gmail refusals in three name no
> window at all, and those say "shortly" rather than inventing a figure. The notice takes no row
> of its own, so nothing moves under the pointer, and it clears itself the moment the wait
> elapses. The sentence is the accessibility name on every platform, so a screen reader hears the
> wait and not just the label.

**English**

```
When a mail provider asks the app to slow down, the status line below your messages now says so, and says when syncing continues.
```

**Nederlands**

```
Wanneer een e-mailprovider de app vraagt om het rustiger aan te doen, meldt de statusregel onder uw berichten dat nu, met daarbij wanneer de synchronisatie verdergaat.
```

**Deutsch**

```
Wenn ein E-Mail-Anbieter die App bittet, langsamer zu machen, weist die Statuszeile unter Ihren Nachrichten jetzt darauf hin und nennt, wann die Synchronisierung fortgesetzt wird.
```

**Français**

```
Lorsqu'un fournisseur de messagerie demande à l'application de ralentir, la ligne d'état sous vos messages l'indique désormais et précise quand la synchronisation reprend.
```

**Español**

```
Cuando un proveedor de correo pide a la aplicación que vaya más despacio, la línea de estado situada bajo sus mensajes ahora lo indica y señala cuándo continuará la sincronización.
```

**Italiano**

```
Quando un provider di posta chiede all'app di rallentare, la riga di stato sotto i messaggi ora lo segnala e indica quando riprende la sincronizzazione.
```

**Português**

```
Quando um fornecedor de e-mail pede à aplicação para abrandar, a linha de estado por baixo das suas mensagens passa a indicá-lo e a dizer quando a sincronização continua.
```
