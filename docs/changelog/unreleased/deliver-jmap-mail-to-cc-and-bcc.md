# Deliver mail sent over JMAP to its Cc and Bcc recipients

Platforms: all
Bump: patch

> The engine built a JMAP send from its To recipients alone: Cc and Bcc were never written into the
> message or its delivery list, and a reply carried no In-Reply-To or References. It now carries all
> of them, with Bcc kept on the sender's copy only. Fixed in the engine
> (allodia-eu/email-calendar-sync-engine#239); this change moves the pin.

**English**

```
On accounts that connect over JMAP, people in Cc and Bcc now receive your message, and your replies stay in the recipients' conversation.
```

**Nederlands**

```
Bij accounts die via JMAP verbinden, ontvangen mensen in Cc en Bcc uw bericht nu ook, en blijven uw antwoorden bij de ontvangers in hetzelfde gesprek.
```

**Deutsch**

```
Bei Konten, die sich über JMAP verbinden, erhalten Personen in Cc und Bcc Ihre Nachricht jetzt ebenfalls, und Ihre Antworten bleiben bei den Empfängern in derselben Unterhaltung.
```

**Français**

```
Sur les comptes connectés via JMAP, les personnes en Cc et Cci reçoivent désormais votre message, et vos réponses restent dans la conversation des destinataires.
```

**Español**

```
En las cuentas que se conectan por JMAP, las personas en Cc y Cco reciben ahora tu mensaje, y tus respuestas se mantienen en la conversación de los destinatarios.
```

**Italiano**

```
Negli account che si collegano tramite JMAP, le persone in Cc e Ccn ricevono ora il tuo messaggio, e le tue risposte restano nella conversazione dei destinatari.
```

**Português**

```
Nas contas que se ligam por JMAP, as pessoas em Cc e Bcc recebem agora a sua mensagem, e as suas respostas ficam na conversa dos destinatários.
```
