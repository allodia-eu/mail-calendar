# Share an IMAP account's connections across its folders

Platforms: all
Bump: patch

> An IMAP account opened one connection per synced folder and one per watched folder, all held
> for the life of the app, and redialled all of them at once after a network change. A server
> with a per-user connection limit refuses the excess at login, which the app read as a wrong
> password. The account now holds at most five, shared by every folder, and push watches up to
> four folders so one connection is always left to sync.

**English**

```
IMAP accounts now use at most five connections across all their folders, so a server that limits connections no longer makes your password look wrong; push now watches up to four folders per account.
```

**Nederlands**

```
IMAP-accounts gebruiken nu hoogstens vijf verbindingen voor al hun mappen, zodat een server die verbindingen beperkt uw wachtwoord niet langer onjuist laat lijken; push volgt nu tot vier mappen per account.
```

**Deutsch**

```
IMAP-Konten verwenden jetzt höchstens fünf Verbindungen für alle ihre Ordner, sodass ein Server, der Verbindungen begrenzt, Ihr Passwort nicht mehr falsch erscheinen lässt; Push überwacht jetzt bis zu vier Ordner pro Konto.
```

**Français**

```
Les comptes IMAP utilisent désormais au plus cinq connexions pour tous leurs dossiers, de sorte qu'un serveur qui limite les connexions ne fait plus paraître votre mot de passe incorrect ; le push surveille désormais jusqu'à quatre dossiers par compte.
```

**Español**

```
Las cuentas IMAP usan ahora como máximo cinco conexiones para todas sus carpetas, así que un servidor que limita las conexiones ya no hace que tu contraseña parezca incorrecta; el push vigila ahora hasta cuatro carpetas por cuenta.
```

**Italiano**

```
Gli account IMAP ora usano al massimo cinque connessioni per tutte le loro cartelle, quindi un server che limita le connessioni non fa più sembrare errata la tua password; il push ora controlla fino a quattro cartelle per account.
```

**Português**

```
As contas IMAP usam agora no máximo cinco ligações para todas as suas pastas, pelo que um servidor que limita as ligações já não faz parecer errada a sua palavra-passe; o push vigia agora até quatro pastas por conta.
```
