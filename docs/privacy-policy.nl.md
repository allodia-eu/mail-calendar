# Privacybeleid: Allodia Mail & Calendar

**Versie 3.0 · Van kracht: 2026-11-23**

Allodia Mail & Calendar is een e-mail- en agenda-app die op je eigen apparaat draait en verbinding
maakt met de providers die **jij** kiest. Dit beleid zegt wat op je apparaat blijft, wat de app
verstuurt en naar wie, en wat ons bereikt. Standaard is dat niets: zonder Allodia-account, zoals de
app zich installeert, hebben wij helemaal niets van je.

## De korte versie

- **Je e-mail blijft tussen je apparaat en je provider.** Wij zitten niet in dat pad en kunnen je
  e-mail, je afspraken en je inloggegevens niet lezen. De enige uitzondering is een verzoek dat je
  zelf start via de dienst van onze schrijfassistent: dat gaat via onze servers naar een
  AI-aanbieder in de EU en wordt niet bewaard (§5.2).
- **Standaard stuurt de app ons niets**: geen telemetrie, geen crashrapporten, geen identifiers,
  zelfs niet het feit dát je de app hebt geïnstalleerd.
- **Drie dingen kunnen ons bereiken, elk alleen als jij daarvoor kiest:** een Allodia-account (§4),
  verzoeken aan de dienst van onze schrijfassistent (§5.2) en gebruiksstatistieken (§6). Een
  abonnement dat je koopt, hoort bij het account (§7).
- **Een Allodia-account bevat nooit je e-mail**, een postvak of een wachtwoord: alleen wie je bent
  en de lijst met e-mailaccounts die het gelijk houdt op je apparaten.
- **AI doet alleen iets als jij erom vraagt.** Een assistent op je computer komt alleen bij de
  accounts die je aanvinkt, en de schrijfassistent verstuurt alleen als je op Leren of Antwoord
  opstellen drukt (§5).
- **We zien je pas nooit, verkopen geen gegevens, tonen geen advertenties, stellen geen profiel van
  je op en gebruiken geen analyse- of trackingdiensten van derden.** Wat we ontvangen, blijft in de
  EU, met één uitzondering die we benoemen (§10).
- **Je rechten zijn die van de AVG**, en de meeste ervan kun je in de app zelf uitoefenen (§12).

## 1. Wie we zijn

Allodia, ingeschreven bij de Kamer van Koophandel (KvK) onder nr. **56789823**, Kamerlingh
Onnesweg 2, 3316GL Dordrecht, Nederland. Voor alles wat in dit beleid staat: **info@allodia.eu**.

We zijn alleen verwerkingsverantwoordelijke voor wat ons bereikt: een Allodia-account (§4),
verzoeken aan de dienst van onze schrijfassistent (§5.2), gebruiksstatistieken (§6), een abonnement
(§7), berichten die je ons stuurt (§8) en de website (§9). Wat de app op je apparaat verwerkt,
ontvangen we nooit, dus daarvoor zijn we verwerkingsverantwoordelijke noch verwerker.

## 2. Je apparaat

**Wat daar staat.** Je e-mail, afspraken, contacten, bijlagen, accountinstellingen,
handtekeningen, schrijfstijlen en een diagnostisch logboek. Niets daarvan wordt naar Allodia
geüpload; er is geen kopie in de cloud van ons.

**Hoe het beschermd is.**

- Wachtwoorden, inlogtokens en sleutels staan in de sleutelopslag van het platform (Keychain,
  Windows Credential Manager, Android Keystore of de Linux-systeemsleutelring via Secret Service),
  nooit in de database of het logboek van de app. De berichtenopslag wordt beschermd door de
  versleuteling van je apparaat.
- Externe afbeeldingen worden standaard geblokkeerd, zodat een trackingpixel niet kan doorgeven dat
  je een bericht hebt geopend. Je kunt ze per bericht laden.
- De HTML van een bericht wordt opgeschoond en scripts draaien nooit, dus een bericht kan geen code
  uitvoeren, niet door de app navigeren, geen server benaderen en niets op je apparaat lezen.
- Het diagnostische logboek bevat aantallen, tijdsduren en technische gebeurtenissen, nooit de
  inhoud van berichten, onderwerpen, adressen, inloggegevens of handtekeningen. Het is begrensd tot
  enkele megabytes en verlaat je apparaat alleen als je het ons stuurt (§8).
