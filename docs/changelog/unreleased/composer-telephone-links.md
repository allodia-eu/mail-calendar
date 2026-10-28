# Telephone links in the composer

Platforms: all
Bump: minor

> A link may now carry `tel:` as well as `http`, `https` and `mailto`, so a signature or a message can
> make a number dial when it is tapped. The scheme joins `mailcal_composer::LINK_SCHEMES`, which is
> the one allowlist behind the link dialog, the reading view's sanitiser, the linkifier and
> `should_open_external_link`, so all four moved together. A bare number is still refused rather than
> completed into `tel:` in the address field, and a `tel:` written as text is still not linked: a
> wrong guess dials a stranger. Nothing changes about which other schemes are accepted.

**English**

```
Link a phone number so that tapping it starts a call, in a message or in your signature.
```

**Nederlands**

```
Koppel een telefoonnummer als link, zodat er wordt gebeld wanneer erop wordt getikt, in een bericht of in je handtekening.
```

**Deutsch**

```
Verknüpfen Sie eine Telefonnummer als Link, sodass ein Tippen darauf einen Anruf startet, in einer Nachricht oder in Ihrer Signatur.
```

**Français**

```
Liez un numéro de téléphone pour qu’un appui lance l’appel, dans un message ou dans votre signature.
```

**Español**

```
Enlaza un número de teléfono para que al tocarlo se inicie una llamada, en un mensaje o en tu firma.
```

**Italiano**

```
Collega un numero di telefono come link, così un tocco avvia la chiamata, in un messaggio o nella tua firma.
```

**Português**

```
Associe um número de telefone como ligação, para que tocar nele inicie a chamada, numa mensagem ou na sua assinatura.
```
