# A server asking us to slow down is no longer reported as an outage

Platforms: all
Bump: patch

> A provider that refuses a request for rate has answered: promptly, and with a number. The app
> read every failed folder the same way, so a Gmail account that spent its per-minute quota
> mid-sync came up badged "can't reach the server", which points at the one thing that is working.
> The engine now classifies the refusal and, where the server names when the limit clears, reports
> that instant rather than sleeping a task through it; the app reads the class, so a throttled
> account stays reachable and keeps whatever prompt it already had, and the background poll sits
> out the rest of the window the server named before its next attempt instead of walking into the
> same refusal. Where no instant is named, which is about two Gmail refusals in three, the poll
> keeps its own schedule. The diagnostic log says how long was asked for, where before it could
> only say how little it had waited.

**English**

```
When a mail provider asks the app to slow down, the app now waits as long as it was asked and stops showing "can't reach the server" for an account whose server answered perfectly well.
```

**Nederlands**

```
Wanneer een e-mailprovider de app vraagt om het rustiger aan te doen, wacht de app nu zo lang als gevraagd en toont hij niet langer "kan de server niet bereiken" voor een account waarvan de server prima antwoordde.
```

**Deutsch**

```
Wenn ein E-Mail-Anbieter die App bittet, langsamer zu machen, wartet die App jetzt so lange wie gewünscht und zeigt für ein Konto, dessen Server einwandfrei geantwortet hat, nicht mehr „Server nicht erreichbar“ an.
```

**Français**

```
Lorsqu'un fournisseur de messagerie demande à l'application de ralentir, celle-ci attend désormais le délai demandé et n'affiche plus « serveur injoignable » pour un compte dont le serveur a parfaitement répondu.
```

**Español**

```
Cuando un proveedor de correo pide a la aplicación que vaya más despacio, ahora espera el tiempo solicitado y deja de mostrar «no se puede conectar con el servidor» en una cuenta cuyo servidor respondió sin problemas.
```

**Italiano**

```
Quando un provider di posta chiede all'app di rallentare, l'app ora attende il tempo richiesto e non mostra più "impossibile raggiungere il server" per un account il cui server ha risposto perfettamente.
```

**Português**

```
Quando um fornecedor de e-mail pede à aplicação para abrandar, esta passa a esperar o tempo pedido e deixa de mostrar "não é possível contactar o servidor" numa conta cujo servidor respondeu sem problemas.
```
