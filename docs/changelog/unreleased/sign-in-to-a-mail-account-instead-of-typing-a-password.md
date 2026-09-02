# Sign in to a mail account instead of typing a password

Platforms: linux, macos, ios, android
Bump: minor

> The setup screen asks the mail server what it accepts before it draws a field, so which
> credential a person is asked for is the server's answer rather than a guess. Three answers,
> because they are three screens: sign in with the provider, a line saying the provider admits
> only applications it registered in advance, or the password form as it always was. The middle
> one is the point: showing one bare password form for a provider whose sign-in is simply
> closed to us leaves someone wondering why the button their colleague has is missing.
> An issuer is only ever taken from the provider describing itself over HTTPS, never from a
> third-party database and never from an untrusted hop, and the endpoints come from that
> issuer's own metadata. An account that signs in stores no password anywhere, including for
> its calendar. `Platforms:` is Linux alone: the core decides for every client, and the other
> three carry the answer no further than the binding until each ships the surface
> (`docs/mail-oauth.md` → Known gaps).

**English**

```
Connect a mail account by signing in with your provider, where your provider supports it, instead
of storing a password. Where it doesn't, the password field is still there, and the setup screen
now says which of the two your provider actually offers.
```

**Nederlands**

```
Verbind een mailaccount door je aan te melden bij je provider, als die dat ondersteunt, in plaats
van een wachtwoord te bewaren. Zo niet, dan blijft het wachtwoordveld gewoon staan, en het
instelscherm vertelt nu welke van de twee je provider echt aanbiedt.
```

**Deutsch**

```
Verbinden Sie ein Mailkonto, indem Sie sich bei Ihrem Anbieter anmelden, sofern er das
unterstützt, statt ein Passwort zu speichern. Andernfalls bleibt das Passwortfeld erhalten, und
das Einrichtungsfenster sagt jetzt, welche der beiden Möglichkeiten Ihr Anbieter tatsächlich
bietet.
```

**Français**

```
Connectez un compte de messagerie en vous connectant chez votre fournisseur, lorsqu’il le prend en
charge, plutôt qu’en enregistrant un mot de passe. Sinon, le champ mot de passe reste là, et
l’écran de configuration indique désormais laquelle des deux votre fournisseur propose vraiment.
```

**Español**

```
Conecta una cuenta de correo iniciando sesión con tu proveedor, cuando lo permita, en lugar de
guardar una contraseña. Si no, el campo de contraseña sigue ahí, y la pantalla de configuración
ahora dice cuál de las dos ofrece realmente tu proveedor.
```

**Italiano**

```
Collega un account di posta accedendo con il tuo provider, quando lo supporta, invece di salvare
una password. Altrimenti il campo password resta dov’è, e la schermata di configurazione ora dice
quale delle due il tuo provider offre davvero.
```

**Português**

```
Ligue uma conta de correio iniciando sessão com o seu fornecedor, quando este o permitir, em vez de
guardar uma palavra-passe. Caso contrário, o campo da palavra-passe continua lá, e o ecrã de
configuração diz agora qual das duas o seu fornecedor oferece de facto.
```
