# Calendar and contacts servers behind a TLS-terminating proxy

Platforms: all
Bump: patch

> A server that redirects from HTTPS to `http://` on its own host, which Nextcloud's
> `.well-known` rewrite does behind a TLS-terminating proxy, is followed over HTTPS, both when
> setup looks for it and when the engine connects; a plaintext redirect to another host is still
> refused. A calendar or address book URL with a path now starts discovery at that path rather
> than at the host's `.well-known`, and goes back to the `.well-known` when the path is gone.
> Contacts that use the calendar's server still start at its CardDAV `.well-known`.

**English**

```
Calendar and contacts servers such as Nextcloud now connect, still encrypted, when their address redirects to an unencrypted one, and a server address with a path is used as you entered it.
```

**Nederlands**

```
Agenda- en contactservers zoals Nextcloud maken nu verbinding, nog steeds versleuteld, als hun adres doorverwijst naar een onversleuteld adres, en een serveradres met een pad wordt gebruikt zoals je het hebt ingevoerd.
```

**Deutsch**

```
Kalender- und Kontaktserver wie Nextcloud verbinden sich jetzt, weiterhin verschlüsselt, auch wenn ihre Adresse auf eine unverschlüsselte weiterleitet, und eine Serveradresse mit Pfad wird so verwendet, wie Sie sie eingegeben haben.
```

**Français**

```
Les serveurs d’agenda et de contacts comme Nextcloud se connectent désormais, toujours de façon chiffrée, même lorsque leur adresse redirige vers une adresse non chiffrée, et une adresse de serveur comportant un chemin est utilisée telle que vous l’avez saisie.
```

**Español**

```
Los servidores de calendario y contactos como Nextcloud ahora se conectan, siempre cifrados, aunque su dirección redirija a una sin cifrar, y una dirección de servidor con una ruta se usa tal como la escribiste.
```

**Italiano**

```
I server di calendario e contatti come Nextcloud ora si collegano, sempre in modo cifrato, anche quando il loro indirizzo reindirizza a uno non cifrato, e un indirizzo del server con un percorso viene usato così come lo hai inserito.
```

**Português**

```
Os servidores de calendário e contactos como o Nextcloud passam a ligar-se, sempre de forma encriptada, mesmo quando o endereço redireciona para um endereço não encriptado, e um endereço de servidor com um caminho é usado tal como o introduziu.
```
