# Privacybeleid: Allodia Mail & Calendar

**Versie 2.2 · Van kracht: 2026-08-28**

Allodia Mail & Calendar is een e-mail- en agenda-app die op je eigen apparaat draait en verbinding
maakt met de e-mailprovider die **jij** kiest. Dit beleid legt in gewone taal uit wat dat betekent
voor je gegevens: wat op je apparaat blijft (vrijwel alles), wat de app verstuurt en naar wie
(standaard niets naar ons), en de twee dingen die je met ons kunt kiezen te delen (een
Allodia-account, en gebruiksstatistieken, allebei uit tenzij je ze aanzet).

Daarom heeft dit beleid twee helften. **Zonder Allodia-account**, zoals de app zich installeert,
hebben wij helemaal niets van je (§2). **Mét een account**, dat de app je aanraadt zodra je je
eerste e-mailaccount toevoegt en dat je met één tik kunt overslaan, hebben we wie je bent en de
lijst met e-mailaccounts die je door ons gelijk laat houden op je apparaten, en nog steeds nooit je
e-mail (§3).

## De korte versie

- **Je e-mail komt nooit langs Allodia.** De app synchroniseert rechtstreeks tussen je apparaat en
  je eigen e-mailprovider. Wij zitten niet in dat pad, we hebben geen servers in dat pad, en we
  kunnen je e-mail, je afspraken en je inloggegevens niet lezen.
- **Standaard stuurt de app ons niets.** Geen telemetrie, geen crashrapporten, geen identifiers,
  zelfs niet het feit dát je de app hebt geïnstalleerd.
- **Een Allodia-account is optioneel, en de app raadt het je aan.** Zodra je je eerste
  e-mailaccount toevoegt, biedt de app het aan als de makkelijkste weg, want het is wat je accounts
  gelijk houdt op al je apparaten. Je e-mail rechtstreeks koppelen staat op hetzelfde scherm en
  kost je niets. Er komt nooit e-mail in een account: het bevat adressen en servernamen, geen
  postvak (§3).
- **Eén ding dat de app aanbiedt te versturen, en jij beslist.** Bij de eerste start vraagt de app
  of je gebruiksstatistieken wilt delen. De schakelaar staat uit; de app laat je precies zien welke
  gegevens het betreft voordat je beslist; nee zeggen kost je niets en wordt onthouden. Je kunt je
  toestemming op elk moment met één klik intrekken.
- **Een AI-assistent kan toegang tot je e-mail krijgen, en alleen als jij dat zegt.** Het staat
  uit, hij draait op je eigen computer, hij komt nergens bij tot je een account aanvinkt, en
  Allodia zit ook niet in dat pad (§7).
- **We verkopen nooit gegevens, tonen nooit advertenties, stellen nooit een profiel van je op en
  gebruiken nooit analyse- of trackingdiensten van derden.** Wat we wél ontvangen, blijft op
  servers die we zelf in de EU beheren.
- **Je rechten zijn die van de AVG**, en de meeste ervan kun je rechtstreeks in de app uitoefenen,
  want de gegevens zijn immers al in jouw handen.

## 1. Wie we zijn

Allodia ("Allodia", "wij"),
ingeschreven bij de Kamer van Koophandel (KvK) onder nr. **56789823**, Kamerlingh Onnesweg 2, 3316GL Dordrecht, Nederland.

Voor alles wat in dit beleid staat: **info@allodia.eu**.

We zijn alleen "verwerkingsverantwoordelijke" voor het weinige dat ons daadwerkelijk bereikt: een
Allodia-account als je er een aanmaakt (§3), optionele gebruiksstatistieken (§6), berichten die je
ons stuurt (§8) en de website (§10). Voor alles wat de app op je apparaat verwerkt, zijn we
verwerkingsverantwoordelijke noch verwerker, want we ontvangen het nooit (§2).

## 2. Standaard: alles blijft op je apparaat

