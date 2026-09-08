# Signing out reaches the right account service

Platforms: all
Bump: patch

> The stored grant carries the account service's sign-out endpoint so that signing out can erase
> the grant first and touch no network: one that had to discover before it could erase would be a
> sign-out that fails offline. That copy was written once, at sign-in, and never revisited, so when
> the account service moved to another host every existing grant went on pointing at the old one.
> Signing out then asked a service that no longer held the session to end it, and the session it
> did hold stayed open. Discovery already runs once per launch to build the token refresher, which
> puts the current endpoint in hand for free at exactly the moment somebody is using the account,
> so `adopt_discovered_end_session` now takes it whenever it differs from the stored copy. The
> write is skipped when the value has not moved, because a store write per refresh is a keychain
> prompt's worth of noise on some hosts. Sign-out stays offline and instant and settles for at most
> one launch of lag, rather than discovering at sign-out and having to explain itself on a plane.

**English**

```
Signing out of an Allodia account now ends the session on the right server, even after the account service has moved.
```

**Nederlands**

```
Uitloggen bij een Allodia-account beëindigt de sessie nu op de juiste server, ook nadat de accountdienst is verhuisd.
```

**Deutsch**

```
Die Abmeldung von einem Allodia-Konto beendet die Sitzung jetzt auf dem richtigen Server, auch nachdem der Kontodienst umgezogen ist.
```

**Français**

```
La déconnexion d'un compte Allodia met désormais fin à la session sur le bon serveur, même après un déplacement du service de comptes.
```

**Español**

```
Cerrar la sesión de una cuenta Allodia ahora la termina en el servidor correcto, incluso después de que el servicio de cuentas se haya trasladado.
```

**Italiano**

```
La disconnessione da un account Allodia ora termina la sessione sul server corretto, anche dopo lo spostamento del servizio account.
```

**Português**

```
Terminar a sessão numa conta Allodia passa a encerrá-la no servidor correto, mesmo depois de o serviço de contas ter mudado.
```
