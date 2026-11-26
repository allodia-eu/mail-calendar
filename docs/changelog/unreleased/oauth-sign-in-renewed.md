# An account signed in through its provider keeps its calendar and contacts

Platforms: all
Bump: patch

> The calendar and address book of an account signed in through its provider's own OAuth
> sign-in, other than Google and Microsoft (CalDAV and CardDAV over the grant's token), were
> built over the token minted at connect, so they stopped an hour later, when it expired. Every
> adapter of an account now asks the account's one credential source for the token on each
> request, and a token a server refuses before its stated expiry is renewed once, through the
> same single-flight refresh, before the request is sent again; only a grant that is gone still
> asks for a sign-in.

**English**

```
The calendar and contacts of an account you signed in to through your provider, other than a Google or Microsoft account, keep working past the first hour, and for every account signed in this way a sign-in the server stops accepting early is renewed without asking you to sign in again.
```

**Nederlands**

```
De agenda en contacten van een account waarbij u zich via uw provider aanmeldde, anders dan een Google- of Microsoft-account, blijven ook na het eerste uur werken, en bij elk account waarbij u zich zo aanmeldde wordt een aanmelding die de server te vroeg weigert vernieuwd, zonder dat u zich opnieuw hoeft aan te melden.
```

**Deutsch**

```
Kalender und Kontakte eines Kontos, bei dem Sie sich über Ihren Anbieter angemeldet haben, außer bei Google- und Microsoft-Konten, funktionieren auch nach der ersten Stunde weiter, und bei jedem so angemeldeten Konto wird eine Anmeldung, die der Server vorzeitig ablehnt, erneuert, ohne dass Sie sich neu anmelden müssen.
```

**Français**

```
Le calendrier et les contacts d’un compte auquel vous vous êtes connecté via votre fournisseur, hors comptes Google et Microsoft, continuent de fonctionner au-delà de la première heure, et pour tout compte connecté ainsi, une connexion que le serveur refuse avant terme est renouvelée sans vous demander de vous reconnecter.
```

**Español**

```
El calendario y los contactos de una cuenta en la que inició sesión a través de su proveedor, salvo las cuentas de Google y Microsoft, siguen funcionando después de la primera hora, y en toda cuenta en la que inició sesión así, un inicio de sesión que el servidor deja de aceptar antes de tiempo se renueva sin pedirle que vuelva a iniciar sesión.
```

**Italiano**

```
Il calendario e i contatti di un account a cui avete effettuato l’accesso tramite il vostro provider, esclusi gli account Google e Microsoft, continuano a funzionare dopo la prima ora, e per ogni account a cui avete effettuato l’accesso in questo modo un accesso che il server rifiuta in anticipo viene rinnovato senza chiedervi di accedere di nuovo.
```

**Português**

```
O calendário e os contactos de uma conta em que iniciou sessão através do seu fornecedor, exceto contas Google e Microsoft, continuam a funcionar depois da primeira hora, e em qualquer conta em que iniciou sessão assim, uma sessão que o servidor deixa de aceitar antes do tempo é renovada sem lhe pedir que inicie sessão de novo.
```
