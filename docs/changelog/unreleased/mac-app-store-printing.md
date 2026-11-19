# Printing works in the Mac App Store build

Platforms: macos
Bump: patch

> The Store build is the only sandboxed one and did not grant `com.apple.security.print`, so AppKit
> refused the print sheet with "This application does not support printing". The grant is added,
> and `cargo xtask check-store-sandbox` now pairs the print call with it.

**English**

```
Printing a message works again in the version from the Mac App Store, which answered that the application does not support printing.
```

**Nederlands**

```
Een bericht afdrukken werkt weer in de versie uit de Mac App Store, die meldde dat de app afdrukken niet ondersteunt.
```

**Deutsch**

```
Das Drucken einer Nachricht funktioniert wieder in der Version aus dem Mac App Store, die meldete, dass die App das Drucken nicht unterstützt.
```

**Français**

```
L’impression d’un message fonctionne de nouveau dans la version du Mac App Store, qui répondait que l’application ne prend pas en charge l’impression.
```

**Español**

```
Imprimir un mensaje vuelve a funcionar en la versión del Mac App Store, que respondía que la aplicación no admite la impresión.
```

**Italiano**

```
La stampa di un messaggio funziona di nuovo nella versione del Mac App Store, che rispondeva che l’applicazione non supporta la stampa.
```

**Português**

```
Imprimir uma mensagem volta a funcionar na versão da Mac App Store, que respondia que a aplicação não suporta a impressão.
```
