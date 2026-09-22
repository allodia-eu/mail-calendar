# Set a server up by hand, with its port and its security

Platforms: macos, ios, windows, android, linux
Bump: minor

> The manual form sent every server implicit TLS, whatever port it was given, so a STARTTLS-only
> server could not be set up by hand at all: the client wrapped TLS around a plaintext listener and
> reported a corrupt message. It now offers the connection security beside the server name, with
> the port as a field of its own rather than a colon the user is expected to know to type. The port
> follows the security picked until the user types one, and is theirs from then on. See rule 11 in
> `docs/account-autodetect.md`.

**English**

```
Setting a server up by hand now offers its port and its connection security beside the server name, so a server that uses STARTTLS, or any port other than the usual one, can be set up.
```

**Nederlands**

```
Een server handmatig instellen biedt nu de poort en de beveiliging naast de servernaam, zodat je ook een server met STARTTLS of een afwijkende poort kunt instellen.
```

**Deutsch**

```
Beim manuellen Einrichten stehen Port und Verbindungssicherheit jetzt neben dem Servernamen, sodass sich auch ein Server mit STARTTLS oder einem anderen Port einrichten lässt.
```

**Français**

```
La configuration manuelle propose désormais le port et la sécurité de la connexion à côté du nom du serveur, ce qui permet de configurer un serveur en STARTTLS ou sur un port inhabituel.
```

**Español**

```
La configuración manual ahora ofrece el puerto y la seguridad de la conexión junto al nombre del servidor, así puedes configurar un servidor con STARTTLS o en un puerto distinto del habitual.
```

**Italiano**

```
La configurazione manuale ora propone la porta e la sicurezza della connessione accanto al nome del server, così puoi configurare anche un server con STARTTLS o su una porta diversa da quella abituale.
```

**Português**

```
A configuração manual passa a oferecer a porta e a segurança da ligação junto ao nome do servidor, pelo que já pode configurar um servidor com STARTTLS ou numa porta diferente da habitual.
```
