# A server at a plaintext address is no longer sent a sign-in

Platforms: all
Bump: patch

> The engine sends no credential over plaintext to a server off this device, so a calendar,
> address book or JMAP server at an `http://` address, or one that leads there, now fails its
> connect as `AccountError::InsecureServer`, with the engine's cause (`PlaintextAddress` or
> `LedToPlaintext`). The app says so on every surface (setup, the account's page, the
> `InsecureServer` notice, the use problems) rather than as a refused sign-in or an outage, and
> says what clears it (`docs/provider-oauth.md` rule 12).

**English**

```
A calendar, address book or JMAP mail server at an http:// address that is not on this device, or one that sends the app on to such an address, is no longer sent your sign-in, and the app says what clears it.
```

**Nederlands**

```
Een agenda-, adresboek- of JMAP-mailserver op een http://-adres buiten dit apparaat, of een die de app naar zo’n adres doorstuurt, krijgt uw aanmelding niet meer, en de app meldt wat dit verhelpt.
```

**Deutsch**

```
Ein Kalender-, Adressbuch- oder JMAP-Mailserver unter einer http://-Adresse außerhalb dieses Geräts, oder einer, der die App an eine solche Adresse weiterleitet, erhält Ihre Anmeldung nicht mehr, und die App sagt, was das behebt.
```

**Français**

```
Un serveur de calendrier, de carnet d’adresses ou de messagerie JMAP à une adresse http:// hors de cet appareil, ou qui renvoie l’application vers une telle adresse, ne reçoit plus vos identifiants, et l’application indique ce qui y remédie.
```

**Español**

```
Un servidor de calendario, de contactos o de correo JMAP en una dirección http:// fuera de este dispositivo, o que redirige la aplicación a una dirección así, ya no recibe su inicio de sesión, y la aplicación indica qué lo resuelve.
```

**Italiano**

```
Un server del calendario, della rubrica o di posta JMAP a un indirizzo http:// esterno a questo dispositivo, o che rimanda l’app a un indirizzo simile, non riceve più i vostri dati di accesso, e l’app indica cosa risolve il problema.
```

**Português**

```
Um servidor de calendário, de contactos ou de correio JMAP num endereço http:// fora deste dispositivo, ou que reencaminha a aplicação para um endereço assim, já não recebe o seu início de sessão, e a aplicação indica o que o resolve.
```