- Meldingen worden op je apparaat gemaakt, nooit door een dienst van ons. Of er een voorbeeld
  (afzender, onderwerp, eerste regel) op je vergrendelscherm verschijnt, volgt je
  systeeminstellingen.

**Hoe lang.** Tot je het verwijdert, of de gegevens van de app of de app zelf.

## 3. Je e-mail-, agenda- en contactenproviders

**Wat waarheen gaat.** De app maakt alleen verbinding met de providers die je toevoegt, via IMAP,
SMTP, JMAP, CalDAV, CardDAV of de eigen API van een provider (zoals Microsoft 365 of Google). Zij
verwerken je gegevens onder hun eigen voorwaarden, zoals bij elke e-mailapp. Wij maken geen deel
uit van die relatie, en er gaat niets van naar Allodia.

**Inloggen.** Inloggen via OAuth gebeurt in de browser van je systeem, rechtstreeks bij de
provider: we hebben geen inlogserver en zien je wachtwoord nooit. Publiceert een JMAP-server zijn
eigen inlog, dan vraagt de app alleen je eigen e-mailserver, is alleen je **domein** erbij betrokken
(nooit je volledige adres) totdat je op de pagina van de provider bent, en vraagt de app alleen om
je e-mail, je agenda's en ingelogd blijven, nooit om je contacten of iets anders. Een server die
niets publiceert, krijgt niets.

**Je instellingen vinden** verstuurt, als je daarom vraagt, alleen je **domein**, nooit je
volledige adres: naar je eigen domein en dat van je provider, naar DNS, en naar de openbare
autoconfig-database van het Thunderbird-project (`autoconfig.thunderbird.net`, beheerd door
MZLA/Mozilla). Alles wordt via HTTPS geprobeerd; een bron die op een andere manier is bereikt, wordt
als niet-vertrouwd getoond en pas gebruikt als jij die goedkeurt. Er komt geen wachtwoord aan te
pas, Allodia ontvangt niets, en met "Handmatig instellen" sla je de opzoeking over.

**Contacten.** Het adresboek van een account wordt bij die provider gelezen en op je apparaat
bewaard; de contacten van je telefoon zelf worden nooit gelezen. De app onthoudt ook aan wie je
e-mail hebt verzonden, uit je map Verzonden. Een contact dat je toevoegt of bewerkt, wordt
teruggeschreven naar zijn adresboek, en de app verwijdert er nooit een. Vraagt een provider op zijn
inlogscherm om je contacten of de adreslijst van je organisatie, dan blijft de rest van het account
werken als je weigert; plaatjes bij afzenders komen van die provider.

**Gedeelde mailboxen en agenda's** worden geopend met de aanmelding van het account waarmee ze zijn
gedeeld, en op je apparaat bewaard zoals je eigen. Bij Microsoft wordt de toegang alleen gevraagd
zolang die optie is aangevinkt, en een adres dat je typt gaat alleen naar Microsoft, om te
controleren dat je het kunt openen. Er gaat niets over naar Allodia, ook niet met een
Allodia-account, en zet je er een uit, dan verdwijnt hij van je apparaat.

**Je afzendernaam** wordt ook in de instellingen van het account zelf opgeslagen, waar de provider
dat toestaat: de enige instelling die de app bij je provider wijzigt, en de app leest er geen
andere.

**Wat je zelf start**, gaat naar partijen die jij kiest, nooit via Allodia:

- E-mail en uitnodigingen die je verstuurt, gaan via je provider. Een handtekening verlaat je
  apparaat alleen daarin, met de afbeeldingen ingesloten, zodat er over het lezen niets kan worden
  teruggemeld.
- Externe afbeeldingen die je laadt, komen van hun servers (vaak die van de afzender), en die zien
  je IP-adres.
- Links en bijlagen openen in je browser of in de app die je systeem kiest; alleen links met
  `http`, `https`, `mailto` en `tel` worden doorgegeven, en bijlagen worden nooit binnen de app
  weergegeven.
- Een videovergadering die je aan een afspraak toevoegt, wordt gemaakt waar jij kiest. Een andere
  dienst dan de eigen server van de agenda krijgt alleen de titel en tijden van de afspraak.

**Hoe lang.** Op je apparaat zoals §2 zegt; bij je provider onder zijn voorwaarden.

## 4. Je Allodia-account (optioneel)

