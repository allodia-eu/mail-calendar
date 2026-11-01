# Google sign-in on a desktop no longer fails when the browser asks for more than the redirect

Platforms: macos, windows
Bump: patch

> The loopback listener took the first request that reached its port as the redirect, so a
> browser fetching `/favicon.ico` first failed a completed sign-in with "callback missing state".
> macOS and Windows now take only a request carrying `state` with a `code` or an `error`
> (`is_oauth_callback`, the rule Linux already applied, now shared), read the whole request first,
> and answer anything else with a 400.

**English**

```
Signing in with Google on a computer no longer fails after you have approved it in the browser.
```

**Nederlands**

```
Aanmelden met Google op een computer mislukt niet meer nadat je het in de browser hebt goedgekeurd.
```

**Deutsch**

```
Die Anmeldung mit Google auf einem Computer schlägt nicht mehr fehl, nachdem Sie sie im Browser bestätigt haben.
```

**Français**

```
La connexion avec Google sur un ordinateur n’échoue plus après que vous l’avez approuvée dans le navigateur.
```

**Español**

```
Iniciar sesión con Google en un ordenador ya no falla después de haberlo aprobado en el navegador.
```

**Italiano**

```
L’accesso con Google su un computer non fallisce più dopo che lo hai approvato nel browser.
```

**Português**

```
Iniciar sessão com o Google num computador já não falha depois de o aprovar no navegador.
```
