# Signing in no longer briefly reports that you are signed out

Platforms: all
Bump: patch

> Storing a new grant drops the access token held for the process, because the service refuses one
> minted from the grant it supersedes. Every subsystem then asks for a token at the same instant:
> the account list, the subscription card and the purchase pass. Each read the same stored refresh
> token and each presented it, and the service rotates, so all but one were replays and were
> answered `invalid_grant`. That refusal is evidence a grant is dead, so the health flipped to
> signed out and the subscription card drew the failure, until the winning refresh flipped it back
> a few milliseconds later. Measured on a device, the two refreshes were 1 ms apart and the wrong
> answer was on screen for 101 ms. `allodia_access_token` now mints under a gate, which
> `mailcal-account` has held per account since two background passes rotated one mail grant twice.
> ⚠️ **The re-check inside the gate is the fix, not the lock.** A caller that merely waited its turn
> would present the token the winner had already spent, which is the same replay one round trip
> later; the regression test counts refreshes rather than errors, so a lock without the re-check
> fails it eight times over. What is still open: this shares state per core, and Android can build a
> second core, which `CredentialOrigin` solves for mail accounts and nothing yet solves here.

**English**

```
Signing in to your Allodia account no longer briefly reports that you're signed out.
```

**Nederlands**

```
Als je je aanmeldt bij je Allodia-account, wordt niet langer even gemeld dat je bent afgemeld.
```

**Deutsch**

```
Beim Anmelden bei Ihrem Allodia-Konto wird nicht mehr kurzzeitig gemeldet, dass Sie abgemeldet sind.
```

**Français**

```
La connexion à votre compte Allodia n'indique plus brièvement que vous êtes déconnecté.
```

**Español**

```
Iniciar sesión en tu cuenta de Allodia ya no indica brevemente que has cerrado sesión.
```

**Italiano**

```
L'accesso al tuo account Allodia non segnala più per un istante che hai effettuato la disconnessione.
```

**Português**

```
Iniciar sessão na sua conta Allodia já não indica por momentos que a sessão foi terminada.
```