**Of je er een hebt.** De app raadt er een aan als je je eerste e-mailaccount toevoegt, omdat het je
accounts gelijk houdt op je apparaten. Je e-mail koppelen zonder account staat op hetzelfde scherm,
er wordt niet opnieuw naar gevraagd, en weigeren kost je geen enkel deel van de app. Inloggen kan
later in **Instellingen → Allodia-account**.

**Wat wij hebben.** Je e-mailadres, je weergavenaam als je die hebt, en de inlog. Zodra je op een
apparaat bent ingelogd: de lijst met e-mailaccounts daarop, per account het e-mailadres, de
servernamen en poorten, de gebruikersnaam en de verbindingsinstellingen. Hoort de schrijfassistent
bij je account, dan ook dát hij erbij hoort en, zodra je onze dienst ervoor gebruikt, het tegoed aan
credits waar zijn verzoeken van afgaan (§5.2). Koop je een abonnement, dan wat §7 noemt. Meer is
het niet, en de app doet geen ander verzoek aan ons.

**Wat wij nooit hebben.** Je e-mail, een postvak, of een wachtwoord of token voor je provider: die
blijven in je sleutelopslag, zodat een nieuw apparaat je accounts invult en je nog steeds om elk
wachtwoord vraagt. Een token dat voor het account is afgegeven, komt niet bij je provider.

**Hoe het wordt bewaard.** Op de eigen servers van Allodia in de EU. De accountlijst is daar
versleuteld, zodat een kopie van onze database alleen hem niet prijsgeeft, maar met een sleutel die
onze servers hebben: hij is niet end-to-end versleuteld, en wij kunnen hem lezen. Je apparaat
bewaart het inlogtoken van het account in zijn sleutelopslag.

**Inloggen met Apple of Google** is jouw keuze, en niets duwt je die kant op. Dat bedrijf weet dan
dat je een Allodia-account hebt en geeft ons het adres en de naam die het vrijgeeft; rechtstreeks
inloggen gebeurt zonder derde partij.

**Grondslag.** Uitvoering van onze overeenkomst met jou (AVG art. 6, lid 1, onder b).

**Hoe lang, en hoe je stopt.** Tot het account wordt verwijderd. Uitloggen in Instellingen →
Allodia-account wist de inlog van dat apparaat; je e-mailaccounts blijven erop staan, en de lijst
blijft bij je account. **Instellingen → Allodia-account → Account verwijderen** verwijdert het
account en alles wat wij erin hebben: het opent je accountpagina, ook te vinden op
**https://mailcal.allodia.eu/account** op elk apparaat zonder de app, waar je inlogt en het
bevestigt. Of mail **info@allodia.eu**.

## 5. AI

### 5.1 Een assistent op je computer

**Wat.** Via het Model Context Protocol (MCP) kan een AI-assistent op je eigen computer de e-mail
lezen en bewerken van de accounts die je toestaat. Dit bestaat op macOS en Windows, en staat uit
tot je het aanzet bij **Instellingen → Geavanceerd**.

**Waar het heen gaat.** Via een lokaal kanaal dat alleen je eigen gebruikersaccount van het
besturingssysteem kan bereiken: geen netwerkpoort, geen server van ons, niets naar Allodia. We
weten niet of het aanstaat, welke assistent je gebruikt of wat je vraagt.

**Wat hij kan.** Aanzetten geeft toegang tot geen enkele mailbox: elk account is een apart vinkje,
en uitvinken werkt meteen. Bij een aangevinkt account kan hij berichten opsommen, doorzoeken en
lezen, als gelezen markeren, een vlag geven, archiveren, naar de prullenbak verplaatsen, als spam
markeren en een vooringevuld concept openen dat jij verstuurt; definitief verwijderen kan hij
nooit. Versturen is een aparte schakelaar, standaard uit, en staat die aan, dan weigert de app nog
steeds ontvangers aan wie je nooit hebt gemaild, tenzij je dat ook uitzet.

**Wat je moet afwegen.** Wat de assistent leest, kan zijn eigen aanbieder ontvangen, onder diens
eigen beleid: een cloud-assistent haalt berichtinhoud van je apparaat zodra je hem vraagt iets te
lezen. Dat is jouw verzending, naar een partij die jij kiest.

### 5.2 Schrijfstijl en opgestelde antwoorden

