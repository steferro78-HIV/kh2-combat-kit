# Interfaccia della libreria `kh2-combat-core`

Questo documento descrive **la forma** che la libreria deve avere perché il resto del progetto
(un'arena di prova, una DLL, un esempio, i README) continui a compilare e a funzionare senza
modifiche: nomi, tipi, campi, firme, eventi, e cosa deve fare ciascuna funzione. Non contiene
il corpo di nessuna funzione: come ottenere quel comportamento lo decide chi scrive la libreria.

Il comportamento del combattimento è in `SPEC-FUNZIONALE.md`; qui lo si richiama con "SPEC" e
il numero della sezione.

Contenuto: 1. il pacchetto; 2. `catalog`; 3. `rules`; 4. `combo`; 5. `air`; 6. `fighter`;
7. il formato del file catalogo; 8. le registrazioni e come rigiocarle.

## 1. Il pacchetto

- Pacchetto Cargo `kh2-combat-core` (libreria `kh2_combat_core`), edizione 2021, **nessuna
  dipendenza**. Versione del pacchetto: `0.4.0`.
- Sta in `src\kh2-combat-core\`, dentro un workspace Cargo il cui file è `src\Cargo.toml`.
- Moduli pubblici: `air`, `catalog`, `combo`, `fighter`, `rules`.
- Nella radice della libreria:

```rust
/// Quante volte al secondo chi usa la libreria deve farla avanzare.
pub const TICKS_PER_SECOND: u32 = 60;
/// La versione del pacchetto (quella scritta in Cargo.toml), mai vuota.
pub fn version() -> &'static str;
```

I tipi `Arc`, `BTreeMap`, `Path` e `PathBuf` che compaiono nelle firme sono quelli della
libreria standard di Rust.

Unità, valide ovunque: un **tick** è 1/60 di secondo; le distanze sono in unità di KH2; una
posizione è `[x, altezza, z]` con l'altezza verso l'alto; una direzione orizzontale è `[x, z]`
di lunghezza 1; i frame delle animazioni si contano da 1.

**Dove stanno i dati per i test.** `Catalog::default_path()` (sezione 2) indica il posto del
catalogo nel progetto principale. Nella cartella di lavoro pulita i dati stanno altrove, quindi
i test che usano dati veri devono cercare in due posti e saltarsi da soli (stampando una riga
che lo dice) se non trovano niente:

| Dato | Primo posto | Secondo posto |
|---|---|---|
| catalogo | `Catalog::default_path()` | `<cartella del pacchetto>\..\..\data\catalog\sora_base.catalog` |
| registrazioni | `<cartella del pacchetto>\..\..\logs\` | `<cartella del pacchetto>\..\..\data\logs\` |

## 2. Modulo `catalog`: i dati

```rust
/// Interruttori di una voce del repertorio (si sommano nel campo `flags`).
pub mod flag {
    pub const AERIAL: u32 = 1;         // voce "in aria" (senza: "a terra")
    pub const REACH_GROWS: u32 = 2;    // il limite alto del dislivello cresce con la dote
    pub const FINISHER: u32 = 4;       // la voce è un finale
    pub const LOW_INVERTED: u32 = 8;   // la taglia si toglie dal limite basso invece di aggiungerla
    pub const HIGH_INVERTED: u32 = 16; // la taglia si aggiunge al limite alto invece di toglierla
}

/// Tipo di una voce: contro chi si valuta (SPEC 6.2).
pub mod kind {
    pub const BY_COMMAND: u8 = 0;
    pub const MAIN_TARGET: u8 = 1;
    pub const ANY_NEARBY: u8 = 2;
    pub const ALWAYS: u8 = 3;
}

/// Numeri degli eventi "a tratto" usati dalle regole (SPEC 3).
pub mod trigger {
    pub const GROUNDED: u8 = 0;          // a terra
    pub const AIRBORNE: u8 = 1;          // in aria
    pub const BLEND_TO_IDLE: u8 = 2;     // ritorno alla posa di riposo
    pub const HITBOX: u8 = 10;           // zona
    pub const COMBO_ALLOWED: u8 = 11;    // combo permessa
    pub const REACTION_ALLOWED: u8 = 14; // comando di reazione permesso
    pub const UNHITTABLE: u8 = 27;       // non colpibile
    pub const HITBOX_COMBO: u8 = 33;     // zona (secondo tipo, vale come il primo)
    pub const FINISHER_ALLOWED: u8 = 50; // finale permesso
}

/// Numeri di animazione con un significato fisso (SPEC 10).
pub mod motion {
    pub const JUMP: u16 = 3;
    pub const FALL: u16 = 4;
    pub const LAND: u16 = 5;
    pub const AERIAL_RECOVERY: u16 = 40;
    pub const DEFLECT: u16 = 41;
    pub const GUARD_SUCCESS: u16 = 42;
    pub const DEFLECT_AIR: u16 = 43;
}

/// Una voce del repertorio. I campi corrispondono uno a uno a quelli della riga `entry`
/// del file catalogo (sezione 7), dove ne è spiegato il significato.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u8,
    pub kind: u8,             // nel file: type
    pub alternative: i8,      // nel file: sub
    pub step_advance: i8,     // nel file: combo_offset
    pub flags: u32,           // nel file: flag
    pub motion: u16,
    pub next_motion: u16,
    pub jump: f32,
    pub jump_max: f32,
    pub jump_min: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub near: f32,
    pub far: f32,
    pub low: f32,
    pub high: f32,
    pub inner_min: f32,
    pub inner_max: f32,
    pub blend: f32,
    pub distance_adjust: f32, // nel file: dist_adjust
    pub ability: u16,
    pub score: u16,
}

impl Entry {
    pub fn is_aerial(&self) -> bool;   // ha l'interruttore AERIAL
    pub fn is_finisher(&self) -> bool; // ha l'interruttore FINISHER
    pub fn lunges(&self) -> bool;      // è un colpo "in volo": il campo jump è maggiore di zero
}

/// Un evento "a tratto".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: i16,
    pub end: i16,       // negativo: fino alla fine dell'animazione
    pub trigger: u8,
    pub params: Vec<i16>,
}

impl Span {
    /// Vero se l'evento vale al frame dato: start <= frame, e (end negativo oppure frame <= end).
    pub fn covers(&self, frame: u16) -> bool;
}

