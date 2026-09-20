# Contacts from a Google account

Platforms: all
Bump: minor

> A Google account requested the People scopes at sign-in but bound no adapter, so its address
> book stayed empty while a CardDAV or JMAP account's filled. The core now binds one adapter per
> source the app reads: the user's own connections (writable, so create and edit land there), the
> Other Contacts Google saves from their mail, and a Workspace domain's directory. The two
> read-only sources do not exist on a personal account and People answers `403`; the engine reads
> that as an unavailable source, so nothing is special-cased on the address. Contact groups stay
> unbound while the people list filters group cards out.

**English**

```
Contacts from a Google account now appear in the app, including the addresses Google saved for you and, on a work account, your colleagues.
```

**Nederlands**

```
Contacten uit een Google-account staan nu in de app, inclusief de adressen die Google voor je bewaarde en, bij een werkaccount, je collega's.
```

**Deutsch**

```
Kontakte aus einem Google-Konto erscheinen jetzt in der App, einschließlich der von Google für Sie gespeicherten Adressen und, bei einem Arbeitskonto, Ihrer Kolleginnen und Kollegen.
```

**Français**

```
Les contacts d'un compte Google apparaissent désormais dans l'application, y compris les adresses que Google a enregistrées pour vous et, sur un compte professionnel, vos collègues.
```

**Español**

```
Los contactos de una cuenta de Google ya aparecen en la aplicación, incluidas las direcciones que Google guardó por ti y, en una cuenta de trabajo, tus compañeros.
```

**Italiano**

```
I contatti di un account Google ora compaiono nell'app, compresi gli indirizzi che Google ha salvato per te e, su un account di lavoro, i tuoi colleghi.
```

**Português**

```
Os contactos de uma conta Google aparecem agora na aplicação, incluindo os endereços que o Google guardou por si e, numa conta de trabalho, os seus colegas.
```