**Wat.** De app kan leren hoe jij schrijft van e-mail die je hebt verstuurd, en in die stijl een
antwoord opstellen dat jij leest, aanpast en verstuurt; zelf verstuurt hij nooit e-mail. In de
versies die wij uitbrengen is hij in vroege toegang, en alleen aangeboden zolang hij bij je
Allodia-account hoort.

**Wanneer.** Voordat hij leert, laat de app zien hoeveel verzonden berichten hij vond in de periode
die je koos en waar ze naartoe zouden gaan, en hij verstuurt niets tot je op **Leren** drukt. Een
antwoord wordt alleen opgesteld als je op **Antwoord opstellen** drukt. Het leren kan oudere
verzonden e-mail van je eigen e-mailserver ophalen, en haalt die weer van je apparaat zodra hij is
gelezen.

**Wat een verzoek meeneemt.** Om te leren: de woorden die je in die berichten schreef, met
geciteerde e-mail en handtekeningen op je apparaat weggeknipt, nooit de onderwerpen of ontvangers.
Om een antwoord op te stellen: het bericht waarop je antwoordt, met de afzender, de datum en de
e-mail die het citeert; de beschrijving van je stijl en een paar passages uit je eigen eerdere
e-mail; wat je schreef in hooguit twee van je laatste berichten aan hetzelfde adres; een instructie
die je typte; en de tekst van de handtekening van je antwoord. Nooit een bijlage, en nooit je
adresboek.

**Waar het heen gaat.**

- **Een eigen AI-endpoint**, in te stellen bij **Instellingen → Geavanceerd**: elke server die de
  chat-interface van OpenAI spreekt, op je computer of gekozen door jou of je organisatie. Zolang
  er een is ingesteld, gaat elk verzoek daarheen en nergens anders, zit Allodia niet in dat pad, en
  is wat er daar gebeurt aan wie hem beheert. De sleutel blijft in je sleutelopslag en gaat alleen
  naar de server waarvoor hij is opgegeven.
- **De dienst van Allodia**, als je geen eigen endpoint hebt en bent ingelogd op een Allodia-account
  waar de schrijfassistent bij hoort. Het verzoek gaat naar onze servers in de EU, die het doorgeven
  aan een AI-aanbieder in de EU, beheerd door een Europees bedrijf, die onze dienst kiest. Allodia
  bewaart niets van het verzoek of het antwoord en legt geen inhoud vast: alleen het soort verzoek,
  de grootte, hoe lang het duurde en de credits die het kostte, afgeschreven van het tegoed van je
  account.

**Gecontroleerd voordat er iets wordt gelezen.** Voordat de app je verzonden e-mail of het bericht
waarop je antwoordt leest, en opnieuw vóór elk verzoek, controleert hij de bestemming: alleen een
dienst die in de EU verwerkt en wordt beheerd door een bedrijf dat in de EU is gevestigd, of een
model op je eigen computer, wordt toegelaten. Onze dienst geeft verzoeken alleen aan zulke
aanbieders door; een eigen endpoint wordt pas toegelaten als jij dat hebt aangegeven waar je hem
instelt. Anders wordt er niets gelezen of verstuurd.

**Wat op je apparaat blijft.** Je stijlen (van elke stijl de naam, de geleerde beschrijving, je
notities en de passages uit je e-mail die hij als voorbeeld bewaart), nergens anders bewaard; het
adres, het model en de sleutel van je endpoint; en alleen de identificaties van de antwoorden waar
een concept in terechtkwam, verstuurd of opgeslagen op dit apparaat. Een passage verlaat het
apparaat alleen binnen een verzoek.

**Een markering op antwoorden waaraan hij meeschreef.** Zodat geen apparaat je stijl leert van de
woorden van het model, vraagt de app je provider de kopie van zo'n antwoord in Verzonden en het
concept in Concepten te markeren: een trefwoord met de naam `writing-assistant` op een IMAP- of
JMAP-server die er een accepteert, een label bij Gmail en een categorie bij Microsoft, met de naam
"Writing assistant", die je daar ziet. Waar een server geen markering bewaart, weet alleen het
apparaat dat het antwoord verstuurde of opsloeg ervan. Er gaat niets over de markering naar
Allodia.

**Grondslag.** Een verzoek via onze dienst: uitvoering van onze overeenkomst met jou (AVG art. 6,
lid 1, onder b). De persoonsgegevens van de afzender in het bericht waarop je antwoordt:
gerechtvaardigd belang, namelijk jouw belang om je eigen e-mail te beantwoorden (art. 6, lid 1,
onder f). Met een eigen endpoint verwerkt Allodia niets, en geldt er dus geen grondslag van ons.

