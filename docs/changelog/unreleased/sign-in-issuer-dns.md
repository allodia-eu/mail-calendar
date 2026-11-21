# Signing in from an account's settings at the server setup found

Platforms: all
Bump: patch

> Setup finds a provider hosting the address's domain through its MX record, which needs the host's
> DNS. The sign-in an IMAP account's settings offer ran the same detection without it, so for such
> a domain it named another authorization server, or none (seen with Soverin: `my.soverin.net` at
> setup, `soverin.net` from Settings). It now looks through the DNS every client hands the core at
> launch, and an answer given before that is not kept.

**English**

```
For an email address on a domain of your own, signing in through your provider from an account’s settings now uses the same sign-in server as adding the account.
```

**Nederlands**

```
Voor een e-mailadres op een eigen domein gebruikt inloggen via je provider vanuit de instellingen van een account nu dezelfde inlogserver als bij het toevoegen van het account.
```

**Deutsch**

```
Bei einer E-Mail-Adresse mit eigener Domain verwendet die Anmeldung über Ihren Anbieter in den Kontoeinstellungen jetzt denselben Anmeldeserver wie beim Hinzufügen des Kontos.
```

**Français**

```
Pour une adresse e-mail sur votre propre domaine, la connexion via votre fournisseur depuis les réglages d’un compte utilise désormais le même serveur de connexion que lors de l’ajout du compte.
```

**Español**

```
Para una dirección de correo en un dominio propio, iniciar sesión a través de tu proveedor desde los ajustes de una cuenta ahora usa el mismo servidor de inicio de sesión que al añadir la cuenta.
```

**Italiano**

```
Per un indirizzo email su un dominio tuo, l’accesso tramite il tuo provider dalle impostazioni di un account ora usa lo stesso server di accesso usato per aggiungere l’account.
```

**Português**

```
Para um endereço de email num domínio próprio, iniciar sessão através do seu fornecedor nas definições de uma conta passa a usar o mesmo servidor de início de sessão que ao adicionar a conta.
```