/// Un evento "a istante".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub frame: i16,
    pub trigger: u8,
    pub params: Vec<i16>,
}

/// Un'animazione con i suoi eventi, nell'ordine in cui sono scritti nel file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Motion {
    pub id: u16,
    pub name: String,
    pub frames: u16,
    pub body: Vec<Span>,          // eventi a tratto del personaggio
    pub weapon: Vec<Span>,        // eventi a tratto dell'arma
    pub body_points: Vec<Point>,
    pub weapon_points: Vec<Point>,
}

impl Motion {
    /// Vero se un evento a tratto **del personaggio** con quel numero vale al frame dato.
    pub fn active(&self, trigger: u8, frame: u16) -> bool;
    /// Il più piccolo `start` (portato a zero se negativo) tra gli eventi a tratto del
    /// personaggio con quel numero; `None` se non ce ne sono.
    pub fn first_frame_of(&self, trigger: u8) -> Option<u16>;
    /// "Neutro" al frame dato: vale GROUNDED, AIRBORNE o BLEND_TO_IDLE (SPEC 5).
    pub fn neutral(&self, frame: u16) -> bool;
    /// Il numero del colpo a segno della **prima** zona che copre il frame dato: il primo
    /// parametro del primo evento HITBOX o HITBOX_COMBO che copre il frame, cercando prima
    /// tra quelli dell'arma e poi tra quelli del personaggio. `None` se non c'è una zona, se
    /// l'evento trovato non ha parametri, o se il suo primo parametro è negativo. È una
    /// domanda sui soli dati dell'animazione; quale zona conta per la combo quando più zone
    /// si sovrappongono è detto nella sezione 4, "Le zone attive".
    pub fn hitbox(&self, frame: u16) -> Option<u16>;
}

/// I numeri di un colpo a segno (riga `attack` del file catalogo).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attack {
    pub id: u16,
    pub sub: u16,          // nel file: sub_id
    pub power: u16,
    pub attack_type: u8,   // nel file: type
    pub element: u8,
    pub reaction: u8,
    pub knock1: i16,
    pub knock2: i16,
    pub interval: u16,
    pub revenge: u8,
    pub combo_group: u8,
    pub kind: u8,
}

impl Attack {
    /// La zona è una parata: il tipo è 2 (parata) o 3 (parata totale).
    pub fn guards(&self) -> bool;
}

/// Tutto il catalogo di un personaggio.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub entries: Vec<Entry>,                    // nell'ordine del file: è l'ordine di priorità
    pub motions: BTreeMap<u16, Motion>,         // per numero di animazione
    pub attacks: BTreeMap<u16, Vec<Attack>>,    // per numero; le righe con lo stesso numero, nell'ordine del file
    pub abilities: BTreeMap<u16, String>,       // numero -> nome
}

impl Catalog {
    /// `<cartella del pacchetto>/../../extracted/kh2/catalog/sora_base.catalog`, dove la
    /// cartella del pacchetto è quella nota al momento della compilazione.
    pub fn default_path() -> PathBuf;
    /// Legge il file e lo passa a `parse`. Se il file non si legge, l'errore contiene il percorso.
    pub fn load(path: &Path) -> Result<Self, String>;
    /// Legge il testo di un catalogo (sezione 7).
    pub fn parse(text: &str) -> Result<Self, String>;
    /// Posizione in `entries` della **prima** voce con quel numero.
    pub fn index_of_id(&self, id: u8) -> Option<usize>;
    pub fn motion(&self, id: u16) -> Option<&Motion>;
    /// La riga del primo contatto di quel colpo a segno: quella con `sub` zero; se non c'è,
    /// la prima con quel numero.
    pub fn attack(&self, id: u16) -> Option<&Attack>;
    /// Il numero dell'abilità con quel nome (per esempio "Slide_Dash").
    pub fn ability_number(&self, name: &str) -> Option<u16>;
}
```

## 3. Modulo `rules`: abilità, limiti, scelta del colpo

```rust
/// Numeri di voce richiamati dalla situazione (SPEC 8) e numero dell'abilità Aerial Recovery.
pub const COUNTERGUARD: u8 = 39;
pub const RETALIATING_SLASH: u8 = 40;
pub const AERIAL_RECOVERY: u16 = 20;

/// Le abilità equipaggiate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loadout {
    // (privato) l'insieme dei numeri delle abilità equipaggiate con `equip`
    pub combo_plus: u8,        // quante copie
    pub air_combo_plus: u8,    // quante copie
    pub negative_combo: bool,
    pub finishing_plus: u8,    // quante copie
    pub combo_master: bool,
    pub berserk_charge: bool,
}

/// Quanto può essere lunga una combo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub ground_steps: u8,
    pub air_steps: u8,
    pub finishers: u8,
}

impl Loadout {
    pub fn equip(&mut self, ability: u16);    // equipaggia un'abilità che ha un numero
    pub fn unequip(&mut self, ability: u16);
    pub fn has(&self, ability: u16) -> bool;
    /// I limiti secondo SPEC 5: passi a terra = 3 + combo_plus, in aria = 3 + air_combo_plus,
    /// ciascuno meno 1 con negative_combo e mai sotto 1; finali = 1 + finishing_plus.
    pub fn limits(&self) -> Limits;
}

/// Un nemico, come lo vedono le regole.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    pub position: [f32; 3], // il punto a cui si mira: x, altezza, z
    pub radius: f32,        // si toglie dalla distanza tra i centri
    pub size: f32,          // "taglia": allarga i limiti di dislivello
}

/// La situazione attorno al personaggio quando parte un colpo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene<'a> {
    pub fighter: [f32; 3],          // dove sta il personaggio (i piedi): x, altezza, z
    pub facing: [f32; 2],           // dove guarda: x, z, di lunghezza 1
    pub main_target: Option<Body>,  // il bersaglio principale
    pub nearby: &'a [Body],         // tutti i nemici nei dintorni, principale compreso
    pub ledge_ahead: bool,          // a terra: subito davanti il terreno manca
}
// `Scene` ha anche `Default`: fighter [0, 0, 0], facing [0, 1], nessun bersaglio, nessun
// nemico, ledge_ahead falso.