De app bewaart je e-mail, agenda-afspraken, contacten, bijlagen, accountinstellingen, je
e-mailhandtekeningen en een lokaal diagnostisch logboek **op je apparaat**. Niets van je inhoud
wordt naar Allodia geüpload. Er is geen kopie in de cloud en geen backend van ons die je inhoud
bewaart, en zolang je niet zelf inlogt op een Allodia-account (§3), hebben wij helemaal niets van
je. Inloggen voegt daar precies één ding aan toe, je lijst met e-mailaccounts, en §3 zegt exact wat
daarin staat.

De enige netwerkverbindingen die de app standaard maakt, lopen tussen je apparaat en **de e-mail-,
agenda- en adresboekproviders die je koppelt**, via open standaarden (IMAP, SMTP, JMAP, CalDAV,
CardDAV) of de eigen API van een provider (bijvoorbeeld Microsoft 365, of Google voor Gmail + Google Agenda). Je
provider verwerkt je e-mail onder **zijn** voorwaarden en privacybeleid, precies zoals bij elke
andere e-mailapp; die relatie verandert niet doordat je de provider hier koppelt, en wij komen er
niet bij in.

Inloggen bij een provider die OAuth gebruikt (bijvoorbeeld Microsoft 365, Google, of een
JMAP-server zoals Fastmail) gebeurt in de browser van je systeem, rechtstreeks tussen jou en de
provider. Allodia beheert geen inlogserver, ziet je wachtwoord nooit, en de tokens die daaruit
volgen worden alleen opgeslagen in de beveiligde sleutelopslag van je apparaat.

Voor een **JMAP**-account kan de app dit inloggen zelfs aanbieden voor een provider die we nooit
hebben geïntegreerd, door te lezen wat je server zelf publiceert: de app vraagt je server waar zijn
inlogdienst zit en welke rechten er bestaan, en registreert zich vervolgens bij die server als een
app op jouw apparaat. Drie dingen begrenzen dat. Die verzoeken gaan **alleen naar je eigen
e-mailserver**, nooit naar Allodia, en nooit naar een derde partij. Alleen je **domein** is erbij
betrokken, nooit je volledige e-mailadres, totdat je op de inlogpagina van je eigen provider
belandt. En de app vraagt om de **smalst mogelijke** rechten om zijn werk te doen: je e-mail, je
agenda's, en toestemming om ingelogd te blijven. Nooit je contacten, en nooit wat je server verder
ook aanbiedt. Publiceert je server dit niet, dan wordt er niets verstuurd en log je gewoon in
met een wachtwoord of een API-token, zoals voorheen.

**Je contacten.** Heeft het account dat je koppelt een adresboek, dan kan de app dat lezen (via
CardDAV of JMAP, bij dezelfde provider, met dezelfde inloggegevens), zodat je je contacten in de
app kunt opzoeken en suggesties krijgt terwijl je een bericht adresseert. Ze worden net als je
e-mail op je apparaat bewaard, ze gaan nooit naar Allodia, en de app leest de contacten van je
telefoon zelf niet. De app onthoudt daarnaast aan wie je e-mail hebt **verzonden**, uit je eigen
map Verzonden op je eigen server, zodat die suggesties al bruikbaar zijn voordat je iemand hebt
toegevoegd. Contacten zijn in deze versie alleen-lezen: de app maakt, wijzigt of verwijdert niets
in je adresboek.

Log je in bij **Microsoft, Google of een provider met een inlogscherm**, dan vraagt de app als
onderdeel van dat inloggen toegang tot je contacten, en waar de provider die heeft ook tot de
adreslijst van je organisatie. Op het toestemmingsscherm zie je precies wat er gevraagd wordt
voordat je akkoord gaat. Ze worden voor dezelfde dingen gebruikt als elk ander adresboek (een
contact opzoeken en een adres voorstellen), plus het kleine plaatje naast een afzender, dat de
app bij je provider ophaalt en op je apparaat bewaart. Er gaat niets naar Allodia, en weiger je,
dan blijft de rest van het account gewoon werken; je ziet dan alleen geen contacten en geen
plaatjes.

