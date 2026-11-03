# A meeting's numbers pass the link policy

Platforms: macos, ios
Bump: patch

> The core decides `join_url` and each dial-in `uri` through `is_openable_link`, the policy a
> clicked link in a message passes; a client opens them as they are. A dial-in `uri` holding a
> service code is `None`, and one carrying a pause and a `#` PIN falls back to the number alone.

**English**

```
A number in an event's video meeting that would run a phone service code is no longer offered to dial, and a dial-in number with its PIN attached dials the number alone.
```

**Nederlands**

```
Een nummer in de videovergadering van een afspraak dat een servicecode van de telefoon zou uitvoeren, wordt niet meer aangeboden om te bellen, en een inbelnummer met de pincode erachter belt alleen het nummer.
```

**Deutsch**

```
Eine Nummer in der Videobesprechung eines Termins, die einen Dienstcode des Telefons ausführen würde, wird nicht mehr zum Wählen angeboten, und eine Einwahlnummer mit angehängter PIN wählt nur die Nummer.
```

**Français**

```
Un numéro de la réunion vidéo d’un événement qui exécuterait un code de service du téléphone n’est plus proposé à l’appel, et un numéro d’appel suivi de son code PIN compose le numéro seul.
```

**Español**

```
Un número de la reunión de vídeo de un evento que ejecutaría un código de servicio del teléfono ya no se ofrece para llamar, y un número de teléfono con su PIN añadido marca solo el número.
```

**Italiano**

```
Un numero della riunione video di un evento che eseguirebbe un codice di servizio del telefono non viene più proposto per la chiamata, e un numero per partecipare con il PIN in coda compone solo il numero.
```

**Português**

```
Um número da reunião de vídeo de um evento que executaria um código de serviço do telefone deixa de ser oferecido para marcar, e um número de telefone com o PIN anexado marca apenas o número.
```
