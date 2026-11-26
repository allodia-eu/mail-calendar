# Answering an invitation to a meeting far ahead works on a CalDAV calendar

Platforms: all
Bump: patch

> A meeting the core stores before answering is put at the address the calendar adapter names
> for its `UID` (`new_event_id`), and read back from there. The core's own spelling encoded an `@`
> that servers list bare, so the read-back missed and fell back to the day scan, which sees only
> the expanded window: beyond it the server route failed and the iMIP route sent the reply with
> no answer stored. It also kept a `UID` too long to be a resource name, which some servers refuse.

**English**

```
Answering an invitation to a meeting further ahead than the calendar had loaded no longer fails or sends the reply without recording your answer, and a meeting with a very long ID can now be added to the calendar.
```

**Nederlands**

```
Een uitnodiging beantwoorden voor een vergadering verder vooruit dan de agenda had geladen, mislukt niet meer en verstuurt het antwoord niet langer zonder het in de agenda vast te leggen, en een vergadering met een zeer lange ID kan nu aan de agenda worden toegevoegd.
```

**Deutsch**

```
Eine Einladung zu einer Besprechung, die weiter in der Zukunft liegt, als der Kalender geladen hatte, lässt sich jetzt beantworten, ohne dass dies fehlschlägt oder die Antwort ohne Eintrag im Kalender versendet wird, und eine Besprechung mit sehr langer ID kann jetzt dem Kalender hinzugefügt werden.
```

**Français**

```
Répondre à une invitation pour une réunion plus lointaine que la période chargée par le calendrier n’échoue plus et n’envoie plus la réponse sans l’enregistrer, et une réunion dotée d’un identifiant très long peut désormais être ajoutée au calendrier.
```

**Español**

```
Responder a una invitación para una reunión más lejana que el periodo cargado por el calendario ya no falla ni envía la respuesta sin registrarla, y una reunión con un identificador muy largo ya puede añadirse al calendario.
```

**Italiano**

```
Rispondere a un invito per una riunione più lontana del periodo caricato dal calendario non fallisce più né invia la risposta senza registrarla, e una riunione con un identificativo molto lungo può ora essere aggiunta al calendario.
```

**Português**

```
Responder a um convite para uma reunião mais distante do que o período carregado pelo calendário já não falha nem envia a resposta sem a registar, e uma reunião com um identificador muito longo pode agora ser adicionada ao calendário.
```
