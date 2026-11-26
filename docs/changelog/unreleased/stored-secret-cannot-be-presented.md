# A stored sign-in the server could never accept asks to be updated

Platforms: all
Bump: patch

> The engine refuses, before sending anything, a stored password or token its protocol cannot
> carry (a Basic username holding `:`, a line break in an IMAP password, an API token a header
> cannot carry). The first connect of an account that stores its secret now reads that as a
> refused sign-in (`AccountError::SigninRejected`, `docs/provider-oauth.md` rule 12). An API
> token a header cannot carry failed every request as `Retryable` before, which read as a server
> that could not be reached; a Basic username holding `:` went out and was refused, which already
> asked for a new sign-in.

**English**

```
A stored password or token that the server could never accept, such as an API token containing a space, now asks you to update it, instead of showing the server as unreachable.
```

**Nederlands**

```
Een opgeslagen wachtwoord of token dat de server nooit kan accepteren, zoals een API-token met een spatie, vraagt u nu om het bij te werken, in plaats van te melden dat de server onbereikbaar is.
```

**Deutsch**

```
Ein gespeichertes Passwort oder Token, das der Server nie annehmen könnte, etwa ein API-Token mit einem Leerzeichen, fordert Sie jetzt auf, es zu ändern, statt den Server als nicht erreichbar anzuzeigen.
```

**Français**

```
Un mot de passe ou un jeton enregistré que le serveur ne pourrait jamais accepter, par exemple un jeton d’API contenant une espace, vous invite désormais à le mettre à jour, au lieu d’indiquer que le serveur est injoignable.
```

**Español**

```
Una contraseña o un token guardado que el servidor nunca podría aceptar, por ejemplo un token de API con un espacio, le pide ahora que lo actualice, en lugar de mostrar el servidor como inaccesible.
```

**Italiano**

```
Una password o un token salvato che il server non potrebbe mai accettare, ad esempio un token API contenente uno spazio, ora vi chiede di aggiornarlo, invece di mostrare il server come non raggiungibile.
```

**Português**

```
Uma palavra-passe ou um token guardado que o servidor nunca poderia aceitar, por exemplo um token de API com um espaço, pede-lhe agora que o atualize, em vez de mostrar o servidor como inacessível.
```