Het toestemmingsscherm noemt dit **volledige toegang tot je contacten**, inclusief het recht om
ze te wijzigen. Dat is bewust, en het is meer dan de app doet: contacten bewerken is een functie
die we bouwen, en door er nu om te vragen geef je één keer toestemming in plaats van opnieuw door
het inlogscherm te moeten wanneer die functie er is. Tot dan geldt de belofte hierboven precies
zoals ze er staat: de app maakt, wijzigt en verwijdert niets in je adresboek, en dit beleid zal
dat blijven zeggen tot de dag dat het verandert.

**Je handtekeningen.** Een handtekening die je schrijft (de tekst, en elke afbeelding die je
erin zet, zoals een logo) staat in een bestand op je apparaat, naast je andere instellingen, en
nergens anders. Hij gaat nooit naar Allodia. Je apparaat verlaat hij alleen waar je dat zou
verwachten: in een bericht dat **jij** verstuurt, naar de ontvangers die **jij** kiest, via je
eigen provider. Een ingesloten afbeelding reist mee in dat bericht; er wordt niets opgehaald op
het moment dat je ontvanger het leest, dus over dat lezen kan niets worden teruggemeld. De inhoud
van een handtekening komt nooit in het diagnostisch logboek.

Privacybescherming die op elk platform is ingebouwd:

- **Externe afbeeldingen in berichten worden standaard geblokkeerd.** Trackingpixels zijn externe
  afbeeldingen; een bericht kan dus niet doorgeven dat je het hebt geopend. Je kunt afbeeldingen
  per bericht laden, en die keuze vervalt weer bij het volgende bericht (§5).
- **De HTML van een bericht wordt opgeschoond en scripts draaien nooit.** Een bericht kan geen code
  uitvoeren, niet door de app navigeren, niet "naar huis bellen" en niets op je apparaat lezen.
- **Het diagnostische logboek bevat nooit inhoud.** Het legt aantallen, tijdsduren en technische
  gebeurtenissen vast, nooit de inhoud van berichten, onderwerpen, adressen of inloggegevens. Het
  is begrensd tot enkele megabytes, en blijft op je apparaat, tenzij je er zelf voor kiest het naar
  ons te sturen (§8).
- **Meldingen van nieuwe e-mail worden op je apparaat gemaakt.** Geen enkele meldingsdienst van ons
  ziet je e-mail. Of er een voorbeeld (afzender, onderwerp) op je vergrendelscherm verschijnt,
  volgt de meldingsinstellingen van je besturingssysteem.
- **Inloggegevens staan in de sleutelopslag van het platform** (Keychain, Windows Credential
  Manager, Android Keystore of de Linux-systeemsleutelring via Secret Service), nooit in de
  database van de app. De berichtenopslag wordt beschermd door de versleuteling van je apparaat.

**Je instellingen vinden als je een account toevoegt.** Zodat je geen servernamen hoeft in te
typen, kan de app ze op jouw verzoek afleiden uit je e-mailadres. Hij kijkt op de standaardplekken
naar de instellingen die je provider publiceert: de autodiscovery-adressen voor e-mail, JMAP en
agenda op **je eigen e-maildomein en het domein van je provider**, een gewone DNS-opzoeking naar de
mailhost van je provider, en de **openbare autoconfig-database van het Thunderbird-project**
(`autoconfig.thunderbird.net`, beheerd door MZLA/Mozilla), een gedeelde gids met
providerinstellingen. Twee regels begrenzen dat allemaal: alleen je **domein** wordt ooit
verstuurd, nooit je volledige e-mailadres; en alles wordt over HTTPS geprobeerd, en een bron van
instellingen die op een andere manier is bereikt, wordt aan je getoond als niet-vertrouwd en wordt
nooit gebruikt om verbinding te maken voordat jij daar toestemming voor geeft. Er komt geen
wachtwoord aan te pas (dit gebeurt vóór het inloggen), Allodia ontvangt er niets van, en met
"Handmatig instellen" sla je de opzoeking helemaal over.

