# A mail server's connection limit is no longer reported as a wrong password

Platforms: all
Bump: patch

> The engine read every refused IMAP `LOGIN` and every refused SMTP `AUTH` as a bad credential.
> Yahoo refuses the sixth session opened within a few seconds with `NO [LIMIT]`, and the app
> opens one session per synced folder, so a Yahoo account added with a correct app password was
> told its password was wrong, and an existing account hitting the limit at launch raised the
> reconnect prompt. The engine now classifies `[LIMIT]` at `LOGIN` and a 4xx at `AUTH` as a rate
> limit, so the first login no longer becomes `SigninRejected`. Adding a Yahoo account still fails
> until one connection serves several folders; this change only stops the failure being blamed on
> the password.

**English**

```
When a mail server temporarily turns a sign-in away because of a connection limit, the app no longer says your password is wrong or asks you to sign in again.
```

**Nederlands**

```
Als een mailserver een aanmelding tijdelijk weigert vanwege een limiet op verbindingen, zegt de app niet langer dat je wachtwoord onjuist is en vraagt hij niet opnieuw om je aan te melden.
```

**Deutsch**

```
Wenn ein Mailserver eine Anmeldung wegen einer Verbindungsgrenze vorübergehend ablehnt, meldet die App nicht mehr, Ihr Passwort sei falsch, und fordert Sie nicht erneut zur Anmeldung auf.
```

**Français**

```
Lorsqu'un serveur de messagerie refuse temporairement une connexion en raison d'une limite de connexions, l'app n'indique plus que votre mot de passe est incorrect et ne vous demande plus de vous reconnecter.
```

**Español**

```
Cuando un servidor de correo rechaza temporalmente un inicio de sesión por un límite de conexiones, la aplicación ya no indica que la contraseña es incorrecta ni le pide que vuelva a iniciar sesión.
```

**Italiano**

```
Quando un server di posta rifiuta temporaneamente un accesso per un limite di connessioni, l'app non indica più che la password è errata né chiede di accedere di nuovo.
```

**Português**

```
Quando um servidor de correio recusa temporariamente um início de sessão devido a um limite de ligações, a aplicação já não indica que a palavra-passe está incorreta nem pede para iniciar sessão novamente.
```
