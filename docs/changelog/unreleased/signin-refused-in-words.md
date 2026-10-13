# Say a refused sign-in in words, not as the server's reply

Platforms: all
Bump: patch

> A wrong password or API token, at setup, in an account's servers or when signing in again, showed
> the server's raw reply (for example "sign-in rejected: JMAP HTTP 401: {…}"). The core now reports
> a refused sign-in as its own kind of error, and each client says it in the person's language;
> the reply goes to the log.

**English**

```
A wrong password or token now says the server didn’t accept the sign-in, instead of showing the server’s technical reply.
```

**Nederlands**

```
Een verkeerd wachtwoord of token meldt nu dat de server de aanmelding niet accepteerde, in plaats van het technische antwoord van de server te tonen.
```

**Deutsch**

```
Ein falsches Passwort oder Token meldet jetzt, dass der Server die Anmeldung nicht akzeptiert hat, statt die technische Antwort des Servers zu zeigen.
```

**Français**

```
Un mot de passe ou un jeton incorrect indique désormais que le serveur n’a pas accepté la connexion, au lieu d’afficher sa réponse technique.
```

**Español**

```
Una contraseña o un token incorrectos indican ahora que el servidor no aceptó el inicio de sesión, en lugar de mostrar su respuesta técnica.
```

**Italiano**

```
Una password o un token errati ora indicano che il server non ha accettato l’accesso, invece di mostrare la sua risposta tecnica.
```

**Português**

```
Uma palavra-passe ou um token incorretos indicam agora que o servidor não aceitou o início de sessão, em vez de mostrar a sua resposta técnica.
```