Er zit geen clouddienst van ons tussen jou en je provider. De app kan je e-mail doorgeven aan een
AI-assistent op je eigen computer, maar alleen als je dat aanzet en alleen voor de accounts die
je kiest (zie §7). Voegen we een functie toe die verandert hoe gegevens worden verwerkt, dan
werken we dit beleid **eerst** bij, en alles wat nieuwe gegevens zou versturen, vraagt het je
vóórdat het iets verstuurt.

## 3. Als je inlogt op een Allodia-account (optioneel)

**Je krijgt het aangeboden, en je kunt het overslaan.** Alles in §2 beschrijft de app zonder
account, en zo blijft het tenzij jij anders beslist. Zodra je je eerste e-mailaccount toevoegt,
raadt de app je aan er een aan te maken, want dat is wat je accounts gelijk houdt op al je
apparaten. Je e-mail rechtstreeks koppelen staat op hetzelfde scherm, later vraagt niets er nog
een keer om, en weigeren kost je geen enkel deel van de app. Inloggen kan ook in
**Instellingen → Allodia-account**, een eigen plek, los van je e-mailaccounts.

**Wat het is.** Een account bij ons, voor de onderdelen van Allodia Mail & Calendar die een server
van ons nodig hebben om überhaupt te werken. Er hoort geen postvak bij: het kan je e-mail niet
lezen, versturen of bewaren, en een token dat ervoor is afgegeven komt niet bij je provider.
Inloggen verandert niets aan §2: je e-mail synchroniseert nog steeds rechtstreeks tussen je
apparaat en de provider die jij hebt gekozen, en wij zitten nog steeds niet in dat pad.

**Wat wij hebben.** Je e-mailadres, je weergavenaam als het account die heeft, en de inlog zelf.
En zodra je op een apparaat bent ingelogd: de lijst met e-mailaccounts op dat apparaat, per account
het e-mailadres, de servernamen en poorten, de gebruikersnaam en de verbindingsinstellingen. Meer
is het niet.

**Nooit een wachtwoord, en nooit een token voor je provider.** Die blijven in de sleutelopslag van
je eigen apparaat, gaan nooit naar ons toe, en vul je één keer per apparaat in. Daarom vult
inloggen op een nieuw apparaat je accounts wel voor je in, maar vraagt het je daarna nog steeds om
elk wachtwoord.

**Wij kunnen de accountlijst die wij hebben lezen.** Die staat op onze servers in gewone leesbare
vorm, niet versleuteld op een manier die ons buitensluit. Hij vertelt ons welke providers je
gebruikt en onder welk adres; hij vertelt ons niets over je e-mail, die wij niet hebben. Vind je
dat meer dan je wilt delen, dan is dit het deel van de app dat optioneel is: gebruik hem zonder
Allodia-account en er gaat niets van je apparaat af.

**Wat je apparaat bewaart.** Het inlogtoken, in de beveiligde sleutelopslag van je
besturingssysteem (Keychain, Windows Credential Manager, Android Keystore of de sleutelbos van
Linux), dezelfde plek waar je e-mailwachtwoorden al staan, en nooit in de database van de app.

**Waar.** Op onze eigen servers, in de EU.

**Hoe je inlogt, en wat dat je kost.** Rechtstreeks bij ons, of met Apple of Google als je dat
liever hebt. Die laatste twee zijn jouw keuze en niets duwt je die kant op. Kies je er een, dan weet
dat bedrijf dat je een Allodia-account hebt, en geeft het ons het adres en de naam die het vrijgeeft;
dat is dan wat wij bewaren, precies zoals hierboven staat, en niets meer. Log je rechtstreeks in,
dan zit er helemaal geen derde partij in. Hoe dan ook is het account van ons, op onze servers in de
EU, en je e-mail zit in niets daarvan.