/// Lo stato del personaggio che conta per la scelta.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FighterView {
    pub airborne: bool,
    pub current_motion: Option<u16>, // l'animazione in corso, se c'è
    pub steps: u8,                   // i passi, già contando il colpo che sta per partire
    pub finishers: u8,
    pub finisher_window: bool,       // l'animazione in corso era nella finestra "finale permesso"
    pub reach_bonus: f32,            // la "dote" (SPEC 13): oggi sempre zero
    pub berserk: bool,               // la condizione di Berserk Charge è vera adesso
}

/// SPEC 6.1.
pub fn available(entry: &Entry, loadout: &Loadout, view: &FighterView) -> bool;
/// SPEC 6.2: una casella per ogni voce del repertorio, nello stesso ordine; vera se la voce
/// è candidata. (Il ripiego di SPEC 6.3 qui non c'entra.)
pub fn candidates(catalog: &Catalog, loadout: &Loadout, view: &FighterView, scene: &Scene) -> Vec<bool>;
/// SPEC 6.2 + 6.3: il colpo che parte. `None` solo se manca perfino la voce di ripiego.
pub fn choose<'a>(catalog: &'a Catalog, loadout: &Loadout, view: &FighterView, scene: &Scene) -> Option<&'a Entry>;
/// SPEC 8: la prima voce con quel numero, se è disponibile (SPEC 6.1); altrimenti `None`.
pub fn by_command<'a>(catalog: &'a Catalog, loadout: &Loadout, view: &FighterView, id: u8) -> Option<&'a Entry>;
```

Le voci di ripiego (5 a terra, 9 in aria) e la voce del bordo (1) sono numeri fissi (SPEC 6).

### Dislivello: il conto esatto

Nel file i limiti `low` e `high` di una voce dicono quanto il bersaglio sta più **in basso**
del personaggio (numeri negativi = più in alto). Per un nemico di taglia `size`:

1. si parte da `high`; se la voce ha REACH_GROWS gli si toglie la dote (`reach_bonus`);
2. limite basso corretto = `low + size`, oppure `low − size` se la voce ha LOW_INVERTED;
3. limite alto corretto = il valore del punto 1 `− size`, oppure `+ size` se ha HIGH_INVERTED;
4. chiamando "salita" quanto il punto di mira del nemico sta più **in alto** dei piedi del
   personaggio, la condizione è: −(limite basso corretto) ≤ salita < −(limite alto corretto).

Esempio: `low = -20`, `high = -110`, taglia 0, nessun interruttore: il bersaglio deve stare
tra 20 (compreso) e 110 (escluso) unità più in alto dei piedi.

Gli stessi due numeri, −(limite basso corretto) e −(limite alto corretto) calcolati con dote
zero, sono il minimo e il massimo della salita dei colpi in volo fatti partire da terra,
quando la voce guarda il dislivello, cioè quando i suoi `low` e `high` sono diversi (SPEC 7).

### Coseno

Il coseno si calcola in orizzontale tra la direzione dal personaggio al nemico e la direzione
di riferimento, e non supera mai 1. Se il nemico è esattamente sopra il personaggio vale 1.

## 4. Modulo `combo`: la combo e le reazioni

```rust
/// Ciò che chi usa la libreria dice a ogni tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickInput<'a> {
    pub attack_pressed: bool,   // Attacco è passato da rilasciato a premuto in questo tick
    pub command: Option<u8>,    // un comando: il numero della voce da richiamare
    pub recover_pressed: bool,  // il tasto del ritorno in equilibrio è stato premuto in questo tick
    pub hit_landed: bool,       // l'animazione in corso ha toccato qualcosa in questo tick
    pub busy: bool,             // il personaggio non può agire
    pub airborne: bool,         // il personaggio è in aria
    pub berserk: bool,          // la condizione di Berserk Charge è vera
    pub scene: Scene<'a>,
}

/// Perché una pressione non ha avuto effetto.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreReason {
    Busy,
    TooEarly,
    NotTouched,
    ComboOver,
    Unavailable,
}

/// Cosa è successo, nell'ordine in cui è successo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    MoveStarted { entry: u8, motion: u16, step: u8 },
    ReactionStarted { motion: u16 },
    Lunge { rise: f32, run: f32, speed: f32 },
    HitWindowOpened { attack: u16, power: u16 },
    HitWindowClosed,
    ChainOpened,
    PressIgnored(IgnoreReason),
    ComboEnded,
    Jumped,   // lo emette solo `Fighter`
    Landed,   // lo emette solo `Fighter`
}

/// La combo di un personaggio.
#[derive(Clone, Debug)]
pub struct Combo {
    pub loadout: Loadout,   // le abilità equipaggiate: si possono cambiare in qualsiasi momento
    // (il resto è privato)
}

