# Specifica funzionale: il combattimento di Sora (KH2 Final Mix, forma base)

Questo documento dice **come si comporta** il combattimento, non come è fatto il programma del
gioco. Chi scrive la libreria parte da qui, da `INTERFACCIA.md` e dai dati in `data\`.

Ogni regola ha una fonte:

- `[dati]` valore letto dai file di dati del gioco (finisce nel catalogo, vedi `INTERFACCIA.md`);
- `[registrazione]` misurato su registrazioni del gioco vero, fotogramma per fotogramma;
- `[community]` documentazione pubblica dei giocatori (wiki, descrizione dei formati);
- `[studio del programma]` comportamento ricavato studiando il gioco, qui descritto solo come
  comportamento;
- `[scelta nostra]` decisione di progetto della libreria, dove il gioco non dice niente o non
  è stato possibile misurarlo.

Versione del gioco usata per dati e misure: Steam, build 1.0.0.10.

## 1. Cosa copre e cosa no

La libreria riproduce, per Sora in forma base:

- la combo a terra e in aria: quando una pressione vale, quale colpo parte, quanto è lunga;
- tutte le abilità che aggiungono colpi al repertorio e quelle che cambiano i limiti;
- i colpi "in volo" (da terra verso un bersaglio in alto, e in aria verso il bersaglio);
- il movimento verticale: salto, balzi dei colpi, caduta, atterraggio; e lo slancio
  orizzontale dei colpi in volo;
- parata, parata riuscita, contrattacco (Counterguard), colpo respinto, sbalzo, colpo di
  ritorno (Retaliating Slash), ritorno in equilibrio (Aerial Recovery), tratti "non colpibile".

La libreria **non** fa (sezione 12): lettura dei tasti, grafica, animazioni, movimento a terra,
collisioni, nemici, calcolo del danno, magie, schivata, forme, comandi di reazione.

## 2. Tempo, unità, assi

- Il tempo si conta in **tick da 1/60 di secondo** `[registrazione]`. La libreria avanza di un
  tick per chiamata; chi la usa la chiama 60 volte al secondo. Se il gioco va al rallentatore,
  è chi usa la libreria a chiamarla più di rado.
- Le distanze sono in **unità di KH2**. Per orientarsi: Sora corre a 8 unità per tick, un
  salto pieno è alto 185 unità `[registrazione]`.
- Una posizione è (x, altezza, z), con l'altezza che cresce verso l'alto. Una direzione
  orizzontale è (x, z) di lunghezza 1.
- I **frame** di un'animazione si contano da 1.

## 3. I dati

Tutto ciò che è specifico di Sora sta nel catalogo, non nella libreria: la libreria non
contiene nessun colpo scritto a mano. Il catalogo ha quattro tipi di informazione `[dati]`:

1. **Il repertorio**: un elenco ordinato di voci. Ogni voce è un colpo (un'animazione) con le
   condizioni in cui può partire: a terra o in aria, normale o finale, contro chi si valuta,
   distanza, dislivello, angolo, abilità richiesta, valori di salto e di velocità.
   **L'ordine delle voci è l'ordine di priorità.**
2. **Le animazioni**: per ognuna la durata in frame e gli eventi, cioè tratti di frame con un
   significato. Gli eventi del personaggio e quelli dell'arma sono tenuti distinti.
3. **I colpi a segno**: per ogni zona citata dagli eventi, i suoi numeri (potenza, tipo...).
4. **I nomi delle abilità** che il repertorio richiede.

Gli eventi che le regole usano:

| Evento | Di chi | Significato |
|---|---|---|
| a terra | personaggio | in quei frame l'animazione dichiara il personaggio fermo a terra: è "neutro" |
| in aria | personaggio | l'animazione lo dichiara libero in aria: è "neutro" |
| ritorno alla posa di riposo | personaggio | il colpo è finito e il corpo torna in posa: è "neutro" |
| zona | arma o personaggio | in quei frame c'è una zona attiva; il primo numero che la accompagna dice quale colpo a segno è |
| combo permessa | personaggio | da quei frame una pressione può far proseguire la combo |
| finale permesso | personaggio | da quei frame un finale può concatenarne un altro |
| comando di reazione permesso | personaggio | in quei frame si può far partire un comando di reazione |
| non colpibile | personaggio | in quei frame il personaggio non può essere colpito |

Un evento "da A a B" vale per i frame da A a B **compresi**; se B manca (nel catalogo: un
numero negativo) vale da A fino alla fine dell'animazione `[dati]`.

Una **zona** può essere di danno oppure di **parata**: lo dice il tipo del colpo a segno a cui
rimanda (tipi "parata" e "parata totale"). Una zona di parata non fa danno: ferma i colpi in
arrivo `[dati]`. Zone di danno e zone di parata possono coprire **gli stessi frame**: alcuni
colpi parano mentre colpiscono (per Sora, tra gli altri, Guard Break, Vicinity Break, Flash
Step, Finishing Leap, Explosion, Counterguard) `[dati]`.

## 4. Lo scorrere di un'animazione

Vale per ogni animazione che la libreria fa scorrere: i colpi e le reazioni (sezione 10).

| Regola | Fonte |
|---|---|
| Nel tick in cui parte, l'animazione è già al suo frame 1. | `[registrazione]` |
| A ogni tick avanza di un frame. | `[registrazione]` |
| **L'ultimo frame non si vede mai**: un'animazione da N frame occupa N−1 tick; nel tick successivo il personaggio è già libero. Un colpo da 48 frame occupa 47 tick. | `[registrazione]` |
| Una pressione che arriva nel tick esatto in cui l'animazione è finita trova il personaggio libero: apre subito una combo nuova. | `[registrazione]` |
| In uno stesso frame possono essere attive insieme una **zona di danno** e una **parata**. | `[dati]` |
| La zona di danno attiva è, tra le zone di danno che coprono quel frame, quella con la potenza più alta; a parità, la prima cercando prima tra gli eventi dell'arma e poi tra quelli del personaggio. Le altre zone di danno sovrapposte non si vedono. | `[scelta nostra]`: la libreria mostra una sola zona di danno alla volta |
| La parata è attiva se almeno una zona di parata copre quel frame. | `[dati]` |
| Un colpo può avere più zone una dopo l'altra (l'affondo ne ha due). | `[dati]` |

Tempi dei cinque colpi a terra di base, come esempio `[dati]`, confermati `[registrazione]`:

| Colpo | Frame | Tick occupati | Si prosegue dal frame |
|---|---|---|---|
| A (primo colpo) | 48 | 47 | 23 |
| B (nemico di lato) | 56 | 55 | 30 |
| C (affondo) | 72 | 71 | 44 |
| Finale 1 | 80 | 79 | non ha "combo permessa": ha solo "finale permesso", che serve con Finishing Plus |
| Finale 2 | 80 | 79 | come il finale 1 |

Il frame "si prosegue da" è compreso: per il colpo A una pressione al frame 22 è ignorata, al
23 è accettata `[registrazione]`.

## 5. La combo: quando una pressione vale

Una "pressione" è il momento in cui il tasto Attacco passa da rilasciato a premuto. Tenerlo
premuto non conta. Non esiste memoria delle pressioni: una pressione che non vale è persa
`[studio del programma]` `[registrazione]`.

Due contatori accompagnano la combo `[studio del programma]`, visibili nelle registrazioni:

- i **passi**: 1 al primo colpo, +1 a ogni prosecuzione;
- i **finali**: 1 all'inizio, +1 quando un finale ne concatena un altro.

Nessuno dei due supera 100.

"**Ha toccato**" vuol dire: il colpo in corso ha fatto danno a qualcosa, oppure l'arma è stata
respinta da una parata nemica. Che un colpo abbia toccato lo dice chi usa la libreria. Ogni
colpo comincia con "ha toccato" falso. Con l'abilità **Combo Master** vale sempre come se
avesse toccato.

"**Neutro**": l'animazione in corso, in quel frame, ha attivo uno dei tre eventi "a terra",
"in aria", "ritorno alla posa di riposo".

Quando arriva una pressione si guarda la prima riga che corrisponde:

| # | Situazione | Effetto |
|---|---|---|
| 1 | Nessuna animazione in corso | Vale: combo nuova, passi = 1 e finali = 1 |
| 2 | Animazione nella finestra "finale permesso", ha toccato, e i finali fatti sono meno del massimo | Vale: passi +1 e finali +1 (finale concatenato) |
| 3 | Animazione nella finestra "combo permessa" e ha toccato | Vale: passi +1 |
| 4 | Animazione neutra in quel frame | Vale: combo nuova, passi = 1 e finali = 1 |
| 5 | Altrimenti | Ignorata |

Conseguenze, tutte viste nelle registrazioni:

- i colpi a terra di base non si dichiarano mai neutri: dopo un colpo a vuoto bisogna
  aspettare che finisca;
- dopo un colpo aereo viene la caduta, che è neutra: una pressione apre una combo nuova;
- il contatore dei finali parte da 1: con un massimo di 1 la riga 2 non scatta mai, e dopo il
  finale la combo è finita.

Se la pressione vale ma poi nessun colpo può partire (succede solo con i comandi, sezione 8),
i contatori tornano com'erano.

### I limiti e le abilità che li cambiano

| | Senza abilità | Fonte |
|---|---|---|
| Passi massimi a terra | 3 | `[registrazione]` |
| Passi massimi in aria | 3 | `[registrazione]` |
| Finali massimi | 1 | `[registrazione]` |

| Abilità | Effetto | Fonte |
|---|---|---|
| Combo Plus (se ne possono avere più copie) | +1 passo a terra per copia | `[community]` |
| Air Combo Plus (più copie) | +1 passo in aria per copia | `[community]` |
| Negative Combo | −1 passo a terra e −1 in aria; mai sotto 1 | `[community]` |
| Finishing Plus (più copie) | +1 finale massimo per copia | `[community]` |
| Combo Master | il colpo conta sempre come "ha toccato" | `[studio del programma]` |
| Berserk Charge | finché la sua condizione è vera il finale non arriva mai (sezione 6) | `[studio del programma]` |

Quando la condizione di Berserk Charge sia vera lo dice chi usa la libreria (nel gioco è
legata alla ricarica dei PM, non verificato).

## 6. La scelta del colpo

Quando una pressione vale, il colpo che parte si sceglie in tre passi: quali voci sono
disponibili, quali diventano candidate guardando i nemici, quale vince. `[studio del
programma]`, con i valori dal catalogo `[dati]`. Verificato rigiocando le registrazioni (per i
colpi senza abilità) e facendo uscire ogni voce del catalogo nella sua situazione.

### 6.1 Disponibilità

I contatori usati qui sono quelli **già aggiornati** dalla pressione (sezione 5): alla terza
pressione di una combo i passi valgono 3.

"Limite" è il limite dei passi a terra se il personaggio è a terra, in aria se è in aria.
"Nella finestra finale" vuol dire che l'animazione in corso, al momento della pressione, era
nella finestra "finale permesso".

Una voce è disponibile se valgono **tutte**:

| Condizione | |
|---|---|
| Ha un'animazione | il suo numero di animazione non è zero |
| Abilità | non ne richiede, oppure quella richiesta è equipaggiata |
| Terra o aria | è una voce "in aria" e il personaggio è in aria, oppure è una voce "a terra" e il personaggio è a terra |
| Se è un **finale** | è nella finestra finale, **oppure** passi + anticipo della voce ≥ limite |
| Se è un colpo **normale** | sempre, **tranne** quando valgono insieme: finali > 1, e (è nella finestra finale oppure passi ≥ limite) |

Con Berserk Charge equipaggiata e la sua condizione vera: nessun finale è disponibile, e i
colpi normali lo sono sempre.

L'"anticipo" è un valore della voce `[dati]`: Finishing Leap ha anticipo 1, quindi è
disponibile un passo prima degli altri finali.

Al passo del finale sono disponibili sia i finali sia i normali: esce il finale solo perché
nel repertorio i finali sono scritti prima.

### 6.2 Candidate

Ogni voce ha un tipo `[dati]`:

| Tipo | Si valuta contro | Diventa candidata se |
|---|---|---|
| 1 | il bersaglio principale | è disponibile e il bersaglio principale soddisfa distanza, dislivello e angolo |
| 2 | ogni nemico nei dintorni (principale compreso) | è disponibile e almeno un nemico soddisfa distanza, dislivello e angolo |
| 3 | nessuno | è disponibile |
| 0 | nessuno | mai: la richiama un comando (sezione 8) |

Le tre condizioni, per un nemico:

- **Distanza**: distanza orizzontale tra i centri, meno il raggio del nemico, mai sotto zero.
  Deve valere: vicino ≤ distanza < lontano.
- **Dislivello**: quanto il punto di mira del nemico sta più in alto dei piedi del
  personaggio. Deve cadere tra un minimo (compreso) e un massimo (escluso). I due limiti della
  voce vengono allargati dalla **taglia** del nemico, e due interruttori della voce possono
  invertire il verso della correzione su ciascun limite; un terzo interruttore fa crescere il
  limite alto con una dote del personaggio che oggi vale sempre zero. Il conto esatto è in
  `INTERFACCIA.md`, "Dislivello".
- **Angolo**: il coseno dell'angolo, in orizzontale, tra la direzione verso il nemico e la
  **direzione di riferimento** deve essere maggiore del minimo della voce e non maggiore del
  massimo (minimo escluso, massimo compreso). La direzione di riferimento è quella verso il
  bersaglio principale; se non c'è un bersaglio principale (o sta esattamente sopra il
  personaggio) è la direzione in cui il personaggio guarda. Un nemico esattamente sopra il
  personaggio ha coseno 1.

Per farsi un'idea, con i dati di Sora: le voci di tipo 1 chiedono "proprio il bersaglio"
(coseno sopra 0,99996); il colpo B e il finale 1 chiedono un nemico di lato, tra circa 22 e
135 gradi; Vicinity Break ne chiede uno di fianco o dietro.

**Il bordo.** A terra, se chi usa la libreria dice che subito davanti al personaggio il
terreno manca, diventa candidata anche la voce numero 1 del repertorio (un colpo in salto),
senza guardare la sua disponibilità `[studio del programma]`, significato non verificato.

### 6.3 Quale vince

1. Si scorre il repertorio nel suo ordine e si prende la **prima candidata**.
2. **Mai la stessa animazione due volte di fila.** Una candidata che userebbe l'animazione già
   in corso viene saltata, a meno che indichi sé stessa come propria alternativa. Della
   **prima** saltata che ha un'alternativa (una voce esistente) ci si ricorda l'alternativa.
   Se scorrendo non si trova nessun'altra candidata, parte quell'alternativa, **senza
   controllare** se sarebbe stata disponibile o candidata.
3. Se non c'è nessuna candidata e nessuna alternativa: si aggiunge alle candidate la voce di
   **ripiego** (la numero 5 a terra, la numero 9 in aria) e si ripete la scelta dal punto 1.
   Se nemmeno così esce una voce (il ripiego ripeterebbe l'animazione in corso e non ha
   un'alternativa), parte lo stesso il ripiego `[scelta nostra]`. Non parte niente solo se la
   voce di ripiego non esiste.

"L'animazione in corso" è quella che sta scorrendo al momento della pressione, anche se è una
reazione; se non ce n'è nessuna, niente viene saltato.

Con i dati di Sora ne viene, per esempio `[registrazione]`: senza bersagli il primo colpo è A
(ripiego); il secondo non può essere di nuovo A, quindi parte la sua alternativa, l'affondo C;
il terzo è un finale. Dopo un A come primo colpo nelle registrazioni è sempre arrivato C.

## 7. I colpi "in volo"

Una voce con un valore di **salto** maggiore di zero `[dati]` stacca il personaggio da terra o
lo porta in volo verso il bersaglio. Sono tutti i colpi aerei e alcuni colpi "a terra" (quelli
verso un bersaglio in alto).

| Regola | Fonte |
|---|---|
| Se un colpo in volo parte mentre il personaggio è **a terra**, i contatori ripartono (passi = 1, finali = 1): è il primo colpo di una combo aerea. | `[studio del programma]`, coerente con le registrazioni |
| Quando parte, il personaggio comincia un **balzo** verso l'alto dall'altezza a cui si trova (sezione 9) e riceve uno **slancio** orizzontale (sotto). | `[registrazione]` |
| I colpi senza valore di salto non danno né balzo né slancio: il loro avanzamento è nell'animazione, e non riguarda la libreria. | `[dati]` |

### Quanto sale

| Caso | Salita | Fonte |
|---|---|---|
| Nessun bersaglio principale | il valore di salto della voce | `[registrazione]`: 21 per il primo colpo aereo, 17 per il secondo |
| Voce "in aria", con bersaglio | quanto il punto di mira del bersaglio sta più in alto dei piedi, portato tra il salto minimo e il salto massimo della voce | `[registrazione]`: misurato 67,0 con il bersaglio molto più in alto e massimo 67 |
| Voce "a terra" che guarda il dislivello (i due limiti di dislivello scritti nella voce sono diversi), con bersaglio | quanto il punto di mira sta più in alto, portato tra il minimo e il massimo di dislivello della voce (gli stessi della sezione 6.2, con la taglia del bersaglio e senza la dote) | `[registrazione]`, pochi campioni: bersaglio a 132,7 → sale 132; a 148,9 → 148,8; a 367,7 → 253 |
| Voce "a terra" che non guarda il dislivello (i due limiti scritti nella voce sono uguali: è il caso del colpo del bordo), con bersaglio | come una voce "in aria": tra il salto minimo e il salto massimo | `[scelta nostra]` |

Se il minimo risulta maggiore del massimo (per una voce a terra o in aria), la salita è
quanto il bersaglio sta più in alto, ma almeno il salto minimo `[scelta nostra]`. "Salto
minimo" è sempre il valore di salto minimo scritto nella voce, anche per le voci a terra.

### Lo slancio orizzontale

| Regola | Fonte |
|---|---|
| Lo slancio va in linea retta nella direzione che il bersaglio principale aveva quando il colpo è partito; senza bersaglio (o con il bersaglio esattamente sopra il personaggio), dove guarda il personaggio. | `[registrazione]` |
| Parte a una velocità (unità per tick) e **perde un ventesimo ogni 2 tick**; il primo calo avviene subito. Partendo da 8, i primi tick valgono 7,6 · 7,6 · 7,22 · 7,22 · 6,86... | `[registrazione]` |
| Senza bersaglio la velocità di partenza è la velocità massima della voce. | `[registrazione]`, due campioni |
| Con un bersaglio è la velocità che, calando come cala, porterebbe il personaggio al bordo del bersaglio (distanza tra i centri meno il raggio, mai sotto zero) **nel momento in cui finisce la salita del balzo**; poi viene portata tra la velocità minima e la massima della voce. Se la salita è nulla (non dura nemmeno un tick) quella velocità non esiste: allora, prima di portarla tra minimo e massimo, vale la velocità massima se c'è strada da fare, zero se il personaggio è già al bordo del bersaglio. | `[scelta nostra]` che torna a grandi linee con le registrazioni |
| La velocità di partenza non è mai sotto il minimo della voce né sopra il massimo; se il minimo fosse maggiore del massimo vale il più grande dei due. | `[scelta nostra]` |
| Lo slancio dura quanto il colpo che lo ha dato: finisce quando quel colpo finisce o viene sostituito. | `[scelta nostra]` |

Quanta strada fa uno slancio partito a velocità v in un numero pari di tick n: la somma di
n/2 coppie di tick, cioè 2·v·(0,95 + 0,95² + ... + 0,95^(n/2)), che è lo stesso di
38·v·(1 − 0,95^(n/2)). Questa seconda forma si usa anche per un tempo t non intero (la durata
di una salita di solito non lo è): strada = 38·v·(1 − 0,95^(t/2)). Con v = 8: circa 155 unità in
28 tick, mai più di 304 `[registrazione]`: 28 colpi aerei registrati su 30, con il bersaglio a
più di 150 unità, fanno la stessa strada entro l'8%.

## 8. Comandi che richiamano una voce precisa

Le voci di tipo 0 non entrano mai nella scelta: partono quando chi usa la libreria le
richiama con il loro numero. Un comando segue **le stesse regole di una pressione** (sezione
5: finestre, "ha toccato", contatori) `[scelta nostra]`, e la voce richiamata parte solo se è
disponibile (sezione 6.1); se non lo è, il comando non ha effetto e i contatori tornano
com'erano.

Voci di tipo 0 nel catalogo di Sora `[dati]`: 11 parata (Guard), 12 Upper Slash, 37 Finishing
Leap, 38 Horizontal Slash, 39 Counterguard, 40 Retaliating Slash, 1 il colpo del bordo.

Due voci le richiama **la situazione**, non un tasto dedicato `[studio del programma]`:

| Situazione | Cosa succede a una pressione di Attacco |
|---|---|
| Il personaggio è nell'animazione di **parata riuscita** | parte la voce 39 (Counterguard), se è disponibile |
| Il personaggio è **sbalzato in aria** (sezione 10) | parte la voce 40 (Retaliating Slash), se è disponibile |

In questi due casi, se il personaggio non è occupato, il colpo parte **subito**, senza
aspettare finestre e senza la regola "ha toccato"; i passi salgono di 1 e i finali valgono
almeno 1. Lo stesso vale se arriva
esplicitamente il comando con quel numero. Un comando con un altro numero, senza Attacco,
non richiama la voce della situazione: viene giudicato come un comando qualsiasi. Fuori da
queste due situazioni i comandi 39 e 40 sono comandi qualsiasi. Se la voce non è disponibile
(abilità non equipaggiata), la pressione viene giudicata come una pressione qualsiasi.

Se valgono tutte e due le situazioni insieme, ognuna si guarda per conto suo:

| In quel tick | Voce che si prova a far partire |
|---|---|
| Attacco (con o senza un comando) | la 40, quella dello sbalzo. Se non può partire la pressione viene giudicata come una pressione qualsiasi: la 39 non viene provata |
| comando 40, senza Attacco | la 40 |
| comando 39, senza Attacco | la 39, quella della parata riuscita |
| un altro comando, senza Attacco | nessuna delle due: è un comando qualsiasi |

Per Retaliating Slash la documentazione dei giocatori indica un tasto diverso da Attacco
`[community]`: per questo la libreria accetta sia Attacco sia il comando esplicito.

## 9. Movimento verticale

Tutto `[registrazione]`: 45 salti e le combo aeree, altezza a ogni tick. Nei dati del gioco
c'è solo quanto deve salire ogni colpo; come si sale e si cade è misurato.

### La salita

Una salita di altezza H, partita dall'altezza B, dura **28 × radice(H / 185) tick**. Dopo t
tick dall'inizio (t = 1 al primo tick) l'altezza è B + H × F(t / durata), dove F è la curva
qui sotto. Tra un punto e l'altro si va in linea retta.

| Frazione della durata | 0 | 1/14 | 2/14 | 3/14 | 4/14 | 5/14 | 6/14 | 7/14 |
|---|---|---|---|---|---|---|---|---|
| Frazione dell'altezza | 0 | 0,1627 | 0,3195 | 0,4649 | 0,5946 | 0,7054 | 0,7957 | 0,8665 |

| Frazione della durata | 8/14 | 9/14 | 10/14 | 11/14 | 12/14 | 13/14 | 1 |
|---|---|---|---|---|---|---|---|
| Frazione dell'altezza | 0,9184 | 0,9546 | 0,9773 | 0,9903 | 0,9968 | 0,9995 | 1 |

Una sola curva vale per tutte le salite: il salto (185), i balzi dei colpi aerei (17, 21, 67)
e i colpi da terra verso l'alto (130–250) tornano entro mezza unità.

Finita la salita il personaggio resta **sospeso** all'altezza B + H fino al tick
30,12 × radice(H / 185), poi cade. Al primo tick di caduta conta la frazione di tick
trascorsa oltre quell'istante: se ne sono passati f (di solito meno di uno), il personaggio è
sceso di 0,204 × f² e ha velocità di caduta 0,408 × f.

### La caduta

Accelerazione costante: **0,408 unità per tick al quadrato**. A ogni tick l'altezza cambia
della velocità che aveva, meno 0,204; poi la velocità di caduta cresce di 0,408. Non c'è
una velocità massima misurata. Quando l'altezza arriva al terreno o sotto, il personaggio è a
terra, esattamente all'altezza del terreno.

### Il salto

| Regola | |
|---|---|
| Altezza | un salto è una salita di 185 unità |
| Tasto tenuto | finché il tasto resta giù la salita continua fino in fondo |
| Tasto rilasciato | la salita si interrompe al primo tick in cui il tasto non è più giù, **ma non prima che siano passati 7 tick di salita**: al più presto, l'ottavo tick è il primo di caduta. Un tocco dà quindi un salto con l'apice a 103 unità circa |
| Dopo il rilascio | il personaggio passa alla caduta con una velocità verso l'alto pari a quanto la curva lo avrebbe fatto salire in quel tick, ma al più 2,04 unità per tick; in quello stesso tick fa già il primo passo di caduta |
| Rilascio a salita finita | durante la sospensione il rilascio non cambia niente |

Il rilascio accorcia solo i salti, mai i balzi dei colpi.

### Il balzo di un colpo

Quando parte un colpo in volo (sezione 7), il personaggio comincia una salita dell'altezza
decisa da quel colpo, dall'altezza a cui si trova in quel momento, qualunque cosa stesse
facendo (salto, caduta, un altro balzo). Se la salita fosse zero o negativa, comincia a cadere
da fermo. Per questo una combo aerea tiene il personaggio quasi alla stessa quota.

### A terra, in aria, atterraggio

| Regola | Fonte |
|---|---|
| Da terra, se il terreno sotto i piedi è più in basso del personaggio (è uscito da un bordo), comincia a cadere da fermo; in quel tick l'altezza non cambia. Altrimenti il personaggio sta all'altezza del terreno. | `[scelta nostra]` |
| Si salta solo da terra e solo quando il personaggio non è impegnato: nessuna animazione in corso, oppure l'animazione in corso è neutra in quel frame (allora il salto la interrompe). | `[registrazione]`: nelle registrazioni un salto non parte mai a metà di un colpo |
| Nei primi frame del salto Attacco non vale: l'animazione del salto dichiara il personaggio "in aria" solo da un certo frame (l'8 per Sora `[dati]`), e fino al frame prima le pressioni sono ignorate perché il personaggio è occupato. Nelle registrazioni il colpo aereo più precoce parte 7 tick dopo l'inizio del salto. | `[dati]` `[registrazione]` |
| Per il resto il personaggio conta come "in aria" dal tick in cui stacca fino a quello in cui tocca terra. | `[scelta nostra]` |
| **Toccare terra interrompe** l'animazione in corso (il colpo aereo finisce lì). | `[registrazione]` |
| Dopo aver toccato terra c'è l'animazione di atterraggio, che non impedisce niente: ci si può muovere, saltare e attaccare subito. | `[registrazione]` |

## 10. Parata, colpo respinto, sbalzo

Numeri di animazione fissi, uguali per ogni personaggio `[dati]`: 3 salto, 4 caduta, 5
atterraggio, 40 ritorno in equilibrio, 41 arma respinta (a terra), 42 parata riuscita, 43 arma
respinta (in aria). I loro eventi stanno nel catalogo come quelli dei colpi.

Le animazioni di questa sezione non sono colpi del repertorio: le chiamo **reazioni**.
Scorrono come le altre (sezione 4) e hanno le loro finestre: le regole della sezione 5 valgono
guardando l'animazione in corso, che sia un colpo o una reazione.

| Cosa succede | Effetto | Fonte |
|---|---|---|
| **Parata** | è la voce 11, un colpo come gli altri richiamato da un comando. In una parte dei suoi frame ha una zona di parata (per Sora dal frame 6 al 37, in un'animazione di 60) | `[dati]` |
| Un colpo nemico arriva mentre è attiva una zona di tipo **parata** | "ha toccato" diventa vero, i passi scendono di 1 (mai sotto zero), e parte la reazione 42, **parata riuscita**. Lì l'arma continua a parare (frame 1–40 per Sora) e poi torna neutra | `[studio del programma]` `[dati]` |
| Un colpo nemico arriva mentre è attiva solo una zona di tipo **parata totale** (capita dentro alcuni colpi e nel ritorno in equilibrio, negli stessi frame in cui il personaggio è non colpibile) | il colpo nemico è fermato e per il personaggio non cambia niente: l'animazione in corso prosegue, "ha toccato" e i passi restano come sono | `[scelta nostra]` |
| Attacco durante la parata riuscita | Counterguard, vedi sezione 8. Senza l'abilità la pressione segue le regole normali: la parata riuscita non ha una finestra "combo permessa", quindi è ignorata finché l'animazione non diventa neutra; da lì apre una combo nuova | `[studio del programma]` `[dati]` |
| Un colpo nemico arriva mentre **non** c'è una zona di parata attiva | la parata non lo ferma (e non succede niente di quanto sopra) | `[dati]` |
| **L'arma viene respinta** da una parata nemica | "ha toccato" diventa vero, i passi scendono di 1 (mai sotto zero), e parte la reazione 41 (a terra) o 43 (in aria). La 41 ha una finestra "combo permessa" (dal frame 16 per Sora): la combo può riprendere da lì, e il colpo respinto non ha contato come passo | `[studio del programma]` `[dati]` `[registrazione]` |
| **Il personaggio subisce un colpo** | la combo si rompe: passi = 0 e finali = 1, "ha toccato" falso, e parte l'animazione di danno indicata da chi usa la libreria. Se il colpo lo ha sbalzato in aria, il personaggio è "sbalzato" | `[registrazione]` per la combo che si rompe; `[scelta nostra]` per i valori |
| Attacco mentre è sbalzato | Retaliating Slash, vedi sezione 8 | `[studio del programma]` |
| Tasto di ritorno in equilibrio (quello del salto) mentre è sbalzato, con l'abilità **Aerial Recovery** | finisce lo sbalzo, passi = 0 e finali = 1, e parte la reazione 40 (non colpibile nei primi 16 frame per Sora, di nuovo libero in aria dal 24) | `[studio del programma]` `[dati]` |

Lo stato "sbalzato" finisce quando: parte un colpo, parte il ritorno in equilibrio, finisce
l'animazione in corso, oppure il personaggio tocca terra.

Se il catalogo non contiene l'animazione di una reazione `[scelta nostra]`:

- parata riuscita e arma respinta: l'animazione in corso prosegue ("ha toccato" e i passi
  cambiano lo stesso);
- colpo subito e ritorno in equilibrio: il personaggio resta libero (l'animazione in corso,
  se c'era, finisce).

**Non colpibile** e **comando di reazione permesso** sono eventi di cui la libreria deve solo
saper dire, tick per tick, se sono attivi. Senza nessuna animazione in corso il personaggio è
colpibile, e un comando di reazione è permesso.

## 11. Tempi esatti, in breve

1. Un colpo o una reazione sono al frame 1 nello stesso tick in cui partono.
2. Un'animazione da N frame occupa N−1 tick; al tick dopo il personaggio è libero.
3. La finestra "combo permessa dal frame K" comprende il frame K.
4. Una pressione viene giudicata sull'animazione **dopo** che è avanzata in quel tick.
5. "Ha toccato" di un tick vale già per la pressione dello stesso tick.
6. Nel tick in cui parte un colpo in volo: il balzo comincia da quel tick (la prima variazione
   di altezza si vede al tick dopo) e il primo passo di slancio si applica in quel tick stesso.
7. Il salto: nel tick stesso in cui parte, il personaggio è già salito del primo passo della
   curva (15 unità circa).
8. Rallentatore: quando il gioco rallenta, rallenta tutto insieme. La libreria non ne sa
   niente: conta tick. Nelle registrazioni i tratti al rallentatore non si confrontano.

## 12. Cosa la libreria non deve fare

- Leggere tasti, disegnare, caricare animazioni o modelli.
- Muovere il personaggio in orizzontale, a parte dire lo slancio dei colpi in volo.
- Decidere se un colpo ha toccato un nemico, o se un colpo nemico ha raggiunto il
  personaggio: lo dice chi la usa.
- Calcolare il danno, la vita, la spinta sui nemici: la libreria dice solo quale zona è
  attiva e con quale potenza.
- Scegliere il bersaglio principale o trovare i nemici nei dintorni.
- Scegliere l'animazione di danno quando il personaggio viene colpito.
- Schivata, magie, forme, evocazioni, comandi di reazione (oltre a dire quando sono permessi),
  livelli, menu.
- Avere dipendenze esterne.

## 13. Da NON riprodurre

Comportamenti o dati che sembrano resti o errori. La libreria non li implementa.

| Cosa | Perché |
|---|---|
| Il **punteggio** delle voci del repertorio | è nei dati, ma la scelta non lo usa: conta solo l'ordine. Nessuna logica deve leggerlo |
| La **correzione della distanza**, il **tempo di sfumatura** e l'**animazione successiva** di una voce | sono nei dati e si conservano per chi li vuole leggere, ma nessuna regola li usa. Il significato della correzione della distanza non è noto |
| La **velocità minima negativa** di alcune voci | farebbe allontanare il personaggio dal bersaglio. In una sola registrazione su settanta si vede qualcosa del genere; la regola non è nota. Lo slancio non va mai all'indietro: una velocità di partenza sotto zero può uscire solo se il massimo della voce è negativo |
| L'altezza che cambia **a scatti ogni 2 tick** | nel gioco la velocità verticale si aggiorna ogni due tick e le posizioni intermedie sono a metà strada. La libreria calcola salita e caduta a ogni tick con le formule della sezione 9 (la differenza è sotto l'unità). Solo lo slancio orizzontale cala ogni 2 tick, come scritto nella sezione 7 |
| Gli eventi "a istante" delle animazioni (suoni, effetti, scie) e tutti gli eventi "a tratto" diversi dagli otto della sezione 3 | si leggono dal catalogo e si conservano, ma non hanno regole |
| I campi dei colpi a segno diversi da potenza e tipo (elemento, reazione, spinta, intervallo, Revenge, gruppo, genere) | si conservano per chi usa la libreria; nessuna regola li usa |
| Righe del catalogo con una parola chiave sconosciuta | si ignorano senza errore |
| La **dote** che allunga la portata verso l'alto | il valore esiste nell'interfaccia, ma la combo lo passa sempre a zero: non si sa da cosa dipenda |
| Una memoria delle pressioni fatte in anticipo | nel gioco non c'è: una pressione ignorata è persa |

## 14. Verifiche attese sulle registrazioni

Le registrazioni in `data\logs\` vengono dal gioco vero. Come si leggono e come si rigiocano è
in `INTERFACCIA.md`. Risultati attesi:

| Verifica | Atteso |
|---|---|
| Decisioni della combo (parte un colpo, pressione ignorata, il colpo finisce), su tutte le registrazioni che hanno le colonne necessarie | **0 differenze**. Combo rigiocate: 128 (36 e 92 nelle due registrazioni utili); prosecuzioni uguali (colpi partiti d'accordo con più di 1 passo): 133 (26 e 107); in più, contate a parte, 8 combo nuove (3 e 5) partite d'accordo con il gioco mentre se ne stava già seguendo una |
| Nella registrazione più lunga, pressioni ignorate | 286 "troppo presto", 22 "non ha toccato", 66 "combo finita" |
| Salti (45), altezza a ogni tick | 36 entro 2 unità, 42 entro 5. I 3 che non tornano finiscono contro un gradino o un nemico |
| Slancio dei colpi aerei con il bersaglio a più di 150 unità (30) | 28 fanno la stessa strada entro l'8% |
