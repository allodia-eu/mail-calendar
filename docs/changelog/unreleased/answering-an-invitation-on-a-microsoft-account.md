# Answering an invitation on a Microsoft account

Platforms: all
Bump: patch

> An invitation is answered on the calendar's copy of the meeting, which is found by the `UID` the
> invitation mail carries. Microsoft Graph reports two identities for an event, and the engine read
> the wrong one: `iCalUId` is Exchange's own re-encoding, which wraps an outside organiser's `UID`
> in a structure of its own, while `uid` keeps the organiser's verbatim. So a meeting organised
> anywhere but Exchange had one identity in the mail and another in the calendar, nothing joined
> the two, and answering fell through to "put the meeting on the calendar first", which Graph has
> no verb for. Fixed in the engine
> (allodia-eu/email-calendar-sync-engine#204). An invitation the calendar already held before this
> release keeps the old identity until the account's calendar is synced afresh.

**English**

```
Accepting, declining or answering "maybe" to an invitation now works on a Microsoft account, instead of reporting that the answer was not sent.
```

**Nederlands**

```
Een uitnodiging accepteren, weigeren of met "misschien" beantwoorden werkt nu op een Microsoft-account, in plaats van te melden dat het antwoord niet is verstuurd.
```

**Deutsch**

```
Eine Einladung annehmen, ablehnen oder mit „vielleicht“ beantworten funktioniert jetzt bei einem Microsoft-Konto, statt zu melden, dass die Antwort nicht gesendet wurde.
```

**Français**

```
Accepter, refuser une invitation ou y répondre « peut-être » fonctionne désormais sur un compte Microsoft, au lieu d'indiquer que la réponse n'a pas été envoyée.
```

**Español**

```
Aceptar, rechazar o responder «quizás» a una invitación ya funciona en una cuenta de Microsoft, en vez de indicar que la respuesta no se envió.
```

**Italiano**

```
Accettare, rifiutare o rispondere «forse» a un invito ora funziona su un account Microsoft, invece di segnalare che la risposta non è stata inviata.
```

**Português**

```
Aceitar, recusar ou responder «talvez» a um convite passa a funcionar numa conta Microsoft, em vez de indicar que a resposta não foi enviada.
```