impl Combo {
    pub fn new(catalog: Arc<Catalog>, loadout: Loadout) -> Self;
    pub fn catalog(&self) -> &Catalog;
    pub fn override_next_move(&mut self, motion: Option<u16>);
    pub fn current_entry(&self) -> Option<&Entry>;
    pub fn current_move(&self) -> Option<(u16, u16)>;
    pub fn hit_power(&self) -> Option<u16>;
    pub fn hit_attack(&self) -> Option<u16>;
    pub fn guarding(&self) -> bool;
    pub fn unhittable(&self) -> bool;
    pub fn reaction_allowed(&self) -> bool;
    pub fn free(&self) -> bool;
    pub fn launched(&self) -> bool;
    pub fn step(&self) -> u8;
    pub fn finishers(&self) -> u8;
    pub fn interrupt(&mut self) -> Vec<Event>;
    pub fn deflected(&mut self, airborne: bool) -> Vec<Event>;
    pub fn guarded(&mut self) -> Vec<Event>;
    pub fn knocked(&mut self, reaction: u16, launched: bool) -> Vec<Event>;
    pub fn tick(&mut self, input: TickInput) -> Vec<Event>;
}
```

### Lo stato di una combo

Una combo appena creata non ha nessuna animazione in corso, non è "sbalzata", e non ha un
colpo imposto. Quando un'animazione è in corso la combo ne conosce: il numero, il frame
(da 1), la voce del repertorio (solo se è un colpo, non per una reazione), se "ha toccato", e
le zone attive in quel frame.

### Le zone attive

A ogni frame dell'animazione in corso si considerano tutti gli eventi "zona" (HITBOX e
HITBOX_COMBO, dell'arma e del personaggio) che coprono quel frame e hanno un primo parametro
non negativo: quel parametro è il numero del colpo a segno. Una zona è **di parata** se il
suo colpo a segno è di tipo parata o parata totale (`Attack::guards`); altrimenti è **di
danno** (anche quando il numero non è nel catalogo: allora la potenza vale 0).

| Cosa | Qual è |
|---|---|
| la zona di danno attiva | tra le zone di danno che coprono il frame, quella con la potenza più alta; a parità, la prima nell'ordine "prima gli eventi dell'arma, poi quelli del personaggio, ciascuno nell'ordine del file" |
| la parata attiva | c'è se almeno una zona di parata copre il frame |
| la zona indicata (`hit_attack`) | la zona di danno attiva, se c'è; altrimenti la prima zona di parata che copre il frame (stesso ordine); altrimenti nessuna |

### Le domande

| Funzione | Risposta |
|---|---|
| `catalog` | il catalogo con cui è stata creata |
| `current_entry` | la voce del colpo in corso; `None` se non c'è niente in corso o se è una reazione |
| `current_move` | (numero dell'animazione in corso, frame da 1), colpo o reazione; `None` se non c'è niente |
| `hit_attack` | il numero del colpo a segno della zona indicata in questo tick ("Le zone attive") |
| `hit_power` | la potenza della zona di danno attiva; `None` se non ce n'è una (anche se c'è una parata). La potenza è quella della riga data da `Catalog::attack`; 0 se quel numero non è nel catalogo |
| `guarding` | vero se in questo tick c'è una parata attiva, dei due tipi (anche se insieme c'è una zona di danno) |
| `unhittable` | vero se l'animazione in corso ha "non colpibile" attivo in questo frame; falso senza animazione |
| `reaction_allowed` | vero se non c'è nessuna animazione in corso, oppure se quella in corso ha "comando di reazione permesso" attivo in questo frame |
| `free` | vero se non c'è nessuna animazione in corso, oppure se quella in corso è neutra in questo frame |
| `launched` | vero se il personaggio è "sbalzato" (SPEC 10) |
| `step` | i passi; **0 se non c'è nessuna animazione in corso** |
| `finishers` | i finali; **0 se non c'è nessuna animazione in corso** |

### Come scorre un'animazione, e che eventi dà

A ogni `tick`, **per prima cosa**, l'animazione in corso avanza (SPEC 4):

1. se il frame successivo sarebbe l'ultimo (frame attuale + 1 ≥ durata) l'animazione
   **finisce**: se c'era una zona attiva esce `HitWindowClosed`; poi `ComboEnded`. Non c'è più
   niente in corso e lo stato "sbalzato" finisce. Lo stesso succede se il catalogo non conosce
   l'animazione in corso;
2. altrimenti il frame sale di 1 e si guarda la zona indicata nel nuovo frame ("Le zone
   attive"). Se il suo numero è diverso da quello del frame prima (o prima non c'era una zona,
   o adesso non c'è): esce `HitWindowClosed` se prima una zona c'era, poi
   `HitWindowOpened { attack, power }` se adesso c'è, con la potenza di quel colpo a segno nel
   catalogo (per una parata di solito 0). Una parata che comincia o finisce mentre resta
   indicata la stessa zona di danno non dà eventi: la si vede con `guarding`;
3. poi, se in questo frame è aperta la finestra "combo permessa" **o** "finale permesso" e al
   frame prima non lo era nessuna delle due, esce `ChainOpened`.

Quando un'animazione **parte** (colpo o reazione) è creata e fatta subito avanzare al frame 1
nello stesso tick, con le regole dei punti 2 e 3: se al frame 1 c'è una zona esce
`HitWindowOpened`, se al frame 1 una finestra è aperta esce `ChainOpened`.

### `tick`: l'ordine delle cose

1. L'animazione in corso avanza (sopra).
2. Se `hit_landed` è vero e c'è un'animazione in corso, questa "ha toccato".
3. **Ritorno in equilibrio.** Se `recover_pressed`, il personaggio è sbalzato, `busy` è falso e
   l'abilità `AERIAL_RECOVERY` è equipaggiata: lo sbalzo finisce, passi = 0 e finali = 1, parte
   la reazione `motion::AERIAL_RECOVERY` (vedi "Far partire una reazione") e il tick **finisce
   qui**: un'eventuale pressione nello stesso tick è persa, senza `PressIgnored`.
4. **Pressione.** Se `attack_pressed` è vero oppure `command` c'è:
   1. se `busy` è falso, il personaggio è in una delle due situazioni di SPEC 8, e in questo
      tick c'è `attack_pressed` **oppure** `command` è proprio il numero della voce di quella
      situazione: si prova a far partire quella voce, cioè passi +1 (al più 100), finali
      almeno 1, poi come un comando con quel numero, con la finestra "finale permesso"
      considerata chiusa. Se parte, il tick finisce qui. Se non parte, i contatori tornano
      com'erano e si prosegue. Un comando con un altro numero, senza `attack_pressed`, non
      richiama la voce della situazione: si passa direttamente al punto 2. Se valgono tutte e
      due le situazioni insieme: con `attack_pressed` si prova solo la voce dello sbalzo (la
      40); senza `attack_pressed`, il comando 40 prova la 40 e il comando 39 prova la 39;
   2. la pressione viene giudicata (tabella qui sotto);
   3. se vale, parte il colpo ("Far partire un colpo"). Se nessun colpo può partire, i
      contatori tornano com'erano prima della pressione ed esce
      `PressIgnored(IgnoreReason::Unavailable)`.

Se nello stesso tick ci sono sia `attack_pressed` sia `command`, vale il comando.

### Il giudizio di una pressione

Si guarda l'animazione in corso al suo frame attuale (dopo l'avanzamento del punto 1). "Ha
toccato" comprende Combo Master. "Altri finali" vuol dire finali < limite dei finali.

| # | Situazione | Esito |
|---|---|---|
| 1 | `busy` | ignorata: `Busy` |
| 2 | nessuna animazione in corso | vale: passi = 1, finali = 1 |
| 3 | finestra "finale permesso", ha toccato, altri finali | vale: passi +1, finali +1 (ciascuno al più 100) |
| 4 | finestra "combo permessa", ha toccato | vale: passi +1 (al più 100) |
| 5 | animazione neutra in questo frame | vale: passi = 1, finali = 1 |
| 6 | finestra "combo permessa" aperta, oppure finestra "finale permesso" aperta con altri finali | ignorata: `NotTouched` |
| 7 | il colpo in corso è un finale, ci sono altri finali, e la finestra "finale permesso" si apre più avanti (frame attuale < `first_frame_of`) | ignorata: `TooEarly` |
| 8 | il colpo in corso è un finale (altri casi) | ignorata: `ComboOver` |
| 9 | l'animazione non è un finale (colpo normale o reazione) e la finestra "combo permessa" si apre più avanti, oppure non ne ha nessuna | ignorata: `TooEarly` |
| 10 | altrimenti | ignorata: `ComboOver` |

Per le righe 3, 4 e 5 la scelta del colpo riceve anche l'informazione "era nella finestra
finale permesso" (vera per la riga 3; per le righe 4 e 5 vera se quella finestra era aperta);
per la riga 2 è falsa.

### Far partire un colpo

1. Si prepara lo stato per la scelta (`FighterView`): `airborne` e `berserk` dall'ingresso,
   l'animazione in corso (se c'è), i contatori già aggiornati, la finestra finale, dote zero.
2. La voce si decide così, nell'ordine:
   - se c'è un colpo imposto con `override_next_move`: tra le voci che hanno quell'animazione,
     la prima il cui "in aria / a terra" corrisponde ad `airborne`; se nessuna corrisponde, la
     prima con quell'animazione. Il colpo imposto vale per una sola partenza e poi è
     dimenticato (anche se non ha trovato nessuna voce);
   - altrimenti, se c'è `command`: `rules::by_command`;
   - altrimenti: `rules::choose`.
   Se non esce nessuna voce, il colpo non parte.
3. Se esce una voce: se c'era una zona attiva esce `HitWindowClosed`; se la voce è un colpo in
   volo e `airborne` è falso i contatori diventano passi = 1 e finali = 1; lo sbalzo finisce;
   l'animazione in corso diventa quel colpo, con "ha toccato" falso; esce
   `MoveStarted { entry, motion, step }` con il numero della voce, la sua animazione e i passi
   attuali; se è un colpo in volo esce subito dopo `Lunge` (sotto); infine l'animazione va al
   frame 1 con gli eventi relativi.

`Lunge { rise, run, speed }` (SPEC 7): `rise` è la salita; `run` la distanza dal bordo del
bersaglio principale (distanza orizzontale tra i centri meno il raggio, mai sotto zero; 0 senza
bersaglio); `speed` la velocità di partenza dello slancio. La libreria `Combo` qui si limita a
dirli: a muovere il personaggio pensa `Fighter` (sezione 6).

### Far partire una reazione

Se il catalogo non conosce quell'animazione: l'animazione in corso (se c'è) finisce come al
punto 1 di "Come scorre un'animazione" (eventuale `HitWindowClosed`, poi `ComboEnded`) e non
parte niente. Altrimenti: se c'era una zona attiva esce `HitWindowClosed`; l'animazione in
corso diventa la reazione (non è una voce del repertorio); esce `ReactionStarted { motion }`;
l'animazione va al frame 1 con gli eventi relativi.

### Le altre funzioni che cambiano lo stato

| Funzione | Cosa fa | Cosa restituisce |
|---|---|---|
| `override_next_move(Some(n))` | impone l'animazione del prossimo colpo che partirà (serve a rigiocare le registrazioni); `None` toglie l'imposizione | niente |
| `interrupt()` | l'animazione in corso finisce; lo sbalzo finisce | gli eventi della fine (`HitWindowClosed` se c'era una zona, poi `ComboEnded`); vuoto se non c'era niente in corso |
| `deflected(airborne)` | l'arma è stata respinta (SPEC 10). Solo se c'è un'animazione in corso: "ha toccato" diventa vero e i passi scendono di 1 (mai sotto 0); poi, se il catalogo conosce l'animazione di rimbalzo (`DEFLECT_AIR` se `airborne`, altrimenti `DEFLECT`), questa parte come reazione, con "ha toccato" già vero; se non la conosce, l'animazione in corso prosegue | gli eventi della partenza della reazione; vuoto negli altri casi |
| `guarded()` | un colpo nemico è finito sulla parata. Solo se in questo momento almeno una delle zone di parata che coprono il frame è di tipo **parata** (tipo 2): come `deflected`, ma con l'animazione `GUARD_SUCCESS`. Se è attiva solo una parata totale (tipo 3), o nessuna parata, non cambia niente | come sopra; vuoto se non cambia niente |
| `knocked(reaction, launched)` | il personaggio ha subito un colpo: passi = 0, finali = 1; parte la reazione `reaction` con "ha toccato" falso (se il catalogo non la conosce, vedi "Far partire una reazione"); poi lo stato "sbalzato" diventa `launched`, in ogni caso | gli eventi |

Lo stato "sbalzato" diventa falso quando: parte un colpo, parte il ritorno in equilibrio,
un'animazione finisce, viene chiamata `interrupt`, o `knocked` viene chiamata con `false`.

## 5. Modulo `air`: altezza e slancio

```rust
/// Quanto sale un salto pieno.
pub const JUMP_HEIGHT: f32 = 185.0;
/// Accelerazione di caduta, per tick al quadrato.
pub const GRAVITY: f32 = 0.408;