**Uitloggen, en verwijderen.** Uitloggen in Instellingen → Allodia-account wist de kopie van de
inlog op
dit apparaat. Je e-mailaccounts blijven op het apparaat staan, en de lijst die wij hebben blijft bij
je Allodia-account tot dat account wordt verwijderd. Wil je het account zelf verwijderen, met alles
wat wij erin hebben en die lijst erbij: **Instellingen → Allodia-account → Account verwijderen**.
Dat opent je accountpagina in je browser, waar je inlogt en het bevestigt — de app verwijdert het
account niet zelf. Die pagina kun je ook rechtstreeks openen op
**https://mailcal.allodia.eu/account**, op elk apparaat en zonder de app. Vraag je het liever aan
ons, mail dan **info@allodia.eu**. Je rechten staan in §13.

**Wat het vandaag doet.** Een identiteit, en de accountlijst hierboven. Verder doet de app geen
enkel verzoek aan ons. Elke volgende functie die echt een server van ons nodig heeft, wordt in dit
beleid opgenomen **voordat** het uitkomt, en vraagt het je voordat er iets wordt verstuurd (§15).

## 4. Gegevens die je niet hoeft te verstrekken

Geen enkele. Er is geen wettelijke of contractuele verplichting om ons persoonsgegevens te
verstrekken, en de app werkt volledig als je ons nooit iets stuurt en elke optionele schakelaar uit
laat staan.

## 5. Wat er alleen gebeurt als jij iets doet

Sommige handelingen in een e-mailapp versturen noodzakelijkerwijs gegevens ergens naartoe, maar
naar partijen die **jij** kiest, op het moment dat jij kiest, nooit via Allodia:

- **Een account koppelen** stuurt je inloggegevens naar die provider en synchroniseert je e-mail
  ermee.
- **Externe afbeeldingen laden** in een bericht haalt ze op bij de servers die ze hosten (vaak die
  van de afzender), en die zien je IP-adres. Daarom blokkeert de app ze standaard en vraagt hij
  het per bericht.
- **Op een link tikken of een bijlage openen** geeft die door aan de browser van je systeem of aan
  de app die je besturingssysteem aan het bestand koppelt. Alleen gewone web- en e-maillinks
  (`http`, `https`, `mailto`) worden ooit doorgegeven; bijlagen worden nooit binnen de app zelf
  weergegeven.
- **E-mail of uitnodigingen versturen** levert ze via je provider af bij je ontvangers.

Dit zijn jouw verzendingen, niet de onze: Allodia ontvangt van geen van alle iets.

## 6. Optionele gebruiksstatistieken (standaard uit)

Het enige dat de app ons kan sturen, en **alleen als je ervoor kiest**.

Bij de eerste start, voordat er een account is ingesteld, vraagt de app of je gebruiksstatistieken
wilt delen. De schakelaar staat **uit**. Er wordt niets opgeslagen of verstuurd tenzij je hem
aanzet en bevestigt. Weigeren wordt onthouden, en we vragen het niet nog eens. Het
toestemmingsscherm heeft een paneel **"Bekijk precies wat we versturen"** dat de letterlijke gegevens toont, byte voor
byte; de beschrijving hieronder vat dat paneel samen, maar het paneel is doorslaggevend.

**Wat er wordt verstuurd als je ervoor kiest:**

| Gegeven | Waarde | Bewust niet |
|---|---|---|
| Installatie-id | Een willekeurige identifier, die pas op het moment van je toestemming wordt aangemaakt | Niet afgeleid van je apparaat, je account of enige hardware-id |
| Platform + besturingssysteem | bijv. `android`, alleen het hoofdversienummer (`15`) | Nooit een buildnummer |
| Apparaatklasse | bijv. `iphone`, `mac-laptop`, `android-tablet` | Nooit een apparaatmodel |
| App-versie + taal | bijv. `1.4.0`, `nl` | Alleen de taal, nooit een volledige locale/regio |
| Accountvorm | Hoeveel accounts (in groepen: `0`, `1`, `2`, `3–5`, `6+`) en welke soorten protocollen | Nooit welke providers, hosts of adressen |
| Gebeurtenissen | App geopend; accountinstelling gestart/voltooid/mislukt en synchronisatie voltooid/mislukt (per protocolsoort); een functie is gebruikt (uit een vaste lijst); welke instellingen aanstaan | Nooit iets wat je als tekst hebt getypt of ingesteld |

