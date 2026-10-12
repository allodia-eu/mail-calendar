# Recognise a Microsoft 365 domain by its Autodiscover record

Platforms: all
Bump: minor

> Detection asks a domain's Autodiscover service, right after the JMAP probe, whether Microsoft
> 365 hosts its mail, and routes the address to the Microsoft sign-in when it does. A domain
> whose mail moved to Microsoft 365 while its website stayed on the old host was routed to that
> host's generated IMAP settings instead. Only Microsoft's own service counts: a self-hosted
> Exchange is not offered anything yet.

**English**

```
An address at a domain whose mail is hosted by Microsoft 365 is now recognised as such, even when an older mail host still publishes settings for it.
```

**Nederlands**

```
Een adres bij een domein waarvan de e-mail door Microsoft 365 wordt gehost, wordt nu als zodanig herkend, ook als een oudere e-mailhost er nog instellingen voor publiceert.
```

**Deutsch**

```
Eine Adresse bei einer Domain, deren E-Mail Microsoft 365 hostet, wird jetzt als solche erkannt, auch wenn ein früherer Mailanbieter noch Einstellungen dafür veröffentlicht.
```

**Français**

```
Une adresse d’un domaine dont la messagerie est hébergée par Microsoft 365 est désormais reconnue comme telle, même si un ancien hébergeur publie encore des réglages pour ce domaine.
```

**Español**

```
Una dirección de un dominio cuyo correo aloja Microsoft 365 se reconoce ahora como tal, aunque un proveedor de correo anterior aún publique ajustes para él.
```

**Italiano**

```
Un indirizzo di un dominio la cui posta è ospitata da Microsoft 365 viene ora riconosciuto come tale, anche se un vecchio provider di posta pubblica ancora impostazioni per quel dominio.
```

**Português**

```
Um endereço de um domínio cujo correio é alojado pelo Microsoft 365 é agora reconhecido como tal, mesmo que um fornecedor de correio anterior ainda publique definições para ele.
```
