# A reply to a plain-text message carries clean line endings

Platforms: all
Bump: patch

> A plain-text original keeps the CRLF its MIME part was written in. The editor now reads it on any
> line ending (`plain_text.ts`) and holds it in `\n` alone (`quote.ts`), so neither half of a reply
> carries a bare carriage return.

**English**

```
Replies to plain-text messages no longer end the quoted text with a stray line-ending character.
```

**Nederlands**

```
Antwoorden op berichten in platte tekst eindigen de geciteerde tekst niet meer met een los regeleindeteken.
```

**Deutsch**

```
Antworten auf Nachrichten im Nur-Text-Format beenden den zitierten Text nicht mehr mit einem überzähligen Zeilenendezeichen.
```

**Français**

```
Les réponses aux messages en texte brut ne terminent plus le texte cité par un caractère de fin de ligne superflu.
```

**Español**

```
Las respuestas a mensajes de texto sin formato ya no terminan el texto citado con un carácter de fin de línea sobrante.
```

**Italiano**

```
Le risposte ai messaggi in testo semplice non terminano più il testo citato con un carattere di fine riga superfluo.
```

**Português**

```
As respostas a mensagens em texto simples já não terminam o texto citado com um carácter de fim de linha a mais.
```