**Wat er nooit wordt verstuurd, door de bouw en niet alleen op papier:** de inhoud van berichten,
onderwerpen, afzenders, ontvangers, e-mailadressen, mapnamen, aantallen berichten, zoekopdrachten,
agendatitels of afspraakdetails, namen van bijlagen, servernamen, apparaatmodellen. Elk veld
hierboven is een label uit een vaste lijst of een groep; er is geen enkel veld in de payload dat je
inhoud *zou kunnen* dragen, en onze server weigert alles wat buiten die vaste lijst valt.

**Waar het heen gaat en hoe lang het blijft:** alleen naar servers die Allodia zelf in de EU
beheert (Hetzner Online GmbH, Industriestraße 25, 91710 Gunzenhausen, Duitsland).
Er komt nooit een analysebedrijf van derden aan te pas. Je IP-adres is technisch gezien zichtbaar
wanneer je apparaat verbinding maakt, zoals bij elke internetverbinding, maar het wordt **niet
opgeslagen** en niet aan de gebruiksgegevens gekoppeld. Gebruiksgebeurtenissen worden maximaal 24
maanden bewaard, waarna ze worden verwijderd of teruggebracht tot geaggregeerde statistieken die
niet meer naar een installatie-id te herleiden zijn.

**Wat het is:** pseudonieme gebruiksgegevens. Het installatie-id bevat niet je naam, je adres of
iets anders over jou, maar het blijft gelijk zolang je het aan laat staan (dat is precies wat ons
laat zien of het instellen lukt en of mensen de app blijven gebruiken), dus behandelen we het als
persoonsgegeven onder de AVG in plaats van het anoniem te noemen.

**Waar het voor is:** productbeslissingen en verder niets, zoals welke platforms en talen
aandacht nodig hebben, of accounts instellen en synchroniseren lukt, welke functies
daadwerkelijk gebruikt worden, en of een update iets verslechterd heeft.

**Intrekken:** Instellingen → Privacy, één schakelaar, op elk moment. De app verwijdert het
installatie-id en de vastlegging van je toestemming van je apparaat, en draagt onze server op alles
te wissen wat onder dat id is opgeslagen (AVG art. 17). Intrekken is precies even eenvoudig als
toestemming geven was.

**Willen we ooit méér versturen,** dan behandelt de app je eerdere toestemming als niet-dekkend en
vraagt het je opnieuw, waarbij het je de nieuwe gegevens laat zien voordat er iets verandert. Een
payload kan nooit groeien onder een toestemming die voor minder is gegeven.

Grondslag: jouw toestemming (AVG art. 6(1)(a); opslaan en uitlezen op je apparaat volgens de
nationale implementaties van artikel 5(3) van de ePrivacyrichtlijn).

## 7. Toegang voor AI-assistenten (standaard uit)

De app kan een **AI-assistent op je eigen computer** je e-mail laten lezen en bewerken, via het
Model Context Protocol (MCP). Dit staat uit tenzij je het aanzet bij **Instellingen → Geavanceerd**,
en het bestaat alleen op macOS en Windows.

Wat dit concreet is:

- Een privékanaal **op je eigen apparaat**. De app opent een lokale socket die alleen je eigen
  gebruikersaccount van het besturingssysteem kan bereiken. Geen netwerkpoort, geen server van
  ons, en niets in te stellen met een wachtwoord of sleutel.
- **Er gaat niets naar Allodia.** Wij zitten helemaal niet in dit pad. We weten niet of je het hebt
  aangezet, welke assistent je hebt verbonden, of wat je hebt gevraagd.
