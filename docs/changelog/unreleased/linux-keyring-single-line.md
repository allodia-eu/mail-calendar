# Linux keeps accounts in a passwordless keyring

Platforms: linux
Bump: patch

> gnome-keyring's plain-text keyring format keeps only the first line of a stored value, so every
> account config came back as its `capabilities` line and was left out at the next launch. Each
> value is now stored as one base64 line; a value stored before that still reads back. An account
> already cut short cannot be recovered and has to be added once more. A stored account left out
> at boot is now named in the diagnostic log.

**English**

```
On Linux, accounts are now kept between launches when the system keyring has no password. An account that disappeared needs to be added once more.
```

**Nederlands**

```
Op Linux blijven accounts nu bewaard tussen het opstarten wanneer de systeemsleutelbos geen wachtwoord heeft. Een verdwenen account moet je nog één keer toevoegen.
```

**Deutsch**

```
Unter Linux bleiben Konten jetzt zwischen zwei Starts erhalten, wenn der System-Schlüsselbund kein Passwort hat. Ein verschwundenes Konto müssen Sie noch einmal hinzufügen.
```

**Français**

```
Sous Linux, les comptes sont désormais conservés d’un lancement à l’autre lorsque le trousseau du système n’a pas de mot de passe. Un compte disparu doit être ajouté une nouvelle fois.
```

**Español**

```
En Linux, las cuentas se conservan ahora entre un inicio y otro cuando el llavero del sistema no tiene contraseña. Una cuenta que haya desaparecido debe añadirse una vez más.
```

**Italiano**

```
Su Linux gli account ora vengono conservati tra un avvio e l’altro quando il portachiavi di sistema non ha una password. Un account scomparso va aggiunto ancora una volta.
```

**Português**

```
No Linux, as contas são agora mantidas entre arranques quando o porta-chaves do sistema não tem palavra-passe. Uma conta que tenha desaparecido tem de ser adicionada mais uma vez.
```