**Hoe lang, en hoe je stopt.** Een verzoek wordt niet bewaard; het tegoed aan credits blijft tot je
Allodia-account wordt verwijderd. Een stijl vergeten haalt hem van je apparaat, en je endpoint
verwijderen haalt het adres, het model en de sleutel weg.

## 6. Gebruiksstatistieken (optioneel)

**Of je ze deelt.** Bij de eerste start vraagt de app het. De schakelaar staat **uit**: er wordt
niets opgeslagen of verstuurd tenzij je hem aanzet en bevestigt, en een nee wordt onthouden. Het
toestemmingsscherm toont de letterlijke gegevens, byte voor byte, en is doorslaggevend boven deze
samenvatting.

**Wat er wordt verstuurd als je ervoor kiest.**

| Gegeven | Waarde | Bewust niet |
|---|---|---|
| Installatie-id | Een willekeurige identifier, die pas op het moment van je toestemming wordt aangemaakt | Niet afgeleid van je apparaat, je account of enige hardware-id |
| Platform + besturingssysteem | bijv. `android`, alleen het hoofdversienummer (`15`) | Nooit een buildnummer |
| Apparaatklasse | bijv. `iphone`, `mac-laptop`, `android-tablet` | Nooit een apparaatmodel |
| App-versie + taal | bijv. `1.4.0`, `nl` | Alleen de taal, nooit een volledige locale/regio |
| Accountvorm | Hoeveel accounts (in groepen: `0`, `1`, `2`, `3–5`, `6+`) en welke soorten protocollen | Nooit welke providers, hosts of adressen |
| Gebeurtenissen | App geopend; accountinstelling gestart/voltooid/mislukt en synchronisatie voltooid/mislukt (per protocolsoort); een functie is gebruikt (uit een vaste lijst); welke instellingen aanstaan | Nooit iets wat je als tekst hebt getypt of ingesteld |

**Nooit verstuurd, door de bouw en niet alleen op papier:** de inhoud van berichten, onderwerpen,
afzenders, ontvangers, e-mailadressen, mapnamen, aantallen berichten, zoekopdrachten, agendatitels
of afspraakdetails, namen van bijlagen, servernamen, apparaatmodellen. Elk veld is een label uit een
vaste lijst of een groep, en onze server weigert al het andere.

**Waar.** Alleen naar servers die Allodia zelf in de EU beheert (Hetzner Online GmbH,
Industriestraße 25, 91710 Gunzenhausen, Duitsland), nooit naar een analysebedrijf van derden. Je
IP-adres is zichtbaar wanneer je apparaat verbinding maakt, maar het wordt niet opgeslagen en niet
aan de gegevens gekoppeld.

**Waarom.** Alleen voor productbeslissingen, zoals of instellen en synchroniseren lukt en welke
functies worden gebruikt. Het installatie-id blijft gelijk zolang je toestemming geeft, dus
behandelen we dit als pseudonieme persoonsgegevens, niet als anoniem.

**Grondslag.** Jouw toestemming (AVG art. 6, lid 1, onder a; opslaan en uitlezen op je apparaat
volgens de nationale implementaties van artikel 5(3) van de ePrivacyrichtlijn).

**Hoe lang, en hoe je stopt.** Maximaal 24 maanden, daarna verwijderd of teruggebracht tot
geaggregeerde cijfers die bij geen installatie-id meer horen. Intrekken doe je bij **Instellingen →
Privacy**, één schakelaar, even eenvoudig als toestemming geven: de app verwijdert het
installatie-id en de vastlegging van je toestemming van je apparaat en draagt onze server op alles
te wissen wat onder dat id is opgeslagen (art. 17). Willen we ooit meer versturen, dan vraagt de app
het opnieuw en laat hij eerst de nieuwe gegevens zien; een payload groeit nooit onder een
toestemming die voor minder is gegeven.

## 7. Betalen, en de appstores

**De stores** waarmee je de app installeert en bijwerkt (Apple's App Store, Google Play, de
Microsoft Store) zijn zelfstandige verwerkingsverantwoordelijken onder hun eigen beleid. Ze geven
ons alleen geaggregeerde dashboards (installaties, actieve apparaten, versies van
besturingssystemen, apparaatmodellen, crashstatistieken) die jou niet identificeren, gevoed volgens
de deelinstellingen van je systeem. De app heeft geen eigen crashrapportage en doet zelf geen
updatecontroles.

