# KH2 Combat Kit

Le regole del combattimento di **Kingdom Hearts II Final Mix** (Sora, forma base), ricostruite
come una piccola libreria in Rust da collegare a un altro gioco.

*English version: [README.md](README.md).*

- **Non contiene file del gioco.** La libreria legge un "catalogo" che ognuno genera dalla
  propria copia del gioco con gli script in `tools/kh2`.
- **Niente grafica, nessuna dipendenza.** La libreria decide solo *cosa succede*: quale
  colpo parte, quando una pressione vale, quanto è alto un salto. A disegnare pensa il tuo
  gioco.
- **Controllata sul gioco vero.** Le regole sono state rigiocate contro registrazioni di KH2
  fotogramma per fotogramma: vedi [Quanto è fedele](#quanto-è-fedele).

È un progetto amatoriale non ufficiale, senza alcun legame con Square Enix o Disney.

## Cosa c'è dentro

| Cartella | Cos'è |
|---|---|
| `src/kh2-combat-core` | La libreria (Rust, nessuna dipendenza). È questa che si integra. |
| `src/first-combo` | Il programma più piccolo che usa la libreria: stampa una combo nel terminale. |
| `src/arena` | Una piccola arena di prova fatta con Bevy: un esempio funzionante di gioco che usa la libreria. |
| `tools/kh2` | Script in Python che leggono la propria copia di KH2 e scrivono il catalogo (e, per l'arena, modello e animazioni di Sora). |
| `docs/SPEC-FUNZIONALE.md`, `docs/INTERFACCIA.md` | I due documenti da cui è stata scritta la libreria: come si comporta il combattimento, e la forma della libreria, del file catalogo e delle registrazioni. |
| `PROVENANCE.md` | Come è stato prodotto il codice della libreria: una riscrittura "clean room" a partire da quei due documenti (in inglese). |

## Cosa copre la libreria

- **Combo a terra e in aria**: il colpo che parte dipende da distanza, altezza e angolo del
  bersaglio, dai nemici attorno, dall'essere in aria e dalle abilità equipaggiate. È la
  tabella di scelta di KH2, non una sequenza fissa.
- **Quando una pressione vale**: finestre di combo, regola "il colpo deve aver toccato
  qualcosa", pressioni ignorate.
- **Lunghezza della combo e finali**: Combo Plus, Air Combo Plus, Negative Combo, Finishing
  Plus, Combo Master, Berserk Charge.
- **Tutte le abilità d'attacco del repertorio base di Sora**: Slide Dash, Flash Step,
  Slapshot, Dodge Slash, Vicinity Break, Guard Break, Explosion, Finishing Leap, Upper Slash,
  Aerial Sweep, Aerial Dive, Aerial Spiral, Horizontal Slash, Aerial Finish, Magnet Burst.
- **Movimento in aria**: salto ad altezza variabile, il piccolo balzo di ogni colpo aereo,
  caduta, atterraggio, e lo slancio in avanti dei colpi aerei.
- **Difesa**: parata, parata riuscita, Counterguard, arma respinta da una parata nemica,
  sbalzo in aria, Retaliating Slash, Aerial Recovery, fotogrammi "non colpibile".

**Non coperti:** Dodge Roll, magie, Drive Form, Limit, evocazioni, comandi di reazione (la
libreria dice solo quando sono permessi), comportamento dei nemici, calcolo del danno (la
formula è descritta nella specifica, il catalogo porta potenza e spinta di ogni colpo),
livelli e menu.

## Cosa serve

- **Rust** 1.85 o più recente (https://rustup.rs).
- **Python** 3.10 o più recente (solo libreria standard), per costruire il catalogo.
- La propria copia di **KINGDOM HEARTS -HD 1.5+2.5 ReMIX-** per PC. Provato con la versione
  Steam, build 1.0.0.10.
- **OpenKH** (https://github.com/OpenKH/OpenKh), usato come programma per aprire gli archivi
  del gioco.
- Windows, perché il gioco e gli strumenti di OpenKH sono programmi per Windows. La libreria
  in sé si compila ovunque si compili Rust.

## Per cominciare

1. Dire agli script dov'è OpenKH (la cartella che contiene `OpenKh.Command.IdxImg.exe`). Se
   il gioco non è nella cartella in cui Steam lo installa di solito, impostare anche
   `KH2_GAME_DIR` sulla cartella del gioco (quella che contiene `Image`).

   ```powershell
   $env:OPENKH_DIR = "D:\tools\OpenKH"
   ```

2. Costruire il catalogo. L'installazione del gioco viene solo letta. Tutto finisce in
   `extracted/`, che Git ignora: **non va mai pubblicata**.

   ```bash
   python tools/kh2/build_catalog.py
   ```

3. Lanciare i test.

   ```bash
   cargo test --manifest-path src/Cargo.toml
   ```

4. Vederla funzionare nel terminale: un personaggio, un nemico fermo e un giocatore che
   preme attacco appena la combo può proseguire. Stampa ciò che la libreria decide, tick per
   tick (il sorgente è `src/first-combo/src/main.rs`, un buon punto da cui cominciare a
   leggere).

   ```bash
   cargo run --manifest-path src/Cargo.toml -p first-combo
   ```

5. Facoltativo: provarla nell'arena. Il primo comando prepara modello e animazioni di Sora
   (serve `OpenKh.Research.Kh2Anim.exe` di OpenKH); senza, l'arena parte lo stesso.

   ```bash
   python tools/kh2/prepare_arena.py
   ```

   ```bash
   cargo run --manifest-path src/Cargo.toml -p arena
   ```

   Tasti: `WASD` muovi, `Spazio` attacca, `K` salta (tenuto premuto salta più in alto), `E`
   colpo speciale (quadrato), `Q` parata, `X` il manichino ti colpisce, frecce spostano il
   manichino, `Pag su`/`Pag giù` lo alzano e lo abbassano, `N` secondo manichino, `F1`/`F2`
   scelgono un'abilità e `F3` la cambia, `1`/`2`/`3` velocità del gioco. Funziona anche il pad.

## Integrarla nel proprio gioco

### Chi fa cosa

| La libreria | Il tuo gioco |
|---|---|
| Sceglie il colpo, ne fa scorrere i fotogrammi, apre e chiude le finestre di danno e di combo | Legge i tasti |
| Decide l'altezza del personaggio (salto, balzi, caduta) | Muove il personaggio a terra e dice alla libreria dov'è il pavimento |
| Dà lo slancio in avanti dei colpi aerei, tick per tick | Somma quello slancio alla posizione |
| Dice quale animazione di KH2 è in corso e a quale fotogramma | Disegna l'animazione |
| Dice quando l'arma può fare danno, e con quale potenza | Controlla se l'arma ha davvero toccato un nemico, e applica il danno |
| Gestisce parata riuscita, rimbalzo, sbalzo e ripresa | Dice alla libreria quando arriva un colpo nemico o quando un nemico para |

Il tempo si conta in **tick da 1/60 di secondo**: `tick` va chiamata esattamente 60 volte al
secondo. Le distanze sono in **unità di KH2** (Sora corre a 8 unità per tick e un salto pieno
è alto 185 unità; l'arena disegna 100 unità come 1 metro). Le posizioni sono `[x, altezza, z]`
con l'altezza che cresce verso l'alto.

### Un esempio minimo

È nel [README in inglese](README.md#a-minimal-example): crea il personaggio una volta, poi a
ogni tick gli passa i tasti e la posizione dei nemici, e applica ciò che la libreria ha
deciso (slancio, altezza, animazione, eventi). `src/first-combo/src/main.rs`
è quel ciclo in un programma che si può lanciare; l'arena (`src/arena/src/main.rs`, funzione
`simulate`) è lo stesso ciclo dentro un gioco vero.

### Cosa il gioco deve dire alla libreria

- **Un colpo è andato a segno.** Finché `sora.combo.hit_power()` vale `Some(potenza)`,
  controlla se l'arma tocca un nemico. Se sì, passa `hit_landed: true` in quel tick. Senza,
  la combo non prosegue, proprio come in KH2. `sora.combo.hit_attack()` dà il numero del
  colpo: `catalog.attack(numero)` ne ha potenza, spinta e gli altri valori.
- **Arriva un colpo nemico.** Se `sora.combo.guarding()` è vero chiama `sora.guarded()`: il
  colpo è fermato. Se è finito su una parata vera parte la parata riuscita, e da lì il tasto
  d'attacco dà il Counterguard; alcuni colpi parano mentre colpiscono, e allora il colpo
  semplicemente prosegue. Se
  `sora.combo.unhittable()` è vero il colpo va a vuoto. Altrimenti chiama
  `sora.knocked(animazione_di_danno, salita)`: la combo si rompe, parte un'animazione di
  danno e, con `salita` maggiore di zero, il personaggio viene sbalzato in aria (da lì
  diventano possibili Retaliating Slash e Aerial Recovery).
- **Un nemico ha parato la tua arma.** Chiama `sora.deflected()`.

### Tasti che richiamano un colpo preciso

Alcuni colpi non vengono dal tasto d'attacco: se ne passa il numero di voce in
`Controls::command`. Nel catalogo di Sora sono: Guard 11, Upper Slash 12, Finishing Leap 37,
Horizontal Slash 38. Counterguard (39) e Retaliating Slash (40) li richiama da solo il tasto
d'attacco quando la situazione è quella giusta. Un comando funziona solo se l'abilità è
equipaggiata e il momento è giusto; altrimenti arriva `Event::PressIgnored`.

### Abilità

`Loadout` contiene ciò che è equipaggiato. Le abilità che aggiungono colpi hanno un numero:
`loadout.equip(catalog.ability_number("Flash_Step").unwrap())`. I nomi sono quelli scritti
nel catalogo (righe che cominciano con `ability`). Le abilità che cambiano le regole sono
campi: `combo_plus`, `air_combo_plus`, `negative_combo`, `finishing_plus`, `combo_master`,
`berserk_charge`. Aerial Recovery è `rules::AERIAL_RECOVERY`. Le abilità si possono cambiare
in qualsiasi momento da `sora.combo.loadout`.

### Animazioni

La libreria parla con i numeri delle animazioni di KH2 (per esempio 151 è il primo colpo a
terra, 3 il salto, 4 la caduta). Le animazioni non le contiene. Per vedere Sora muoversi
vanno convertite dalla propria copia del gioco nel formato del proprio motore: gli script
`tools/kh2/prepare_arena.py` e `tools/kh2/export_arena_poses.py` mostrano un modo (scrivono
una posa per tick per ogni osso, che l'arena riproduce).

### Usarla da un altro linguaggio

La libreria è Rust semplice, senza dipendenze. Per chiamarla da C, C++, C# o da un
linguaggio di script basta avvolgere `Fighter` in un piccolo pacchetto `cdylib` che espone
funzioni `extern "C"` (crea, avanza di un tick, leggi lo stato). Quel pacchetto non è incluso.

## Come è stato scritto il codice

La libreria in `src/kh2-combat-core` è una **riscrittura "clean room"**. Una prima versione
era stata scritta mentre si studiava il gioco. Prima di pubblicare, il comportamento è stato
messo per iscritto in una specifica funzionale (`docs/SPEC-FUNZIONALE.md`) e in una
descrizione dell'interfaccia (`docs/INTERFACCIA.md`), e la libreria è stata riscritta da
zero, solo da quei due documenti e dai dati, da una sessione separata che non aveva accesso
né al codice precedente né agli appunti dello studio. [PROVENANCE.md](PROVENANCE.md) dice con
precisione cosa quella sessione aveva e cosa no, e cosa il metodo garantisce e cosa non
garantisce. Il diario di lavoro e i file di domande e risposte che vi sono citati sono
conservati privatamente da chi ha fatto il progetto, perché contengono nomi personali: non
fanno parte di questo repository. Dopo la riscrittura, l'unica modifica fatta dentro la
cartella della libreria sono due righe di dati del pacchetto nel suo `Cargo.toml` (`license`
e `publish`): configurazione, non codice. I due documenti sono qui
esattamente come quella sessione li ha ricevuti (citano una cartella `data\`: è la cartella
di lavoro della riscrittura, con il catalogo e le registrazioni).

L'esempio, l'arena e gli script in `tools/kh2` non fanno parte della riscrittura: usano solo
l'interfaccia pubblica della libreria o leggono i file di dati del gioco.

## Quanto è fedele

Le regole vengono da tre fonti: le tabelle di dati del gioco (lette dagli script dalla
propria copia), registrazioni del gioco vero fotogramma per fotogramma ("telemetria"), e una
descrizione scritta del comportamento (`docs/SPEC-FUNZIONALE.md`). I test rigiocano le
registrazioni nella libreria e confrontano.

| Cosa | Come è stata controllata | Risultato |
|---|---|---|
| Regole della combo a terra e in aria (quali pressioni valgono, quando parte il colpo dopo, quando arriva il finale) | 128 combo registrate e rigiocate | 0 differenze |
| Salto e caduta | 45 salti registrati e rigiocati, altezza confrontata a ogni tick | 36 entro 2 unità, 42 entro 5 (gli altri finiscono contro un gradino o un nemico) |
| Slancio in avanti dei colpi aerei | 30 colpi aerei registrati con il bersaglio lontano | 28 fanno la stessa strada entro l'8% |
| Scelta del colpo per ogni mossa e abilità | ogni voce del catalogo di Sora provata nella sua situazione | passa; nel gioco vero controllata solo per i colpi senza abilità |
| Effetto di Combo Plus, Negative Combo, Finishing Plus | preso dalla documentazione della community | **non controllato nel gioco** |
| Parata, parata riuscita, Counterguard, Retaliating Slash, Aerial Recovery | tempi dai dati del gioco; regole dalla specifica | provate nell'arena, **non confrontate con registrazioni** |
| Velocità dello slancio verso un bersaglio vicino; salita dei colpi da terra verso l'alto | ricavate da pochi campioni | **approssimate** |

Le registrazioni non sono incluse (vengono dalla partita di un giocatore), quindi i due test
che le rigiocano si saltano da soli e lo dicono. La build Steam usata per tutte le misure è
la 1.0.0.10.

## Il file del catalogo

Un file di testo semplice, una cosa per riga, scritto da `tools/kh2/build_catalog.py` in
`extracted/kh2/catalog/sora_base.catalog`:

- `entry ...` una riga per ogni voce della tabella di scelta: quando può essere scelta
  (distanza, altezza, angolo, terra o aria, abilità richiesta) e quale animazione fa partire;
- `motion ...` un'animazione e la sua durata in tick;
- `span ...` e `point ...` gli eventi di un'animazione: zone di danno, "combo permessa",
  "finale permesso", "non colpibile", e così via;
- `attack ...` i numeri di un colpo: potenza, spinta, e così via;
- `ability ...` numero e nome di un'abilità.

Lo legge `Catalog::load(percorso)`; `Catalog::default_path()` indica il posto in cui lo
scrive lo script. Il catalogo contiene dati del gioco: va tenuto fuori da ogni repository.

## Test

```bash
cargo test --manifest-path src/Cargo.toml
```

- I test dentro i moduli e `tests/interfaccia.rs` usano piccoli cataloghi inventati: girano
  ovunque.
- `tests/catalogo_vero.rs` usa il catalogo vero: si salta da solo finché non lo si costruisce.
- `tests/registrazioni.rs` rigioca registrazioni del gioco da una cartella `logs/`: si salta
  da solo quando non ce ne sono.

## Sulla lingua

I commenti nel codice, i nomi dei test e i due documenti sono in italiano; i nomi nel codice,
e l'esempio, sono in inglese.

## Note legali

- Questo repository non contiene **file del gioco, codice decompilato o dati estratti**. Per
  generare il catalogo serve la propria copia originale del gioco.
- La cartella `extracted/` e tutto ciò che viene generato dal gioco non vanno pubblicati.
- Kingdom Hearts è un marchio di Disney e Square Enix. Questo progetto non è ufficiale.

## Riconoscimenti

Vedi [CREDITS.md](CREDITS.md).

## Licenza

MIT, vedi [LICENSE](LICENSE). Riguarda il codice e i documenti di questo repository, non il
gioco né ciò che se ne genera.
