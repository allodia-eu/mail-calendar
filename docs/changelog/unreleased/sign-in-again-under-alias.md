# Signing in again to an account set up under another spelling of its address

Platforms: all
Bump: patch

> A server following draft-ietf-mailmaint-oauth-public §3.5 names the account a grant is for in
> its token response (`username`), which Stalwart now does. Setup keeps the typed address, and a
> later sign-in was compared with that address, so an account set up under an alias of the login
> name was refused as "a different account" every time. The grant now records the name the first
> sign-in got (`signed_in_as`) and a later one is held to it. A grant stored before this records no
> name and stays held to the address: accepting the first name offered would let one sign-in as
> another account through unchecked, so such an account is removed and added again once.
> Separately, a token now goes only to a server whose URL an entry of the issuer's
> `protected_resources` matches by scheme, host, port and path, where it went to any server on a
> listed host.

**English**

```
Signing in again to an email account set up from this version on under an alias of its address no longer says you signed in to a different account.
```

**Nederlands**

```
Opnieuw inloggen bij een e-mailaccount dat vanaf deze versie is ingesteld met een alias van het adres, meldt niet langer dat je bij een ander account bent ingelogd.
```

**Deutsch**

```
Wenn Sie sich erneut bei einem E-Mail-Konto anmelden, das ab dieser Version mit einem Alias seiner Adresse eingerichtet wurde, heißt es nicht mehr, Sie hätten sich bei einem anderen Konto angemeldet.
```

**Français**

```
Se reconnecter à un compte e-mail configuré à partir de cette version avec un alias de son adresse n’indique plus que vous vous êtes connecté à un autre compte.
```

**Español**

```
Volver a iniciar sesión en una cuenta de correo configurada a partir de esta versión con un alias de su dirección ya no indica que has iniciado sesión en otra cuenta.
```

**Italiano**

```
Accedere di nuovo a un account email configurato da questa versione in poi con un alias del suo indirizzo non segnala più che hai effettuato l’accesso a un altro account.
```

**Português**

```
Iniciar sessão novamente numa conta de email configurada a partir desta versão com um alias do endereço já não indica que iniciou sessão noutra conta.
```