**Een abonnement** hoort bij een Allodia-account (§4), dat het meeneemt naar elk apparaat waarop je
inlogt, dus kopen bij een store betekent eerst inloggen. Het voegt geen e-mail, geen postvak en geen
nieuwe toegang tot een van beide toe. Waar je ook koopt, je pas- of bankgegevens ontvangen we nooit.

- **Bij ons**, via **Mollie B.V.** (Amsterdam, Nederland) op hun eigen afrekenpagina. We bewaren
  het plan, de prijs, wanneer de volgende betaling volgt en of elke betaling is gelukt, mislukt of
  terugbetaald, met de referenties die Mollie gebruikt voor je klantgegevens, je betaalmachtiging en
  elke betaling. We geven Mollie je naam en e-mailadres, om de betaling te doen en te factureren.
- **Bij Apple of Google, in de app.** Je betaalt die store, als zelfstandige
  verwerkingsverantwoordelijke. Van je apparaat ontvangen we alleen de referentie van de store voor
  de aankoop, welke store, en wanneer; niets over je apparaat. We vragen de store of de referentie
  echt is en waar je recht op hebt, en bewaren de store, de referentie, het plan, of het actief,
  opgezegd, verlopen of terugbetaald is, tot wanneer het loopt, of het doorloopt, en de prijs en
  valuta die is gerekend. De app vraagt het van tijd tot tijd opnieuw, zodat opzeggen of laten
  verlopen hier ook doorwerkt.

**Grondslag.** Uitvoering van onze overeenkomst met jou (AVG art. 6, lid 1, onder b); voor facturen
en betaalgegevens onze wettelijke plicht om ze te bewaren (art. 6, lid 1, onder c).

**Hoe lang, en hoe je stopt.** Tot je je account verwijdert, behalve facturen en betaalgegevens, die
we bewaren zolang de Nederlandse belastingwet vereist (zeven jaar). Een abonnement bij ons zeg je op
je accountpagina op, en een abonnement van Apple of Google bij die store, die ook over een
terugbetaling beslist. Opzeggen stopt de volgende betaling; het wist de gegevens van betalingen die
al zijn gedaan niet.

## 8. Als je contact met ons opneemt

Wat je ons stuurt per e-mail (bijvoorbeeld naar **info@allodia.eu**) of via het contactformulier op
de website (je naam en e-mailadres, een optionele bedrijfsnaam, je bericht en wat je meestuurt, bij
support mogelijk het diagnostische logboek, dat geen berichtinhoud bevat) gebruiken we alleen om je
te antwoorden en het probleem op te lossen. Grondslag: ons gerechtvaardigd belang om de mensen die
ons schrijven te antwoorden (art. 6, lid 1, onder f), of stappen voorafgaand aan een overeenkomst
wanneer je naar een aankoop vraagt (art. 6, lid 1, onder b). Onze mailboxen worden gehost door
**Soverin** in Nederland, en correspondentie wordt 24 maanden na het laatste bericht verwijderd.

## 9. De website

allodia.eu gebruikt alleen de cookies en lokale opslag die nodig zijn om te functioneren
(taalvoorkeur, beveiliging, sessiebeheer), en geen analyse- of marketingcookies; mocht dat ooit
veranderen, dan vragen we het eerst. De site wordt gehost in Nederland en Duitsland. Het
contactformulier valt onder §8.

## 10. Wat we nooit doen

- We verkopen of verhuren nooit persoonsgegevens, en delen ze nooit voor advertenties.
- We gebruiken nooit analyse-, tracking- of advertentie-SDK's van derden in de app.
- We nemen nooit geautomatiseerde besluiten over je en stellen nooit een profiel van je op (AVG
  art. 22).
- We trainen nooit AI-modellen op je e-mail. We hebben er geen toegang toe, behalve tot wat een
  verzoek aan de dienst van onze schrijfassistent meeneemt terwijl onze servers het doorgeven, en
  daarvan bewaren we niets (§5.2).