- **Jij kiest welke accounts hij mag gebruiken.** Het aanzetten van de functie geeft toegang tot
  *geen enkele* mailbox. Elk account is een apart vinkje, en het uitvinken werkt meteen.
- Het programma dat je verbindt **kan de e-mail lezen en bewerken** van de accounts die je hebt
  aangevinkt: berichten opsommen en doorzoeken, één bericht volledig lezen, als gelezen markeren,
  markeren met een vlag, archiveren, naar de prullenbak verplaatsen, als spam markeren, en een
  vooringevuld concept openen dat jij nakijkt en verstuurt. Definitief verwijderen kan hij niet.
- **Versturen is een aparte schakelaar**, standaard uit. Staat die uit, dan kan een assistent
  alleen een concept openen dat jij zelf leest en verstuurt. Staat die aan, dan weigert de app
  bovendien te versturen naar iemand aan wie je nooit eerder hebt gemaild, tenzij je *dat* ook
  uitzet.

Wat je moet weten voordat je dit aanzet: alles wat de assistent leest, kan zijn eigen aanbieder
ook ontvangen, onder **hun** privacybeleid, niet het onze. Is het een cloud-assistent, dan verlaat
je berichtinhoud je apparaat op het moment dat je hem vraagt iets te lezen, omdat jij daarom hebt
gevraagd. Dat is dezelfde vorm als een bericht doorsturen of een externe afbeelding laden: jouw
verzending, naar een partij die jij hebt gekozen. Het is de moeite waard om bewust te kiezen welke
accounts je aanvinkt.

## 8. Als je contact met ons opneemt

Als je ons mailt (bijvoorbeeld naar **info@allodia.eu**), verwerken we wat je stuurt, namelijk
je adres, je bericht en alles wat je meestuurt, uitsluitend om je te antwoorden en het probleem op te
lossen. Bij support kan een bijlage het diagnostische logboek van de app zijn, dat door zijn opzet
geen berichtinhoud bevat (§2).

Onze mailboxen worden gehost door **Soverin (Nederland)**, onze EU-e-mailprovider.
Supportcorrespondentie wordt 24 maanden na het laatste bericht bewaard en daarna verwijderd.

Grondslag: ons gerechtvaardigd belang om de mensen die ons schrijven te antwoorden (art. 6(1)(f)),
of stappen voorafgaand aan een overeenkomst wanneer je naar een aankoop informeert (art. 6(1)(b)).

## 9. Appstores en updates

Je installeert en werkt de app bij via Apple's App Store, Google Play of de Microsoft Store. De
stores verwerken je aankoop- en apparaatgegevens als **zelfstandige verwerkingsverantwoordelijken**
onder hun eigen privacybeleid, niet namens ons. Wat wij van hen ontvangen zijn geaggregeerde
dashboards (installaties, actieve apparaten, versies van besturingssystemen, apparaatmodellen,
crashstatistieken) die jou voor ons niet identificeren; of jouw apparaat daaraan bijdraagt, wordt
bepaald door de deelinstellingen van je besturingssysteem. De app zelf bevat geen crashrapportage
en doet zelf geen updatecontroles.

## 10. De website allodia.eu

Onze website gebruikt alleen cookies en lokale opslag die nodig zijn om te functioneren
(taalvoorkeur, beveiliging, sessiebeheer), en geen analyse- of marketingcookies. Mocht dat ooit
veranderen, dan vragen we het eerst. Gebruik je het contactformulier, dan verwerken we je naam,
e-mailadres, optionele bedrijfsnaam en bericht om je te antwoorden, net als in §8. De site wordt
gehost in Nederland en Duitsland.

## 11. Wat we nooit doen

- We verkopen of verhuren nooit persoonsgegevens, en delen ze nooit voor advertenties.
- We gebruiken nooit analyse-, tracking- of advertentie-SDK's van derden in de app.
- We nemen nooit geautomatiseerde besluiten over je en stellen nooit een profiel van je op (AVG
  art. 22).