/// Quanti tick dura una salita alta `rise` (zero se `rise` è negativa): SPEC 9.
pub fn rise_ticks(rise: f32) -> f32;
/// Quanta strada fa in `ticks` tick uno slancio che parte a velocità 1 (SPEC 7, la forma
/// che vale anche per tempi non interi; zero se `ticks` è negativo).
pub fn drift_reach(ticks: f32) -> f32;

/// Lo slancio orizzontale di un colpo in volo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drift { /* privato */ }

impl Drift {
    /// `direction`: verso dove (x, z), di lunghezza 1. `speed`: velocità di partenza, per tick.
    pub fn new(direction: [f32; 2], speed: f32) -> Self;
    /// Avanza di un tick e dice lo spostamento (x, z) di questo tick. Alla prima chiamata e
    /// poi a una chiamata sì e una no (terza, quinta...), prima di rispondere la velocità
    /// perde un ventesimo.
    pub fn tick(&mut self) -> [f32; 2];
}

/// L'altezza del personaggio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air { /* privato */ }
// `Air` ha `Default`: a terra, all'altezza zero.

impl Air {
    pub fn height(&self) -> f32;
    /// Falso solo quando è a terra.
    pub fn airborne(&self) -> bool;
    /// Vero solo durante la salita (sospensione compresa) di un **salto**, non di un balzo.
    pub fn jumping(&self) -> bool;
    /// Comincia un salto (una salita di JUMP_HEIGHT dall'altezza attuale). Se è già in aria
    /// non fa niente e restituisce falso.
    pub fn jump(&mut self) -> bool;
    /// Il balzo di un colpo: comincia una salita di `rise` dall'altezza attuale, da terra o in
    /// aria, al posto di ciò che stava facendo. Con `rise` zero o negativa comincia a cadere
    /// da fermo.
    pub fn hop(&mut self, rise: f32);
    /// Avanza di un tick (SPEC 9). `jump_held`: il tasto del salto è giù. `floor`: l'altezza
    /// del terreno sotto il personaggio. Restituisce vero nel tick in cui tocca terra.
    pub fn tick(&mut self, jump_held: bool, floor: f32) -> bool;
}
```

Precisazioni su `Air::tick`:

- le chiamate `jump` e `hop` non cambiano l'altezza: la prima variazione arriva con il `tick`
  successivo, che è il tick numero 1 della salita;
- durante una salita o una sospensione il terreno non viene guardato;
- nel tick in cui il personaggio tocca terra l'altezza è esattamente `floor`.

## 6. Modulo `fighter`: il personaggio intero

```rust
/// Ciò che chi usa la libreria dice al personaggio a ogni tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls<'a> {
    pub attack_pressed: bool,
    pub command: Option<u8>,
    pub jump_pressed: bool,   // il tasto del salto è passato da rilasciato a premuto in questo tick
    pub jump_held: bool,      // il tasto del salto è giù
    pub hit_landed: bool,
    pub busy: bool,
    pub berserk: bool,
    pub floor: f32,           // l'altezza del terreno sotto il personaggio
    pub scene: Scene<'a>,     // l'altezza del personaggio (fighter[1]) la mette `Fighter`
}

