# A dial-in number with its PIN reads on a CalDAV calendar

Platforms: macos, ios
Bump: patch

> Stalwart 0.16 serves a `CONFERENCE` URI with its commas TEXT-escaped
> (`tel:+31205550101\,\,123456#`), and the engine refused a URI holding a backslash, so a dial-in
> that pauses before its PIN was dropped from the meeting on a CalDAV account. The engine now
> unescapes the value first (allodia-eu/email-calendar-sync-engine#337). Android, Linux and Windows
> gain meetings in this release, so their own notes already cover it.

**English**

```
A meeting's dial-in number with its PIN attached is no longer left out on some CalDAV calendars.
```

**Nederlands**

```
Een inbelnummer van een vergadering met de pincode erachter wordt in sommige CalDAV-agenda's niet meer weggelaten.
```

**Deutsch**

```
Eine Einwahlnummer einer Besprechung mit angehängter PIN fehlt in manchen CalDAV-Kalendern nicht mehr.
```

**Français**

```
Le numéro d’appel d’une réunion suivi de son code PIN n’est plus omis dans certains calendriers CalDAV.
```

**Español**

```
El número de teléfono de una reunión con su PIN añadido ya no se omite en algunos calendarios CalDAV.
```

**Italiano**

```
Il numero per partecipare a una riunione con il PIN in coda non viene più omesso in alcuni calendari CalDAV.
```

**Português**

```
O número de telefone de uma reunião com o PIN anexado deixa de ser omitido em alguns calendários CalDAV.
```
