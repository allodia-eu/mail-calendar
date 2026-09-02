# Share a file into a new message

Platforms: android, linux
Bump: minor

> The shared half is `mailcal_composer::share`, so Apple and Windows will inherit the naming, the
> typing, the cap and the refusal reporting rather than each deciding again. Linux went first
> because it is the one client a Linux workstation can actually run, which is where the design got
> its first real exercise; Android followed, and its JVM suite gates the intent half.
>
> Linux has no share portal, so "Open With" plus a local `--attach` is what a desktop actually
> offers. That has a cost worth stating: a `MimeType=` entry reads as "this app opens that type",
> and there is no key for "attaches but does not display", so the list is kept to what a person
> plausibly emails and a test pins it exactly.
>
> `mailto:?attach=` stays unhonoured, and now says so as a standing rule: a handler cannot tell a
> URI that came from `xdg-email` from one that came from a web page.

**English**

```
Share a file to Allodia Mail & Calendar from any other app, and a new message opens with it
already attached.
```

**Nederlands**

```
Deel een bestand vanuit een andere app met Allodia Mail & Calendar, en er opent een nieuw bericht
met het bestand er al aan.
```

**Deutsch**

```
Teilen Sie eine Datei aus einer anderen App mit Allodia Mail & Calendar, und eine neue Nachricht
öffnet sich mit der Datei bereits im Anhang.
```

**Français**

```
Partagez un fichier depuis une autre application vers Allodia Mail & Calendar, et un nouveau
message s'ouvre avec le fichier déjà joint.
```

**Español**

```
Comparte un archivo desde otra aplicación con Allodia Mail & Calendar y se abre un mensaje nuevo
con el archivo ya adjunto.
```

**Italiano**

```
Condividi un file da un'altra app con Allodia Mail & Calendar e si apre un nuovo messaggio con il
file già allegato.
```

**Português**

```
Partilhe um ficheiro de outra aplicação com o Allodia Mail & Calendar e abre-se uma mensagem nova
com o ficheiro já anexado.
```
