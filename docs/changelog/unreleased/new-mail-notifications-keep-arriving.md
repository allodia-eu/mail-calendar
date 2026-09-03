# New-mail notifications keep arriving

Platforms: linux
Bump: patch

> `ashpd` keeps one session-bus connection for the whole process, and zbus drives it from the
> runtime that opened it. Both the secure store (`oo7` asks the Secret portal for the keyring key
> inside a sandbox) and `notifications::post` built a runtime of their own, so the first one to
> finish took the shared connection's reader with it and every later portal call awaited a reply
> that could never arrive: no error, no timeout, a thread parked for the life of the process. The
> new-mail scan is held open across that call, so the wedge stopped the client noticing mail at
> all, not merely notifying about it. One process-global runtime now serves every portal caller
> (`host_runtime`), and the post is bounded so a portal that stops answering costs one pass rather
> than the session. The suite's disabled-notifications leg asserts an absence and so passed
> silently throughout; it now runs the enabled half first and measures the silence against it.

**English**

```
New-mail notifications keep arriving all session, instead of stopping after the first one.
```

**Nederlands**

```
Meldingen over nieuwe e-mail blijven de hele sessie komen in plaats van na de eerste te stoppen.
```

**Deutsch**

```
Benachrichtigungen über neue E-Mails kommen die ganze Sitzung lang und hören nicht nach der ersten
auf.
```

**Français**

```
Les notifications de nouveaux messages continuent d'arriver pendant toute la session au lieu de
s'arrêter après la première.
```

**Español**

```
Los avisos de correo nuevo siguen llegando durante toda la sesión en lugar de detenerse tras el
primero.
```

**Italiano**

```
Le notifiche di nuova posta continuano ad arrivare per tutta la sessione invece di fermarsi dopo
la prima.
```

**Português**

```
As notificações de correio novo continuam a chegar durante toda a sessão em vez de pararem depois
da primeira.
```