- We dragen de persoonsgegevens die we hebben nooit over buiten de EU/EER, met één uitzondering die
  we liever benoemen dan wegstoppen: **een abonnement controleren dat bij Apple of Google is
  gekocht** betekent die store ernaar vragen, en dat zijn allebei Amerikaanse bedrijven (§7). De
  vraag bevat de eigen referentie van de store voor een transactie die die store al heeft, en kopen
  bij ons voorkomt het. Waar je eigen provider zit, en of je met Apple of Google inlogt op een
  Allodia-account (§4), is jouw keuze.

## 11. Bewaartermijnen in één oogopslag

| Gegevens | Bewaard | Waar |
|---|---|---|
| Je e-mail, afspraken, contacten, instellingen, handtekeningen, schrijfstijlen, diagnostisch logboek | Tot je het verwijdert, of de gegevens van de app of de app zelf | Je apparaat |
| Je Allodia-account en de lijst met e-mailaccounts (§4) | Tot het account wordt verwijderd | Servers beheerd door Allodia, EU |
| Een verzoek aan de dienst van onze schrijfassistent (§5.2) | Niet bewaard | Via servers beheerd door Allodia, EU, doorgegeven aan een AI-aanbieder in de EU |
| Het tegoed aan credits van de schrijfassistent, zodra je onze dienst gebruikt (§5.2) | Tot je Allodia-account wordt verwijderd | Servers beheerd door Allodia, EU |
| Gebruiksstatistieken, als je ervoor kiest (§6) | Tot je ze intrekt, maximaal 24 maanden | Servers beheerd door Allodia, EU |
| Een abonnement dat je bij ons koopt, en de betaalgeschiedenis ervan (§7) | Tot je account wordt verwijderd, facturen uitgezonderd | Servers beheerd door Allodia, EU; betalingen door Mollie, NL |
| Een abonnement gekocht bij Apple of Google, en de referentie van de store (§7) | Tot je account wordt verwijderd | Servers beheerd door Allodia, EU |
| Facturen en betaalgegevens, als je bij ons koopt (§7) | Zolang de Nederlandse belastingwet vereist (7 jaar) | Administratie van Allodia, EU |
| Supportcorrespondentie en berichten via het contactformulier (§8) | 24 maanden na het laatste bericht | Soverin, NL |

## 12. Je rechten

Onder de AVG kun je ons vragen om inzage in, correctie of verwijdering van je persoonsgegevens of
een kopie ervan (overdraagbaarheid), ons vragen de verwerking te beperken, bezwaar maken tegen
verwerking op basis van gerechtvaardigd belang, en elke toestemming op elk moment intrekken, zonder
dat dat afdoet aan wat daarvóór rechtmatig was. Je bent wettelijk noch contractueel verplicht ons
persoonsgegevens te geven: de app werkt volledig met elke optionele keuze uit.

De snelste route is meestal de app, waar je gegevens al zijn. **Instellingen → Privacy** trekt de
gebruiksstatistieken in en laat ze wissen; omdat een installatie-id ons niet vertelt wie je bent,
kunnen we het jouwe niet op naam of e-mailadres vinden, en is die schakelaar de betrouwbare manier
om het te laten wissen.

Voor al het overige: mail **info@allodia.eu**; we reageren binnen een maand (art. 12(3)). Je kunt
ook een klacht indienen bij een toezichthouder: die van ons is de Nederlandse **Autoriteit
Persoonsgegevens** (autoriteitpersoonsgegevens.nl), maar je mag ook die van je eigen EU/EER-land
gebruiken.

## 13. Beveiliging

Verbindingen met je provider gebruiken de versleutelde protocollen die hij aanbiedt (TLS), en §2
zegt hoe je apparaat je gegevens beschermt. Het weinige dat we serverzijdig beheren, draait op
EU-infrastructuur met toegang beperkt tot wie die nodig heeft, beschermd met
meerfactorauthenticatie en versleuteling.

## 14. Wijzigingen in dit beleid

We werken dit beleid, met versienummer en datum, bij **vóórdat** een verandering in welke gegevens
worden verwerkt uitkomt, en alles wat nieuwe gegevens zou versturen, vraagt het je voordat het
verstuurt. Waar een verandering zou verbreden wat de app verstuurt, vraagt de app je opnieuw om
toestemming in plaats van die aan te nemen. De actuele versie staat altijd op
**https://allodia.eu/privacy/mail-calendar**.

## 15. Contact

Allodia · KvK 56789823 · Kamerlingh Onnesweg 2, 3316GL Dordrecht, Nederland · **info@allodia.eu**