/// Un personaggio: la combo, l'altezza e le regole che le legano.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub combo: Combo,
    // (il resto è privato)
}

impl Fighter {
    pub fn new(catalog: Arc<Catalog>, loadout: Loadout) -> Self;
    pub fn height(&self) -> f32;
    pub fn airborne(&self) -> bool;
    pub fn slide(&self) -> [f32; 2];
    pub fn shown(&self) -> Option<(u16, u16)>;
    pub fn deflected(&mut self) -> Vec<Event>;
    pub fn guarded(&mut self) -> Vec<Event>;
    pub fn knocked(&mut self, reaction: u16, rise: f32) -> Vec<Event>;
    pub fn tick(&mut self, controls: Controls) -> Vec<Event>;
}
```

Un personaggio appena creato è a terra all'altezza zero, fermo, senza slancio.

### `tick`: l'ordine delle cose

1. **Salto.** Se `jump_pressed`, `busy` è falso, il personaggio è a terra e `combo.free()` è
   vero: l'animazione in corso viene interrotta (`combo.interrupt()`, i suoi eventi vanno in
   uscita), comincia un salto, ed esce `Jumped`.
2. **Altezza.** L'altezza avanza di un tick (`Air::tick` con `jump_held` e `floor`). Se in
   questo tick il personaggio tocca terra: l'animazione in corso viene interrotta (eventi in
   uscita), comincia l'atterraggio, ed esce `Landed`.
3. **Combo.** `combo.tick` con: i tasti di `Controls`; `recover_pressed` uguale a
   `jump_pressed`; `airborne` dall'altezza (vero dal tick in cui stacca a quello prima di
   toccare terra); `busy` vero se lo era in `Controls` **oppure** se il salto è ancora nei
   suoi primi frame (SPEC 9: il frame del salto è il numero di tick dall'inizio, 1 nel tick
   in cui parte; è occupato finché quel frame è minore del primo frame "in aria"
   dell'animazione `JUMP`; se l'animazione `JUMP` non è nel catalogo o non ha quell'evento,
   non è mai occupato per questo motivo); la scena di `Controls` con l'altezza del personaggio
   al posto di `fighter[1]`. Gli eventi vanno in uscita dopo quelli dei punti 1 e 2.
4. **Balzo e slancio.** Per ogni `Lunge` uscito al punto 3: comincia un balzo di `rise`
   (`Air::hop`) e uno slancio alla velocità `speed`, verso il bersaglio principale della
   scena (in orizzontale; se non c'è, o è esattamente sopra, nella direzione `facing`).
5. Se in questo momento il colpo in corso non è un colpo in volo (o non c'è nessun colpo),
   lo slancio finisce. Se uno slancio c'è, avanza di un tick: il suo spostamento è ciò che
   `slide()` restituirà fino al prossimo `tick`; altrimenti `slide()` vale `[0, 0]`.

### Le altre funzioni

| Funzione | Cosa fa |
|---|---|
| `height` | l'altezza attuale |
| `airborne` | vero se non è a terra |
| `slide` | lo spostamento orizzontale (x, z) dovuto allo slancio nell'ultimo `tick`: chi usa la libreria lo somma alla posizione |
| `deflected` | `combo.deflected` con "in aria" preso dall'altezza |
| `guarded` | `combo.guarded` |
| `knocked(reaction, rise)` | se `rise` è maggiore di zero comincia un balzo di `rise` (il colpo lo sbalza in aria); poi `combo.knocked(reaction, è in aria adesso)` |

### `shown`: quale animazione mostrare

Restituisce (numero dell'animazione, frame da 1), oppure `None` quando il personaggio è fermo
o cammina (lo decide chi usa la libreria).

| Situazione | Risposta |
|---|---|
| un colpo o una reazione in corso | `combo.current_move()` |
| in aria, nella salita di un salto (sospensione compresa) | (`JUMP`, il frame del salto: 1 nel tick in cui parte, poi 2, 3...; al più durata − 1 e almeno 1) |
| in aria negli altri casi (caduta, balzo finito, colpo aereo finito...) | (`FALL`, un frame che gira in tondo da 1 alla durata dell'animazione `FALL`, durata compresa, e poi di nuovo 1; riparte da 1 ogni volta che la caduta **comincia a essere mostrata**, cioè quando dopo il tick precedente `shown` rispondeva un'altra cosa) |
| a terra, appena atterrato | (`LAND`, frame da 1 in su), finché l'animazione `LAND` non è finita (come ogni animazione, non mostra l'ultimo frame); poi `None` |
| a terra, altrimenti | `None` |

Se un'animazione non è nel catalogo, per questi conti la sua durata vale 1.

## 7. Il file catalogo

File di testo (UTF-8), una cosa per riga. Ogni riga è una **parola chiave** seguita da coppie
`nome=valore` separate da spazi; i valori non contengono spazi. Le righe vuote e quelle con
una parola chiave sconosciuta (per esempio la prima, `kh2-catalog 1`, e la riga `source`) si
ignorano. I pezzi di riga senza `=` si ignorano. I campi in più, non elencati qui, si ignorano.

Sono **richiesti** tutti i campi elencati nelle tabelle qui sotto, tranne tre che possono
mancare: `who` (allora è un evento del personaggio), `params` (allora nessun numero) e `name`
della riga `motion` (allora il nome è vuoto). Se manca un campo richiesto o il suo valore non
è un numero del tipo giusto (vale anche per un decimale non finito, come `NaN` o `inf`), la
lettura fallisce con un messaggio che contiene il numero della riga. Fallisce anche se alla
fine non c'è nemmeno una riga `entry`.

### `entry`: una voce del repertorio

Le righe `entry` sono nell'ordine di priorità. Due voci possono avere lo stesso `id`.

| Campo | Tipo | Significato |
|---|---|---|
| `id` | intero 0–255 | numero della voce |
| `type` | intero 0–255 | contro chi si valuta (SPEC 6.2). I valori con un significato sono da 0 a 3; una voce con un altro valore si legge lo stesso, non è mai candidata, e un comando la può richiamare come ogni altra voce |
| `sub` | intero, anche negativo | numero della voce alternativa (SPEC 6.3); negativo: nessuna |
| `combo_offset` | intero, anche negativo | anticipo del finale (SPEC 6.1) |
| `flag` | intero | somma degli interruttori (`catalog::flag`) |
| `motion` | intero | numero dell'animazione; 0: la voce non ha animazione |
| `next_motion` | intero | animazione che il gioco mette in coda dopo il colpo; non usato dalle regole |
| `jump`, `jump_min`, `jump_max` | decimali | valore di salto, minimo e massimo, in unità di altezza (SPEC 7) |
| `speed_min`, `speed_max` | decimali | velocità minima e massima dello slancio, in unità per tick (SPEC 7) |
| `near`, `far` | decimali | limiti di distanza (SPEC 6.2) |
| `low`, `high` | decimali | limiti di dislivello, vedi sezione 3, "Dislivello" |
| `inner_min`, `inner_max` | decimali | limiti del coseno (SPEC 6.2) |
| `blend` | decimale | tempo di sfumatura; non usato |
| `dist_adjust` | decimale | correzione della distanza; non usato |
| `ability` | intero | numero dell'abilità richiesta; 0: nessuna |
| `score` | intero | punteggio; non usato |

### `motion`: un'animazione

| Campo | Significato |
|---|---|
| `id` | numero dell'animazione |
| `name` | nome del file dell'animazione nel gioco (solo informativo) |
| `frames` | durata in frame (un frame = un tick) |

Una seconda riga `motion` con lo stesso `id` ne aggiorna nome e durata e conserva gli eventi.

### `span` e `point`: gli eventi di un'animazione

Devono venire dopo la riga `motion` della loro animazione (altrimenti la lettura fallisce).

| Campo | Riga | Significato |
|---|---|---|
| `motion` | entrambe | numero dell'animazione |
| `who` | entrambe | `weapon`: evento dell'arma; qualsiasi altro valore (di solito `body`) o assente: evento del personaggio |
| `start`, `end` | `span` | primo e ultimo frame in cui vale; `end` negativo: fino alla fine |
| `frame` | `point` | il frame in cui scatta |
| `trigger` | entrambe | numero dell'evento (0–255). Per le `span` quelli con un significato sono in `catalog::trigger` |
| `params` | entrambe | numeri interi separati da `;`, anche nessuno (`params=`). I pezzi che non sono numeri si saltano. Per una zona il primo è il numero del colpo a segno |

### `attack`: un colpo a segno

| Campo | Significato |
|---|---|
| `id` | numero del colpo a segno |
| `sub_id` | 0 per il primo contatto; i colpi che toccano più volte hanno altre righe per i contatti successivi |
| `type` | 0 normale, 1 perfora la parata, 2 parata, 3 parata totale, altri valori per usi che qui non servono |
| `power` | potenza |
| `element`, `reaction`, `knock1`, `knock2`, `interval`, `revenge`, `combo_group`, `kind` | elemento, tipo di reazione del nemico, spinta (due valori), intervallo tra contatti, valore di Revenge, gruppo, genere (bit 0 finale a terra, bit 1 finale aereo, bit 2 comando di reazione). Si conservano, nessuna regola li usa |

### `ability`: il nome di un'abilità

`id` (numero) e `name` (senza spazi, per esempio `Slide_Dash`).

## 8. Le registrazioni e come rigiocarle

File `kh2_telemetry_*.csv`: una riga per fotogramma del gioco vero. Le righe che cominciano
con `#` sono commenti; la prima riga che non lo è contiene i nomi delle colonne, separati da
virgole. **Non tutti i file hanno tutte le colonne**: un file a cui manca una colonna
necessaria per una verifica va saltato per quella verifica.

