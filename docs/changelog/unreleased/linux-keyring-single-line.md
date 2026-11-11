# Linux keeps accounts in a passwordless keyring

Platforms: linux
Bump: patch

> gnome-keyring's plain-text keyring format keeps only the first line of a stored value, so every
> account config came back as its `capabilities` line and was left out at the next launch. Each
> value is now stored as one base64 line; a value stored before that still reads back. An account
> already cut short cannot be recovered and has to be added once more; the next launch logs it and
> raises the notice, then removes it, so the notice does not come back. A stored account left out
> at boot is now named in the diagnostic log, and one damaged entry no longer stops the others
> loading. An earlier version cannot read an entry this one wrote, so going back means adding
> accounts again.

**English**

```
On Linux, accounts are now kept between launches when the system keyring has no password; one that disappeared has to be added once more, and an earlier version cannot read accounts saved by this one.
```

**Nederlands**

```
Op Linux blijven accounts nu bewaard tussen het opstarten wanneer de systeemsleutelbos geen wachtwoord heeft; een verdwenen account moet je nog één keer toevoegen, en een eerdere versie kan accounts die deze versie heeft opgeslagen niet lezen.
```

**Deutsch**

```
Unter Linux bleiben Konten jetzt zwischen zwei Starts erhalten, wenn der System-Schlüsselbund kein Passwort hat; ein verschwundenes Konto müssen Sie noch einmal hinzufügen, und eine frühere Version kann von dieser Version gespeicherte Konten nicht lesen.
```

**Français**

```
Sous Linux, les comptes sont désormais conservés d’un lancement à l’autre lorsque le trousseau du système n’a pas de mot de passe ; un compte disparu doit être ajouté une nouvelle fois, et une version antérieure ne peut pas lire les comptes enregistrés par celle-ci.
```

**Español**

```
En Linux, las cuentas se conservan ahora entre un inicio y otro cuando el llavero del sistema no tiene contraseña; una cuenta que haya desaparecido debe añadirse una vez más, y una versión anterior no puede leer las cuentas guardadas por esta.
```

**Italiano**

```
Su Linux gli account ora vengono conservati tra un avvio e l’altro quando il portachiavi di sistema non ha una password; un account scomparso va aggiunto ancora una volta, e una versione precedente non può leggere gli account salvati da questa.
```

**Português**

```
No Linux, as contas são agora mantidas entre arranques quando o porta-chaves do sistema não tem palavra-passe; uma conta que tenha desaparecido tem de ser adicionada mais uma vez, e uma versão anterior não consegue ler as contas guardadas por esta.
```