- We trainen nooit AI-modellen op je e-mail. We hebben überhaupt geen toegang tot je e-mail.
- We dragen de persoonsgegevens die we hebben nooit over buiten de EU/EER. (Waar **je eigen
  provider** zich bevindt, is jouw keuze en een rechtstreekse relatie tussen jou en hen. Dat geldt
  ook voor Apple of Google kiezen om in te loggen op een Allodia-account, een route die je gewoon
  niet hoeft te nemen, §3.)

## 12. Bewaartermijnen in één oogopslag

| Gegevens | Bewaard | Waar |
|---|---|---|
| Je e-mail, afspraken, contacten, instellingen, handtekeningen, diagnostisch logboek | Op je apparaat, onder jouw controle; verwijder de gegevens van de app of de app zelf en het is weg | Je apparaat |
| Je Allodia-account en de lijst met e-mailaccounts die het gelijk houdt, als je er een aanmaakt | Tot je ons vraagt het te verwijderen | Servers beheerd door Allodia, EU |
| Gebruiksstatistieken (opt-in) | Tot je ze intrekt, maximaal 24 maanden | Servers beheerd door Allodia, EU |
| Supportcorrespondentie | 24 maanden na het laatste bericht | Soverin, NL |
| Berichten via het contactformulier | Als supportcorrespondentie | Soverin, NL |
| Facturen/administratie, als je iets bij ons koopt | Zolang de Nederlandse belastingwet vereist (7 jaar) | Administratie van Allodia, EU |

## 13. Je rechten

Onder de AVG kun je ons vragen om inzage in, correctie of verwijdering van je persoonsgegevens of
een kopie ervan (overdraagbaarheid), ons vragen de verwerking te beperken, bezwaar maken tegen
verwerking op basis van gerechtvaardigd belang, en elke toestemming op elk moment intrekken (wat
niets afdoet aan wat daarvóór rechtmatig was).

In de praktijk is de snelste route meestal de app zelf: je e-mail en instellingen zijn al in jouw
handen, en de schakelaar voor gebruiksstatistieken (Instellingen → Privacy) is het middel om die
ene dataset die wij over de app hebben in te trekken én te laten wissen. Eén eerlijke beperking:
een installatie-id vertelt ons niet wie je bent, dus we kunnen dat van jou niet opzoeken op je naam
of e-mailadres. De schakelaar in de app, die aantoont dat je het apparaat beheert waar het id bij
hoort, is de betrouwbare manier om die gegevens te laten wissen.

Voor al het overige: **info@allodia.eu**. We reageren binnen een maand (art. 12(3)). Je kunt ook
een klacht indienen bij een toezichthouder. Die van ons is de Nederlandse **Autoriteit
Persoonsgegevens** (autoriteitpersoonsgegevens.nl), maar je mag de toezichthouder van je eigen
EU/EER-land gebruiken.

## 14. Beveiliging

Inloggegevens worden alleen opgeslagen in de beveiligde sleutelopslag van je apparaat en worden
nooit weggeschreven naar de database of de logboeken van de app. Verbindingen met je provider
gebruiken de versleutelde protocollen die hij aanbiedt (TLS). Binnenkomende HTML wordt opgeschoond
in een geharde weergavecomponent waarin scripts nooit draaien en externe inhoud standaard wordt
geblokkeerd. Het weinige dat we serverzijdig beheren, draait op EU-infrastructuur met toegang
beperkt tot wie die nodig heeft, beschermd met meerfactorauthenticatie en versleuteling.

## 15. Wijzigingen in dit beleid

Veranderen we welke gegevens worden verwerkt, dan werken we dit beleid **vóór** die verandering
bij en verhogen we het versienummer en de datum bovenaan. Waar de verandering zou verbreden wat
de app verstuurt, vraagt de app je opnieuw om toestemming in plaats van die aan te nemen. De
actuele versie staat altijd op **https://allodia.eu/privacy/mail-calendar**.

## 16. Contact

Allodia · KvK 56789823 · Kamerlingh Onnesweg 2, 3316GL Dordrecht, Nederland · **info@allodia.eu**