| Colonna | Significato |
|---|---|
| `frame` | numero del fotogramma del gioco. Può saltare dei numeri: allora, per ogni numero mancante, si considera una riga uguale alla precedente con quel numero |
| `input` | i tasti, numero esadecimale. **Attacco è giù** quando (valore AND `08000004`) è uguale a `08000004`. **Il tasto del salto è giù** quando il bit `100` è acceso |
| `gamspd` | velocità del gioco: 1 è normale. Se la colonna manca vale 1 |
| `motion` | numero dell'animazione di Sora in quel fotogramma |
| `touch` | 1 se il colpo in corso ha toccato qualcosa |
| `step` | il contatore dei passi della combo (0 fuori da una combo) |
| `fin` | il contatore dei finali |
| `limg`, `lima`, `limf` | i limiti: passi a terra, passi in aria, finali |
| `px`, `py`, `pz` | posizione di Sora. **L'asse `py` cresce verso il basso**: altezza = −`py` |
| `tgt` | il bersaglio: `-1` (o un valore negativo, o `0`) vuol dire nessuno |
| `tx`, `ty`, `tz` | posizione del bersaglio, stessi assi di Sora. Non si sa se sia il punto di mira o la base |
| `hp1`…`hp12`, `thp`, `thpmax` | vita di Sora, dei compagni e dei nemici, e del bersaglio |
| `clock`, `world`, `room`, `btltyp`, `pause`, `cntrl`, `mp1`, `mstate`, `pang`, `str`, `tdef` | non servono per le verifiche |

