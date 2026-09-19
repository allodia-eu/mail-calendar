# A quoted plain-text message keeps its line breaks

Platforms: all
Bump: patch

> Replying to a message with no `text/html` part put the original in the composer as text whose
> line breaks were laid out by one rule in the editor's own stylesheet. That stylesheet stops at
> the editor: `quoteBlock` serialises the body element's markup, which had nothing between the
> lines, so the sent HTML collapsed the whole quoted thread, blank lines, wrapped lines and every
> `>` level alike, into a single paragraph. Plain-text senders are the ones this hits, and their
> newlines are the only structure the body has. The editor now states `white-space: pre-wrap`
> inline on the wrapper it builds, so the breaks travel in the message rather than in the page
> that composed it; the text is still assigned with `textContent`, so nothing in the original can
> become markup. Two suites hold the halves together, one over the editor's round trip and one
> over a real reply submit, since the submit-time re-sanitiser that must keep the declaration is
> in the other language.

**English**

```
Replying to a message written in plain text no longer squashes the quoted conversation into one paragraph; its line breaks and quote levels are kept.
```

**Nederlands**

```
Antwoorden op een bericht in platte tekst plet het geciteerde gesprek niet langer tot één alinea; de regeleindes en citaatniveaus blijven staan.
```

**Deutsch**

```
Die Antwort auf eine Nachricht in reinem Text quetscht das zitierte Gespräch nicht mehr in einen Absatz; Zeilenumbrüche und Zitatebenen bleiben erhalten.
```

**Français**

```
Répondre à un message en texte brut n'écrase plus la conversation citée en un seul paragraphe ; ses sauts de ligne et ses niveaux de citation sont conservés.
```

**Español**

```
Responder a un mensaje escrito en texto sin formato ya no aplasta la conversación citada en un solo párrafo; se conservan sus saltos de línea y niveles de cita.
```

**Italiano**

```
Rispondere a un messaggio in testo semplice non schiaccia più la conversazione citata in un unico paragrafo; restano le interruzioni di riga e i livelli di citazione.
```

**Português**

```
Responder a uma mensagem em texto simples deixa de esmagar a conversa citada num único parágrafo; as quebras de linha e os níveis de citação mantêm-se.
```
