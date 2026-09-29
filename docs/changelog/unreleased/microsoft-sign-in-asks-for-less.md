# Microsoft sign-in asks for less

Platforms: all
Bump: patch

> Drops `openid`, `profile` and `email` from the Graph request: nothing read the ID token they
> bought. An organisation that approves each permission by hand now reviews seven, each with a call
> site named in `docs/provider-oauth.md`. Existing grants keep what they hold; nobody is asked to
> sign in again.

**English**

```
Signing in with a Microsoft account no longer asks for permissions the app does not use.
```

**Nederlands**

```
Inloggen met een Microsoft-account vraagt niet langer om rechten die de app niet gebruikt.
```

**Deutsch**

```
Die Anmeldung mit einem Microsoft-Konto fragt nicht mehr nach Berechtigungen, die die App nicht verwendet.
```

**Français**

```
La connexion avec un compte Microsoft ne demande plus d'autorisations que l'application n'utilise pas.
```

**Español**

```
Iniciar sesión con una cuenta de Microsoft ya no pide permisos que la aplicación no utiliza.
```

**Italiano**

```
L'accesso con un account Microsoft non chiede più autorizzazioni che l'app non usa.
```

**Português**

```
Iniciar sessão com uma conta Microsoft já não pede permissões que a aplicação não utiliza.
```