Tutte le registrazioni sono state fatte senza abilità di combo: limiti 3, 3 e 1.

### Verifica 1: le decisioni della combo (attese: 0 differenze)

Servono le colonne `frame`, `input`, `motion`, `touch`, `step` (e `gamspd` se c'è). Si usa il
catalogo vero e una `Combo` senza abilità. L'idea: a ogni combo del gioco se ne affianca una
della libreria, le si danno le stesse pressioni e gli stessi "ha toccato", le si **impone**
quale colpo parte (con `override_next_move`: la scelta del colpo qui non si verifica, perché
le registrazioni non hanno abbastanza dati sui nemici), e si confronta **se** e **quando** la
combo prosegue, ignora una pressione o finisce.

Si scorrono le righe a coppie (la precedente e la attuale). Per ogni coppia:

- **pressione**: Attacco è giù nella riga attuale e non lo era nella precedente;
- **"è un colpo"**: l'animazione di una riga è un colpo se nel catalogo c'è una voce con
  quell'animazione e di tipo diverso da 0 (parate e comandi non si rigiocano);
- **il gioco ha fatto partire un colpo** in questa riga se: la sua animazione è un colpo,
  `step` non è 0, e rispetto alla riga precedente è cambiata l'animazione oppure è cambiato
  `step`;
- **in aria**: Sora era in aria se l'animazione della riga **precedente** è 3 o 4, oppure è
  l'animazione di una voce "in aria" o di un colpo in volo;
- **ha toccato in questo tick**: `touch` è 1 nella riga attuale e non lo era nella precedente.

Quando **non** si sta seguendo una combo:

| Riga attuale | Cosa si fa |
|---|---|
| il gioco fa partire un colpo con `step` = 1 | si crea una `Combo` nuova, le si impone quell'animazione, le si dà un tick con `attack_pressed` vero e l'`airborne` dedotto: da qui la si segue |
| il gioco fa partire un colpo con `step` diverso da 1 | niente (la combo era già cominciata: non si può seguire) |
| altro | niente |

Quando si sta seguendo una combo:

1. se `gamspd` della riga non è 1 (rallentatore): si smette di seguire questa combo, senza
   contarla come differenza;
2. "il gioco ha lasciato il colpo" se in questa riga il gioco non ha fatto partire un colpo e
   l'animazione della riga (considerata solo se è un colpo) non è quella che la libreria ha in
   corso. Se la riga non è un colpo e la libreria non ha niente in corso, non l'ha lasciato;
3. si impone alla libreria l'animazione del colpo che il gioco ha fatto partire in questa riga
   (nessuna, se non ne ha fatti partire); le si dà un tick con: `attack_pressed` = c'è una
   pressione e il gioco non ha lasciato il colpo; `hit_landed` = ha toccato in questo tick;
   `airborne` dedotto; poi si toglie l'imposizione;
4. si confronta:

| Nel gioco | Nella libreria | Esito |
|---|---|---|
| ha lasciato il colpo | qualsiasi | nessuna differenza; si smette di seguire (se in questo tick la libreria ha dato `ComboEnded`, il colpo è finito nello stesso fotogramma del gioco; altrimenti lo ha interrotto qualcosa: un colpo subito, un salto...) |
| è partito un colpo | è partito un colpo con lo stesso numero di passi (`MoveStarted.step` = `step`) | d'accordo; si continua. In più si controlla che "passi ≥ 3" coincida con "il colpo partito è un finale": se no, **differenza** |
| è partito un colpo | non è partito, o con un altro numero di passi | **differenza**; si smette di seguire |
| non è partito niente | è partito un colpo | **differenza**; si smette di seguire |
| non è partito niente | `ComboEnded` | **differenza**; si smette di seguire |
| non è partito niente | niente, o una pressione ignorata | d'accordo; si continua (conviene contare le pressioni ignorate per motivo) |

Come si contano i casi d'accordo: "prosecuzioni uguali" sono i colpi partiti d'accordo con più
di 1 passo; i colpi partiti d'accordo con passi = 1 mentre si stava già seguendo una combo
(una pressione nel fotogramma esatto in cui un colpo finisce, oppure un colpo in volo fatto
partire da terra) si contano a parte, come "ripartenze uguali", e non tra le combo rigiocate.

Risultati attesi: SPEC 14.

### Verifica 2: i salti

Servono `input`, `motion`, `py`. Un salto comincia in una riga con animazione 3 preceduta da
una riga con un'altra animazione; l'altezza di partenza è quella di quella riga. L'animazione
di salto parte fino a due fotogrammi prima che Sora si stacchi davvero: il confronto comincia
dalla prima riga, ancora con animazione 3, in cui l'altezza supera quella di partenza (se non
c'è, il salto non si confronta). Da lì, per ogni riga finché l'animazione è 3 o 4: un `Air` che
ha appena cominciato un salto avanza di un tick con `jump_held` preso dalla riga e un terreno
lontanissimo verso il basso (così non atterra mai), e si confronta la sua altezza con
l'altezza della riga meno quella di partenza. Per ogni salto si tiene la differenza più grande.

Risultati attesi: SPEC 14 (alcuni salti delle registrazioni sono fatti in salita o finiscono
contro un ostacolo: non possono tornare).

### Verifica 3: lo slancio dei colpi aerei

Servono `motion`, `px`, `pz`, `tgt`, `tx`, `tz`, e il catalogo vero. Si considerano le righe in
cui comincia l'animazione 181 o 182 (riga precedente con un'altra animazione), quando
l'animazione precedente è 3, 4, 181 o 182 (Sora era già in aria) e nella riga precedente c'è
un bersaglio ad almeno 150 unità di distanza orizzontale. La voce è la prima "in aria" con
quell'animazione. Si confronta la strada fatta da Sora in orizzontale per tutta la durata di
quell'animazione (somma degli spostamenti riga per riga) con la strada di uno slancio partito
alla velocità massima della voce, per lo stesso numero di tick.

Risultati attesi: SPEC 14.
