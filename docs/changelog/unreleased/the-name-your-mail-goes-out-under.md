# The name your mail goes out under

Platforms: all
Bump: minor

> Mail from an IMAP account left as a bare address, because nothing ever asked what to call the
> sender. The name is now the core's: one value per account, stored with the other per-account
> preferences and put in the `From` of every send. It is asked for once the account connects
> rather than on the first screen, which stays the address field and nothing else, and it is
> filled in already where the provider knows it. Three providers keep a name of their own and
> disagree about who owns it, so the engine reports which of the three cases an account is in: no
> server-side name at all, one the account holder can change, or one an organisation's directory
> hands down. Settings offers a field for the first two and states the third rather than showing
> an editor nobody can use. The value is sanitised on store, so a name pasted out of a document
> cannot smuggle a second header into a message. The composer's From then shows what the recipient
> will see, `Name <address>`, from one exported formatter rather than four hand-rolled ones: the
> case worth getting right is the empty one, where the label is the address alone and never a lone
> pair of angle brackets. The sidebar still shows the address, which is what an account is
> recognised by. No second store note: this is the same feature reaching the place a sender picks
> who a message comes from, and the one above already promises it.

**English**

```
Set the name your mail goes out under. You are asked once when an account is added, and can change
it any time in Settings, so your messages arrive as your name rather than just your address.
```

**Nederlands**

```
Stel in onder welke naam je e-mail wordt verzonden. De app vraagt het eenmalig bij het toevoegen
van een account en je past het altijd aan in Instellingen, zodat berichten met je naam aankomen in
plaats van alleen je adres.
```

**Deutsch**

```
Legen Sie fest, unter welchem Namen Ihre E-Mails versendet werden. Die App fragt einmal beim
Hinzufügen eines Kontos danach, und Sie ändern ihn jederzeit in den Einstellungen, damit Ihre
Nachrichten mit Ihrem Namen statt nur Ihrer Adresse ankommen.
```

**Français**

```
Choisissez le nom sous lequel vos messages partent. L'app vous le demande une fois lors de l'ajout
d'un compte et vous le modifiez quand vous voulez dans les Réglages, pour que vos messages arrivent
à votre nom plutôt qu'à votre seule adresse.
```

**Español**

```
Elige el nombre con el que sale tu correo. La app te lo pregunta una vez al añadir una cuenta y
puedes cambiarlo cuando quieras en Ajustes, para que tus mensajes lleguen con tu nombre y no solo
con tu dirección.
```

**Italiano**

```
Scegli il nome con cui parte la tua posta. L'app te lo chiede una volta quando aggiungi un account
e puoi cambiarlo quando vuoi nelle Impostazioni, così i messaggi arrivano con il tuo nome e non
solo con il tuo indirizzo.
```

**Português**

```
Defina o nome com que o seu correio é enviado. A aplicação pergunta uma vez ao adicionar uma conta
e pode alterá-lo quando quiser nas Definições, para que as mensagens cheguem com o seu nome e não
apenas com o seu endereço.
```
