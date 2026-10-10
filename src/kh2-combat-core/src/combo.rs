//! La combo e le reazioni, un tick alla volta (SPEC, sezioni 4, 5, 7, 8 e 10).

use std::sync::Arc;

use crate::air;
use crate::catalog::{motion, trigger, Catalog, Entry, Motion};
use crate::rules::{
    self, FighterView, Loadout, Scene, AERIAL_RECOVERY, COUNTERGUARD, RETALIATING_SLASH,
};

/// Né i passi né i finali superano questo numero (SPEC 5).
const COUNTER_CAP: u8 = 100;
/// Il tipo di colpo a segno "parata" (la "parata totale" è un altro tipo).
const PLAIN_GUARD: u8 = 2;

/// Ciò che chi usa la libreria dice a ogni tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TickInput<'a> {
    /// Attacco è passato da rilasciato a premuto in questo tick.
    pub attack_pressed: bool,
    /// Un comando: il numero della voce da richiamare.
    pub command: Option<u8>,
    /// Il tasto del ritorno in equilibrio è stato premuto in questo tick.
    pub recover_pressed: bool,
    /// L'animazione in corso ha toccato qualcosa in questo tick.
    pub hit_landed: bool,
    /// Il personaggio non può agire.
    pub busy: bool,
    /// Il personaggio è in aria.
    pub airborne: bool,
    /// La condizione di Berserk Charge è vera.
    pub berserk: bool,
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
    /// Lo emette solo `Fighter`.
    Jumped,
    /// Lo emette solo `Fighter`.
    Landed,
}

/// L'animazione in corso: un colpo del repertorio oppure una reazione.
#[derive(Clone, Debug)]
struct Running {
    motion: u16,
    /// Il frame, da 1 (zero solo nell'istante in cui l'animazione viene creata).
    frame: u16,
    /// La posizione della voce nel repertorio; `None` per una reazione.
    entry: Option<usize>,
    /// "Ha toccato".
    touched: bool,
    /// Le zone attive in questo frame.
    zones: Zones,
}

/// Le zone attive in un frame di un'animazione (INTERFACCIA, "Le zone attive"): in uno
/// stesso frame possono esserci insieme una zona di danno e una parata.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Zones {
    /// La zona di danno attiva: (numero del colpo a segno, potenza).
    damage: Option<(u16, u16)>,
    /// La prima zona di parata che copre il frame.
    guard: Option<u16>,
    /// Tra le zone di parata ce n'è almeno una di tipo "parata" (e non "parata totale").
    plain_guard: bool,
}

impl Zones {
    /// Le zone attive a quel frame di quell'animazione.
    fn at(catalog: &Catalog, data: Option<&Motion>, frame: u16) -> Zones {
        let mut zones = Zones::default();
        let Some(data) = data else {
            return zones;
        };
        for attack in data.zones(frame) {
            let row = catalog.attack(attack);
            if let Some(row) = row.filter(|row| row.guards()) {
                zones.guard.get_or_insert(attack);
                zones.plain_guard |= row.attack_type == PLAIN_GUARD;
            } else {
                // Di danno; se il numero non è nel catalogo la potenza vale 0. Tra le zone
                // di danno vince la potenza più alta e, a parità, la prima.
                let power = row.map_or(0, |row| row.power);
                if zones.damage.map_or(true, |(_, best)| power > best) {
                    zones.damage = Some((attack, power));
                }
            }
        }
        zones
    }

    /// La zona indicata: quella di danno attiva, se c'è; altrimenti la prima di parata.
    fn indicated(&self) -> Option<u16> {
        self.damage.map(|(attack, _)| attack).or(self.guard)
    }
}

/// La combo di un personaggio.
///
/// SPEC 13 (da NON riprodurre): non c'è nessuna memoria delle pressioni fatte in anticipo.
/// Una pressione che non vale è persa.
#[derive(Clone, Debug)]
pub struct Combo {
    /// Le abilità equipaggiate: si possono cambiare in qualsiasi momento.
    pub loadout: Loadout,
    catalog: Arc<Catalog>,
    running: Option<Running>,
    steps: u8,
    finishers: u8,
    launched: bool,
    /// L'animazione imposta al prossimo colpo che partirà.
    forced: Option<u16>,
}

impl Combo {
    pub fn new(catalog: Arc<Catalog>, loadout: Loadout) -> Self {
        Combo {
            loadout,
            catalog,
            running: None,
            steps: 0,
            finishers: 0,
            launched: false,
            forced: None,
        }
    }

    /// Il catalogo con cui è stata creata.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    /// Impone l'animazione del prossimo colpo che partirà (serve a rigiocare le
    /// registrazioni); `None` toglie l'imposizione.
    pub fn override_next_move(&mut self, motion: Option<u16>) {
        self.forced = motion;
    }

    /// La voce del colpo in corso; `None` se non c'è niente in corso o se è una reazione.
    pub fn current_entry(&self) -> Option<&Entry> {
        let index = self.running.as_ref()?.entry?;
        self.catalog.entries.get(index)
    }

    /// (Numero dell'animazione in corso, frame da 1), colpo o reazione.
    pub fn current_move(&self) -> Option<(u16, u16)> {
        self.running.as_ref().map(|running| (running.motion, running.frame))
    }

    /// La potenza della zona di danno attiva; `None` se non ce n'è una (anche se c'è una
    /// parata). Vale 0 se quel colpo a segno non è nel catalogo.
    pub fn hit_power(&self) -> Option<u16> {
        let (_, power) = self.running.as_ref()?.zones.damage?;
        Some(power)
    }

    /// Il numero del colpo a segno della zona indicata in questo tick: la zona di danno
    /// attiva, se c'è; altrimenti la prima zona di parata; altrimenti niente.
    pub fn hit_attack(&self) -> Option<u16> {
        self.running.as_ref()?.zones.indicated()
    }

    /// Vero se in questo tick c'è una parata attiva, dei due tipi (anche se insieme c'è una
    /// zona di danno).
    pub fn guarding(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|running| running.zones.guard.is_some())
    }

    /// Vero se l'animazione in corso ha "non colpibile" attivo in questo frame.
    pub fn unhittable(&self) -> bool {
        self.active_now(trigger::UNHITTABLE)
    }

    /// Vero se non c'è nessuna animazione in corso, oppure se quella in corso ha "comando
    /// di reazione permesso" attivo in questo frame.
    pub fn reaction_allowed(&self) -> bool {
        self.running.is_none() || self.active_now(trigger::REACTION_ALLOWED)
    }

    /// Vero se non c'è nessuna animazione in corso, oppure se quella in corso è neutra in
    /// questo frame.
    pub fn free(&self) -> bool {
        match &self.running {
            None => true,
            Some(running) => self
                .catalog
                .motion(running.motion)
                .is_some_and(|data| data.neutral(running.frame)),
        }
    }

    /// Vero se il personaggio è "sbalzato" (SPEC 10).
    pub fn launched(&self) -> bool {
        self.launched
    }

    /// I passi; 0 se non c'è nessuna animazione in corso.
    pub fn step(&self) -> u8 {
        if self.running.is_some() {
            self.steps
        } else {
            0
        }
    }

    /// I finali; 0 se non c'è nessuna animazione in corso.
    pub fn finishers(&self) -> u8 {
        if self.running.is_some() {
            self.finishers
        } else {
            0
        }
    }

    /// L'animazione in corso finisce; lo sbalzo finisce.
    pub fn interrupt(&mut self) -> Vec<Event> {
        let mut events = Vec::new();
        self.finish(&mut events);
        self.launched = false;
        events
    }

    /// L'arma è stata respinta da una parata nemica (SPEC 10).
    pub fn deflected(&mut self, airborne: bool) -> Vec<Event> {
        let reaction = if airborne {
            motion::DEFLECT_AIR
        } else {
            motion::DEFLECT
        };
        self.bounce(reaction)
    }

    /// Un colpo nemico è finito sulla parata (SPEC 10). Vale solo se in questo momento è
    /// attiva almeno una zona di tipo "parata". Con la sola "parata totale" (quella che sta
    /// dentro altre animazioni: ritorno in equilibrio, Counterguard...) il colpo nemico è
    /// fermato ma per il personaggio non cambia niente (R10).
    pub fn guarded(&mut self) -> Vec<Event> {
        let plain_guard = self
            .running
            .as_ref()
            .is_some_and(|running| running.zones.plain_guard);
        if !plain_guard {
            return Vec::new();
        }
        self.bounce(motion::GUARD_SUCCESS)
    }

    /// Il personaggio ha subito un colpo (SPEC 10): la combo si rompe e parte l'animazione
    /// di danno indicata; `launched`: il colpo lo ha sbalzato in aria.
    pub fn knocked(&mut self, reaction: u16, launched: bool) -> Vec<Event> {
        let mut events = Vec::new();
        self.steps = 0;
        self.finishers = 1;
        self.start_reaction(reaction, false, &mut events);
        self.launched = launched;
        events
    }

    /// Avanza di un tick.
    pub fn tick(&mut self, input: TickInput) -> Vec<Event> {
        let mut events = Vec::new();

        // 1. L'animazione in corso avanza.
        self.advance(&mut events);

        // 2. "Ha toccato" di un tick vale già per la pressione dello stesso tick.
        if input.hit_landed {
            if let Some(running) = self.running.as_mut() {
                running.touched = true;
            }
        }

        // 3. Ritorno in equilibrio: il tick finisce qui, un'eventuale pressione è persa.
        if input.recover_pressed
            && self.launched
            && !input.busy
            && self.loadout.has(AERIAL_RECOVERY)
        {
            self.launched = false;
            self.steps = 0;
            self.finishers = 1;
            self.start_reaction(motion::AERIAL_RECOVERY, false, &mut events);
            return events;
        }

        // 4. Pressione di Attacco oppure comando (se ci sono tutti e due vale il comando).
        if input.attack_pressed || input.command.is_some() {
            if !input.busy {
                if let Some(called) = self.called_by_situation(&input) {
                    // Parte subito, senza finestre e senza "ha toccato"; se non può
                    // partire i contatori tornano com'erano e la pressione viene
                    // giudicata come una qualsiasi.
                    let before = (self.steps, self.finishers);
                    self.steps = bump(self.steps);
                    self.finishers = self.finishers.max(1);
                    if self.start_move(&input, Some(called), false, &mut events) {
                        return events;
                    }
                    (self.steps, self.finishers) = before;
                }
            }
            let before = (self.steps, self.finishers);
            match self.judge(input.busy) {
                Ok(finisher_window) => {
                    if !self.start_move(&input, input.command, finisher_window, &mut events) {
                        (self.steps, self.finishers) = before;
                        events.push(Event::PressIgnored(IgnoreReason::Unavailable));
                    }
                }
                Err(reason) => events.push(Event::PressIgnored(reason)),
            }
        }
        events
    }

    /// La voce che la situazione richiama con questa pressione o questo comando (SPEC 8),
    /// se ce n'è una. Le due situazioni (sbalzato, parata riuscita) si guardano ognuna per
    /// conto suo: la voce di una situazione la richiamano Attacco oppure il comando con
    /// proprio quel numero; un comando con un altro numero, senza Attacco, no (R3).
    fn called_by_situation(&self, input: &TickInput) -> Option<u8> {
        let launched = self.launched;
        let guard_success = self
            .running
            .as_ref()
            .is_some_and(|running| running.motion == motion::GUARD_SUCCESS);
        if input.attack_pressed {
            // Con Attacco (con o senza un comando), se valgono entrambe vince lo sbalzo: si
            // prova solo la sua voce (R12).
            if launched {
                Some(RETALIATING_SLASH)
            } else {
                guard_success.then_some(COUNTERGUARD)
            }
        } else {
            match input.command {
                Some(RETALIATING_SLASH) if launched => Some(RETALIATING_SLASH),
                Some(COUNTERGUARD) if guard_success => Some(COUNTERGUARD),
                _ => None,
            }
        }
    }

    /// Il giudizio di una pressione (INTERFACCIA, sezione 4). Se vale aggiorna i contatori
    /// e dice se l'animazione in corso era nella finestra "finale permesso".
    fn judge(&mut self, busy: bool) -> Result<bool, IgnoreReason> {
        if busy {
            return Err(IgnoreReason::Busy);
        }
        let Some(running) = &self.running else {
            self.steps = 1;
            self.finishers = 1;
            return Ok(false);
        };
        let data = self.catalog.motion(running.motion);
        let frame = running.frame;
        let active = |number: u8| data.is_some_and(|data| data.active(number, frame));
        let first_frame = |number: u8| data.and_then(|data| data.first_frame_of(number));
        let opens_later = |number: u8| first_frame(number).is_some_and(|first| frame < first);

        let finisher_window = active(trigger::FINISHER_ALLOWED);
        let combo_window = active(trigger::COMBO_ALLOWED);
        // "Ha toccato" comprende Combo Master.
        let touched = running.touched || self.loadout.combo_master;
        let more_finishers = self.finishers < self.loadout.limits().finishers;

        if finisher_window && touched && more_finishers {
            self.steps = bump(self.steps);
            self.finishers = bump(self.finishers);
            return Ok(true);
        }
        if combo_window && touched {
            self.steps = bump(self.steps);
            return Ok(finisher_window);
        }
        if data.is_some_and(|data| data.neutral(frame)) {
            self.steps = 1;
            self.finishers = 1;
            return Ok(finisher_window);
        }
        if combo_window || (finisher_window && more_finishers) {
            return Err(IgnoreReason::NotTouched);
        }
        let finisher = running
            .entry
            .and_then(|index| self.catalog.entries.get(index))
            .is_some_and(|entry| entry.is_finisher());
        if finisher {
            if more_finishers && opens_later(trigger::FINISHER_ALLOWED) {
                return Err(IgnoreReason::TooEarly);
            }
            return Err(IgnoreReason::ComboOver);
        }
        // Colpo normale o reazione: la finestra "combo permessa" si apre più avanti, oppure
        // non c'è proprio.
        match first_frame(trigger::COMBO_ALLOWED) {
            Some(first) if frame >= first => Err(IgnoreReason::ComboOver),
            _ => Err(IgnoreReason::TooEarly),
        }
    }

    /// Prova a far partire un colpo (INTERFACCIA, "Far partire un colpo"); `command`: la
    /// voce richiamata, se la partenza viene da un comando. Dice se è partito.
    fn start_move(
        &mut self,
        input: &TickInput,
        command: Option<u8>,
        finisher_window: bool,
        events: &mut Vec<Event>,
    ) -> bool {
        let catalog = Arc::clone(&self.catalog);
        let view = FighterView {
            airborne: input.airborne,
            current_motion: self.running.as_ref().map(|running| running.motion),
            steps: self.steps,
            finishers: self.finishers,
            finisher_window,
            // SPEC 13 (da NON riprodurre): la dote che allunga la portata verso l'alto
            // esiste nell'interfaccia, ma la combo la passa sempre a zero.
            reach_bonus: 0.0,
            berserk: input.berserk,
        };
        let chosen = if let Some(imposed) = self.forced.take() {
            // Il colpo imposto vale per una sola partenza, anche se non trova nessuna voce.
            let entries = &catalog.entries;
            entries
                .iter()
                .position(|entry| entry.motion == imposed && entry.is_aerial() == input.airborne)
                .or_else(|| entries.iter().position(|entry| entry.motion == imposed))
        } else if let Some(id) = command {
            rules::by_command_index(&catalog, &self.loadout, &view, id)
        } else {
            rules::choose_index(&catalog, &self.loadout, &view, &input.scene)
        };
        let Some(index) = chosen else {
            return false;
        };
        let entry = &catalog.entries[index];

        if self.hit_attack().is_some() {
            events.push(Event::HitWindowClosed);
        }
        // Un colpo in volo che parte da terra è il primo colpo di una combo aerea.
        if entry.lunges() && !input.airborne {
            self.steps = 1;
            self.finishers = 1;
        }
        self.launched = false;
        self.running = Some(Running {
            motion: entry.motion,
            frame: 0,
            entry: Some(index),
            touched: false,
            zones: Zones::default(),
        });
        events.push(Event::MoveStarted {
            entry: entry.id,
            motion: entry.motion,
            step: self.steps,
        });
        if entry.lunges() {
            events.push(lunge(entry, &input.scene));
        }
        self.enter_next_frame(events);
        true
    }

    /// Fa partire una reazione (INTERFACCIA, "Far partire una reazione").
    fn start_reaction(&mut self, reaction: u16, touched: bool, events: &mut Vec<Event>) {
        if self.catalog.motion(reaction).is_none() {
            // Il catalogo non la conosce: l'animazione in corso finisce e non parte niente.
            self.finish(events);
            return;
        }
        if self.hit_attack().is_some() {
            events.push(Event::HitWindowClosed);
        }
        self.running = Some(Running {
            motion: reaction,
            frame: 0,
            entry: None,
            touched,
            zones: Zones::default(),
        });
        events.push(Event::ReactionStarted { motion: reaction });
        self.enter_next_frame(events);
    }

    /// Parata riuscita e arma respinta: "ha toccato" diventa vero, i passi scendono di 1, e
    /// parte la reazione se il catalogo la conosce (altrimenti l'animazione in corso prosegue).
    fn bounce(&mut self, reaction: u16) -> Vec<Event> {
        let mut events = Vec::new();
        let Some(running) = self.running.as_mut() else {
            return events;
        };
        running.touched = true;
        self.steps = self.steps.saturating_sub(1);
        if self.catalog.motion(reaction).is_some() {
            self.start_reaction(reaction, true, &mut events);
        }
        events
    }

    /// L'animazione in corso avanza di un frame, oppure finisce (SPEC 4: l'ultimo frame non
    /// si vede mai).
    fn advance(&mut self, events: &mut Vec<Event>) {
        let Some(running) = &self.running else {
            return;
        };
        let lasts = self
            .catalog
            .motion(running.motion)
            .is_some_and(|data| u32::from(running.frame) + 1 < u32::from(data.frames));
        if lasts {
            self.enter_next_frame(events);
        } else {
            self.finish(events);
        }
    }

    /// Porta l'animazione in corso al frame successivo e dice cosa cambia: la zona indicata
    /// e l'apertura di una finestra. Un'animazione appena creata arriva così al frame 1.
    fn enter_next_frame(&mut self, events: &mut Vec<Event>) {
        let catalog = Arc::clone(&self.catalog);
        let Some(running) = self.running.as_mut() else {
            return;
        };
        let data = catalog.motion(running.motion);
        // Prima del frame 1 non c'era niente: né zona né finestra.
        let was_open = running.frame >= 1 && chain_open(data, running.frame);
        running.frame = running.frame.saturating_add(1);

        // Gli eventi seguono la zona indicata: una parata che comincia o finisce mentre
        // resta indicata la stessa zona di danno non dà eventi (la si vede con `guarding`).
        let zones = Zones::at(&catalog, data, running.frame);
        let (before, now) = (running.zones.indicated(), zones.indicated());
        if now != before {
            if before.is_some() {
                events.push(Event::HitWindowClosed);
            }
            if let Some(attack) = now {
                let power = catalog.attack(attack).map_or(0, |row| row.power);
                events.push(Event::HitWindowOpened { attack, power });
            }
        }
        running.zones = zones;
        if !was_open && chain_open(data, running.frame) {
            events.push(Event::ChainOpened);
        }
    }

    /// L'animazione in corso finisce: non c'è più niente in corso e lo sbalzo finisce.
    fn finish(&mut self, events: &mut Vec<Event>) {
        let Some(running) = self.running.take() else {
            return;
        };
        if running.zones.indicated().is_some() {
            events.push(Event::HitWindowClosed);
        }
        events.push(Event::ComboEnded);
        self.launched = false;
    }

    /// Vero se l'animazione in corso ha quell'evento del personaggio attivo in questo frame.
    fn active_now(&self, number: u8) -> bool {
        self.running.as_ref().is_some_and(|running| {
            self.catalog
                .motion(running.motion)
                .is_some_and(|data| data.active(number, running.frame))
        })
    }
}

/// Un contatore sale di 1, senza superare il tetto.
fn bump(counter: u8) -> u8 {
    counter.saturating_add(1).min(COUNTER_CAP)
}

/// È aperta la finestra "combo permessa" o "finale permesso".
fn chain_open(data: Option<&Motion>, frame: u16) -> bool {
    data.is_some_and(|data| {
        data.active(trigger::COMBO_ALLOWED, frame) || data.active(trigger::FINISHER_ALLOWED, frame)
    })
}

/// Porta un valore tra un minimo e un massimo; se il minimo fosse maggiore del massimo
/// vale il più grande dei due.
fn bounded(value: f32, min: f32, max: f32) -> f32 {
    if min > max {
        min
    } else {
        value.max(min).min(max)
    }
}

/// Balzo e slancio di un colpo in volo che parte adesso (SPEC 7).
fn lunge(entry: &Entry, scene: &Scene) -> Event {
    let Some(target) = scene.main_target else {
        return Event::Lunge {
            rise: entry.jump,
            run: 0.0,
            speed: bounded(entry.speed_max, entry.speed_min, entry.speed_max),
        };
    };
    // Quanto il punto di mira del bersaglio sta più in alto dei piedi.
    let climb = target.position[1] - scene.fighter[1];
    // Una voce "a terra" guarda il dislivello solo se i due limiti scritti nella voce sono
    // diversi. Se sono uguali (il colpo del bordo li ha tutti e due a zero) la salita si
    // calcola come per una voce "in aria" (R8).
    let watches_height = !entry.is_aerial() && entry.low != entry.high;
    let (lowest, highest) = if watches_height {
        // Gli stessi limiti di dislivello della scelta, con la taglia e senza la dote.
        rules::rise_limits(entry, target.size, 0.0)
    } else {
        (entry.jump_min, entry.jump_max)
    };
    let rise = if lowest > highest {
        // "Almeno il salto minimo": è sempre il campo `jump_min` della voce, anche per una
        // voce a terra (R8 b).
        climb.max(entry.jump_min)
    } else {
        climb.max(lowest).min(highest)
    };

    let run = rules::gap(scene.fighter, &target);
    // La velocità che porterebbe al bordo del bersaglio quando finisce la salita.
    let reach = air::drift_reach(air::rise_ticks(rise));
    let needed = if run <= 0.0 {
        0.0
    } else if reach > 0.0 {
        run / reach
    } else {
        // Salita nulla: non c'è tempo per arrivare e quella velocità non esiste. Con della
        // strada da fare vale la velocità massima della voce (R8 a): un valore senza
        // limite, portato qui sotto tra minimo e massimo, dà proprio quella.
        f32::INFINITY
    };
    // SPEC 13 (da NON riprodurre): la velocità minima negativa di alcune voci farebbe
    // allontanare il personaggio dal bersaglio. Qui la velocità che serve non è mai
    // sotto zero, quindi un minimo negativo non ha effetto e lo slancio non va mai
    // all'indietro (salvo una voce con il massimo negativo).
    let speed = bounded(needed, entry.speed_min, entry.speed_max);
    Event::Lunge { rise, run, speed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Body;

    /// Un catalogo inventato, piccolo, con un colpo per ogni caso che serve ai test.
    ///
    /// Voci a terra: 5 (animazione 100, 20 frame, zona 7 ai frame 4-6, combo dal 10),
    /// 6 (101, alternativa della 5), finale 3 (102, finale permesso 8-14), parata 11
    /// (103, zona di parata 3-9), 39 e 40 richiamate dalla situazione, 12 un comando.
    /// Voci in aria: 9 (110, colpo in volo) e il suo finale 8 (111).
    ///
    /// In fondo, richiamate solo con il loro comando: 14 e 17 (zone sovrapposte), 1 (il
    /// colpo del bordo), 15, 16 e 18 (colpi in volo con limiti rovesciati o nulli).
    const TEXT: &str = "\
entry id=3 type=3 sub=-1 combo_offset=0 flag=4 motion=102 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=20 type=1 sub=-1 combo_offset=0 flag=0 motion=120 next_motion=4 jump=17 jump_max=0 jump_min=17 speed_min=-8 speed_max=8 near=0 far=260 low=-110 high=-250 inner_min=0.9 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=5 type=3 sub=6 combo_offset=0 flag=0 motion=100 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=6 type=0 sub=-1 combo_offset=0 flag=0 motion=101 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=8 type=3 sub=-1 combo_offset=0 flag=5 motion=111 next_motion=4 jump=21 jump_max=71 jump_min=21 speed_min=12 speed_max=18 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=9 type=3 sub=9 combo_offset=0 flag=1 motion=110 next_motion=4 jump=21 jump_max=71 jump_min=21 speed_min=-8 speed_max=8 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=11 type=0 sub=-1 combo_offset=0 flag=0 motion=103 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=12 type=0 sub=-1 combo_offset=0 flag=0 motion=104 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=60 score=0
entry id=39 type=0 sub=-1 combo_offset=0 flag=0 motion=105 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=96 score=0
entry id=40 type=0 sub=-1 combo_offset=0 flag=1 motion=112 next_motion=4 jump=21 jump_max=121 jump_min=21 speed_min=0 speed_max=18 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=101 score=0
entry id=13 type=0 sub=-1 combo_offset=0 flag=0 motion=999 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=14 type=0 sub=-1 combo_offset=0 flag=0 motion=106 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=17 type=0 sub=-1 combo_offset=0 flag=0 motion=107 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=1 type=0 sub=-1 combo_offset=0 flag=0 motion=121 next_motion=4 jump=21 jump_max=71 jump_min=21 speed_min=-8 speed_max=8 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=15 type=0 sub=-1 combo_offset=0 flag=1 motion=113 next_motion=4 jump=30 jump_max=10 jump_min=40 speed_min=0 speed_max=8 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=16 type=0 sub=-1 combo_offset=0 flag=1 motion=114 next_motion=4 jump=5 jump_max=0 jump_min=0 speed_min=2 speed_max=9 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=18 type=0 sub=-1 combo_offset=0 flag=1 motion=115 next_motion=4 jump=20 jump_max=20 jump_min=20 speed_min=10 speed_max=4 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
motion id=115 name=VELOCITA_ROVESCIATE frames=18
motion id=106 name=PARA_E_COLPISCE frames=30
span motion=106 who=weapon start=1 end=2 trigger=10 params=77;1
span motion=106 who=weapon start=3 end=20 trigger=10 params=161;2
span motion=106 who=weapon start=10 end=15 trigger=33 params=7;1
span motion=106 who=body start=3 end=20 trigger=10 params=124;6
span motion=106 who=body start=8 end=18 trigger=10 params=8;1
motion id=107 name=PARITA frames=20
span motion=107 who=body start=1 end=9 trigger=10 params=88;1
span motion=107 who=weapon start=1 end=9 trigger=10 params=8;1
span motion=107 who=weapon start=3 end=6 trigger=10 params=161;2
motion id=113 name=LIMITI_ROVESCIATI frames=18
motion id=114 name=SENZA_SALITA frames=18
motion id=121 name=BORDO frames=18
motion id=100 name=A frames=20
span motion=100 who=body start=10 end=-1 trigger=11 params=
span motion=100 who=body start=2 end=12 trigger=14 params=
span motion=100 who=weapon start=4 end=6 trigger=10 params=7;1
motion id=101 name=C frames=30
span motion=101 who=body start=15 end=-1 trigger=11 params=
span motion=101 who=weapon start=5 end=8 trigger=10 params=7;1
span motion=101 who=weapon start=9 end=12 trigger=10 params=8;1
motion id=102 name=FINALE frames=24
span motion=102 who=body start=8 end=14 trigger=50 params=
span motion=102 who=body start=3 end=5 trigger=27 params=
motion id=103 name=PARATA frames=16
span motion=103 who=weapon start=3 end=9 trigger=10 params=161;2
span motion=103 who=body start=13 end=-1 trigger=2 params=
motion id=104 name=COMANDO frames=12
motion id=105 name=CONTRATTACCO frames=12
span motion=105 who=body start=0 end=8 trigger=10 params=124;6
span motion=105 who=body start=6 end=-1 trigger=11 params=
motion id=110 name=ARIA frames=18
span motion=110 who=body start=8 end=-1 trigger=11 params=
span motion=110 who=weapon start=1 end=4 trigger=10 params=9
motion id=111 name=FINALE_ARIA frames=18
span motion=111 who=body start=6 end=12 trigger=50 params=
motion id=112 name=RITORSIONE frames=18
motion id=120 name=VERSO_L_ALTO frames=18
motion id=41 name=RESPINTA frames=14
span motion=41 who=body start=5 end=-1 trigger=11 params=
span motion=41 who=body start=11 end=-1 trigger=2 params=
motion id=42 name=PARATA_RIUSCITA frames=14
span motion=42 who=weapon start=0 end=6 trigger=10 params=162;2
span motion=42 who=body start=9 end=-1 trigger=2 params=
motion id=40 name=EQUILIBRIO frames=10
span motion=40 who=body start=0 end=4 trigger=10 params=124;6
span motion=40 who=body start=0 end=4 trigger=27 params=
span motion=40 who=body start=6 end=-1 trigger=1 params=
motion id=4 name=CADUTA frames=8
span motion=4 who=body start=0 end=-1 trigger=1 params=
motion id=50 name=DANNO frames=12
attack sub_id=0 id=7 type=0 power=100 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=0 id=8 type=0 power=33 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=0 id=161 type=2 power=0 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=0 id=162 type=2 power=5 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=0 id=88 type=0 power=33 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=0 id=124 type=3 power=0 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
";

    fn combo_with(loadout: Loadout) -> Combo {
        Combo::new(Arc::new(Catalog::parse(TEXT).unwrap()), loadout)
    }

    fn combo() -> Combo {
        combo_with(Loadout::default())
    }

    fn idle<'a>() -> TickInput<'a> {
        TickInput::default()
    }

    fn press<'a>() -> TickInput<'a> {
        TickInput { attack_pressed: true, ..TickInput::default() }
    }

    fn hit<'a>() -> TickInput<'a> {
        TickInput { hit_landed: true, ..TickInput::default() }
    }

    fn command<'a>(id: u8) -> TickInput<'a> {
        TickInput { command: Some(id), ..TickInput::default() }
    }

    fn in_air<'a>(input: TickInput<'a>) -> TickInput<'a> {
        TickInput { airborne: true, ..input }
    }

    /// Fa passare dei tick senza tasti e raccoglie gli eventi.
    fn wait(combo: &mut Combo, ticks: usize) -> Vec<Event> {
        (0..ticks).flat_map(|_| combo.tick(idle())).collect()
    }

    /// Fa passare tick senza tasti finché l'animazione in corso arriva a quel frame.
    fn wait_frame(combo: &mut Combo, frame: u16) {
        while combo.current_move().expect("serve un'animazione in corso").1 < frame {
            combo.tick(idle());
        }
        assert_eq!(combo.current_move().unwrap().1, frame);
    }

    fn started(events: &[Event]) -> Option<(u8, u16, u8)> {
        events.iter().find_map(|event| match event {
            Event::MoveStarted { entry, motion, step } => Some((*entry, *motion, *step)),
            _ => None,
        })
    }

    fn ignored(events: &[Event]) -> Option<IgnoreReason> {
        events.iter().find_map(|event| match event {
            Event::PressIgnored(reason) => Some(*reason),
            _ => None,
        })
    }

    #[test]
    fn una_combo_nuova_non_ha_niente_in_corso() {
        let combo = combo();
        assert!(combo.current_move().is_none() && combo.current_entry().is_none());
        assert!(combo.hit_attack().is_none() && combo.hit_power().is_none());
        assert!(!combo.guarding() && !combo.unhittable() && !combo.launched());
        assert!(combo.reaction_allowed() && combo.free());
        assert_eq!((combo.step(), combo.finishers()), (0, 0));
        assert_eq!(combo.catalog().entries.len(), 17);
    }

    #[test]
    fn senza_tasti_non_succede_niente() {
        let mut combo = combo();
        assert!(wait(&mut combo, 5).is_empty());
    }

    #[test]
    fn la_prima_pressione_apre_la_combo_e_il_colpo_e_gia_al_frame_uno() {
        let mut combo = combo();
        let events = combo.tick(press());
        assert_eq!(events, [Event::MoveStarted { entry: 5, motion: 100, step: 1 }]);
        assert_eq!(combo.current_move(), Some((100, 1)));
        assert_eq!(combo.current_entry().unwrap().id, 5);
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
        assert!(!combo.free());
    }

    #[test]
    fn un_animazione_da_n_frame_occupa_n_meno_uno_tick() {
        let mut combo = combo();
        combo.tick(press());
        // 20 frame: il tick di partenza più altri 18, fino al frame 19.
        for expected in 2..=19 {
            let events = combo.tick(idle());
            assert!(!events.contains(&Event::ComboEnded));
            assert_eq!(combo.current_move(), Some((100, expected)));
        }
        let events = combo.tick(idle());
        assert_eq!(events, [Event::ComboEnded]);
        assert!(combo.current_move().is_none());
        assert_eq!((combo.step(), combo.finishers()), (0, 0));
    }

    #[test]
    fn una_pressione_nel_tick_in_cui_l_animazione_finisce_apre_una_combo_nuova() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 19);
        let events = combo.tick(press());
        assert_eq!(
            events,
            [Event::ComboEnded, Event::MoveStarted { entry: 5, motion: 100, step: 1 }]
        );
        assert_eq!(combo.current_move(), Some((100, 1)));
    }

    #[test]
    fn eventi_della_zona_e_della_finestra() {
        let mut combo = combo();
        combo.tick(press());
        let mut log = Vec::new();
        for _ in 0..18 {
            let frame = combo.current_move().unwrap().1 + 1;
            for event in combo.tick(idle()) {
                log.push((frame, event));
            }
        }
        assert_eq!(
            log,
            [
                (4, Event::HitWindowOpened { attack: 7, power: 100 }),
                (7, Event::HitWindowClosed),
                (10, Event::ChainOpened),
            ]
        );
    }

    #[test]
    fn zona_attiva_e_potenza_tick_per_tick() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 3);
        assert_eq!((combo.hit_attack(), combo.hit_power()), (None, None));
        wait_frame(&mut combo, 4);
        assert_eq!((combo.hit_attack(), combo.hit_power()), (Some(7), Some(100)));
        wait_frame(&mut combo, 6);
        assert_eq!(combo.hit_attack(), Some(7));
        wait_frame(&mut combo, 7);
        assert_eq!(combo.hit_attack(), None);
    }

    #[test]
    fn due_zone_una_dopo_l_altra_chiudono_e_riaprono() {
        let mut combo = combo();
        combo.override_next_move(Some(101));
        combo.tick(press());
        let events = wait(&mut combo, 12);
        assert_eq!(
            events,
            [
                Event::HitWindowOpened { attack: 7, power: 100 },
                Event::HitWindowClosed,
                Event::HitWindowOpened { attack: 8, power: 33 },
                Event::HitWindowClosed,
            ]
        );
    }

    #[test]
    fn una_zona_gia_al_frame_uno_si_apre_alla_partenza() {
        let mut combo = combo();
        let events = combo.tick(in_air(press()));
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], Event::MoveStarted { entry: 9, motion: 110, step: 1 });
        assert!(matches!(events[1], Event::Lunge { .. }));
        // Il colpo a segno 9 non è nel catalogo: potenza 0.
        assert_eq!(events[2], Event::HitWindowOpened { attack: 9, power: 0 });
        assert_eq!(combo.hit_power(), Some(0));
        assert!(!combo.guarding());
    }

    #[test]
    fn prima_della_finestra_la_pressione_e_troppo_presto() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 8);
        // Tick successivo: frame 9, la finestra si apre al 10.
        let events = combo.tick(TickInput { hit_landed: true, ..press() });
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::TooEarly)]);
        assert_eq!(combo.step(), 1);
    }

    #[test]
    fn il_frame_da_cui_si_prosegue_e_compreso() {
        let mut combo = combo();
        combo.tick(press());
        combo.tick(hit());
        wait_frame(&mut combo, 9);
        // La pressione si giudica dopo l'avanzamento: questo tick è il frame 10.
        let events = combo.tick(press());
        assert_eq!(started(&events), Some((6, 101, 2)));
        assert_eq!(combo.step(), 2);
    }

    #[test]
    fn nella_finestra_senza_aver_toccato_la_pressione_e_persa() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 12);
        let events = combo.tick(press());
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::NotTouched)]);
        // Nessuna memoria: toccare dopo non fa partire niente da solo.
        let events = combo.tick(hit());
        assert!(events.is_empty());
        assert_eq!(combo.current_move().unwrap().0, 100);
    }

    #[test]
    fn ha_toccato_vale_gia_per_la_pressione_dello_stesso_tick() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 11);
        let events = combo.tick(TickInput { hit_landed: true, ..press() });
        assert_eq!(started(&events), Some((6, 101, 2)));
    }

    #[test]
    fn ogni_colpo_comincia_senza_aver_toccato() {
        let mut combo = combo();
        combo.tick(press());
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        assert_eq!(started(&combo.tick(press())), Some((6, 101, 2)));
        wait_frame(&mut combo, 20);
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::NotTouched));
    }

    #[test]
    fn combo_master_vale_sempre_come_ha_toccato() {
        let mut loadout = Loadout::default();
        loadout.combo_master = true;
        let mut combo = combo_with(loadout);
        combo.tick(press());
        wait_frame(&mut combo, 10);
        assert_eq!(started(&combo.tick(press())), Some((6, 101, 2)));
    }

    /// Porta la combo a terra fino al finale (voci 5, 6, 3) e lo restituisce in corso.
    fn at_finisher(loadout: Loadout) -> Combo {
        let mut combo = combo_with(loadout);
        combo.tick(press());
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        assert_eq!(started(&combo.tick(press())), Some((6, 101, 2)));
        combo.tick(hit());
        wait_frame(&mut combo, 15);
        assert_eq!(started(&combo.tick(press())), Some((3, 102, 3)));
        combo
    }

    #[test]
    fn senza_bersagli_la_combo_e_primo_colpo_alternativa_finale() {
        let combo = at_finisher(Loadout::default());
        assert!(combo.current_entry().unwrap().is_finisher());
        assert_eq!((combo.step(), combo.finishers()), (3, 1));
    }

    #[test]
    fn dopo_il_finale_la_combo_e_finita() {
        let mut combo = at_finisher(Loadout::default());
        combo.tick(hit());
        // Con un massimo di 1 finale la finestra "finale permesso" non serve a niente.
        for frame in 3..=23 {
            let events = combo.tick(press());
            assert_eq!(ignored(&events), Some(IgnoreReason::ComboOver), "frame {frame}");
            assert!(started(&events).is_none());
        }
        // Finito il finale, la pressione apre una combo nuova.
        let events = combo.tick(press());
        assert_eq!(started(&events), Some((5, 100, 1)));
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
    }

    #[test]
    fn finishing_plus_concatena_un_altro_finale() {
        let mut loadout = Loadout::default();
        loadout.finishing_plus = 1;
        let mut combo = at_finisher(loadout);
        // Prima della finestra: troppo presto (ci sono altri finali da fare).
        wait_frame(&mut combo, 6);
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
        // Nella finestra ma senza aver toccato.
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::NotTouched));
        assert_eq!(combo.current_move(), Some((102, 8)));
        combo.tick(hit());
        let events = combo.tick(press());
        // Il finale non può ripetere la propria animazione e non ha alternativa: l'unica
        // altra candidata è il colpo normale... che dopo un finale concatenato non è
        // disponibile; resta il ripiego (la voce 5), aggiunto alle candidate.
        assert_eq!(started(&events), Some((5, 100, 4)));
        assert_eq!((combo.step(), combo.finishers()), (4, 2));
    }

    #[test]
    fn dopo_la_finestra_finale_la_combo_e_finita_anche_con_finishing_plus() {
        let mut loadout = Loadout::default();
        loadout.finishing_plus = 1;
        let mut combo = at_finisher(loadout);
        combo.tick(hit());
        wait_frame(&mut combo, 14);
        // Frame 15: la finestra 8-14 è chiusa e non si riapre.
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::ComboOver));
    }

    #[test]
    fn i_finali_non_superano_il_massimo() {
        let mut loadout = Loadout::default();
        loadout.finishing_plus = 1;
        let mut combo = at_finisher(loadout);
        combo.tick(hit());
        wait_frame(&mut combo, 9);
        // Si impone di nuovo un finale: così resta un finale in corso con finali = 2.
        // (È il finale aereo, quindi la pressione si dà "in aria": un colpo in volo
        // partito da terra farebbe ripartire i contatori.)
        combo.override_next_move(Some(111));
        assert!(started(&combo.tick(in_air(press()))).is_some());
        assert_eq!(combo.finishers(), 2);
        combo.tick(hit());
        wait_frame(&mut combo, 8);
        // Nella finestra, ha toccato, ma i finali sono già al massimo (2).
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::ComboOver));
    }

    #[test]
    fn un_animazione_neutra_lascia_aprire_una_combo_nuova() {
        let mut combo = combo();
        assert_eq!(started(&combo.tick(command(11))), Some((11, 103, 1)));
        wait_frame(&mut combo, 11);
        assert!(!combo.free());
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
        // Frame 13: ritorno alla posa di riposo.
        let events = combo.tick(press());
        assert_eq!(started(&events), Some((5, 100, 1)));
        assert!(!events.contains(&Event::ComboEnded));
    }

    #[test]
    fn occupato_ignora_la_pressione_ma_l_animazione_avanza() {
        let mut combo = combo();
        let events = combo.tick(TickInput { busy: true, ..press() });
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::Busy)]);
        combo.tick(press());
        let events = combo.tick(TickInput { busy: true, ..press() });
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::Busy)]);
        assert_eq!(combo.current_move(), Some((100, 2)));
    }

    #[test]
    fn un_comando_segue_le_regole_di_una_pressione() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 5);
        assert_eq!(ignored(&combo.tick(command(11))), Some(IgnoreReason::TooEarly));
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        assert_eq!(started(&combo.tick(command(11))), Some((11, 103, 2)));
    }

    #[test]
    fn un_comando_non_disponibile_lascia_i_contatori_com_erano() {
        let mut combo = combo();
        // La voce 12 richiede un'abilità che non c'è.
        let events = combo.tick(command(12));
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::Unavailable)]);
        assert!(combo.current_move().is_none());
        combo.tick(press());
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        let events = combo.tick(command(12));
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::Unavailable)]);
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
        assert_eq!(combo.current_move(), Some((100, 11)));
        // Con l'abilità parte.
        combo.loadout.equip(60);
        assert_eq!(started(&combo.tick(command(12))), Some((12, 104, 2)));
    }

    #[test]
    fn con_attacco_e_comando_insieme_vale_il_comando() {
        let mut combo = combo();
        let events = combo.tick(TickInput { attack_pressed: true, ..command(11) });
        assert_eq!(started(&events), Some((11, 103, 1)));
    }

    #[test]
    fn un_colpo_la_cui_animazione_manca_dal_catalogo_finisce_al_tick_dopo() {
        let mut combo = combo();
        let events = combo.tick(command(13));
        assert_eq!(events, [Event::MoveStarted { entry: 13, motion: 999, step: 1 }]);
        assert_eq!(combo.current_move(), Some((999, 1)));
        assert!(!combo.free() && !combo.reaction_allowed());
        assert_eq!(combo.tick(idle()), [Event::ComboEnded]);
    }

    #[test]
    fn il_colpo_imposto_vale_per_una_sola_partenza() {
        let mut combo = combo();
        combo.override_next_move(Some(102));
        assert_eq!(started(&combo.tick(press())), Some((3, 102, 1)));
        combo.interrupt();
        assert_eq!(started(&combo.tick(press())), Some((5, 100, 1)));
    }

    #[test]
    fn il_colpo_imposto_sceglie_la_voce_di_terra_o_di_aria() {
        // Due voci con la stessa animazione, una a terra e una in aria.
        let text = format!("{TEXT}entry id=70 type=0 sub=-1 combo_offset=0 flag=1 motion=100 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0\n");
        let catalog = Arc::new(Catalog::parse(&text).unwrap());
        let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
        combo.override_next_move(Some(100));
        assert_eq!(started(&combo.tick(in_air(press()))), Some((70, 100, 1)));
        let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
        combo.override_next_move(Some(100));
        assert_eq!(started(&combo.tick(press())), Some((5, 100, 1)));
        // Se nessuna corrisponde: la prima con quell'animazione.
        let mut combo = Combo::new(catalog, Loadout::default());
        combo.override_next_move(Some(102));
        assert_eq!(started(&combo.tick(in_air(press()))), Some((3, 102, 1)));
    }

    #[test]
    fn un_colpo_imposto_che_non_esiste_non_parte_e_viene_dimenticato() {
        let mut combo = combo();
        combo.override_next_move(Some(555));
        assert_eq!(combo.tick(press()), [Event::PressIgnored(IgnoreReason::Unavailable)]);
        assert!(combo.current_move().is_none());
        assert_eq!(started(&combo.tick(press())), Some((5, 100, 1)));
    }

    #[test]
    fn togliere_l_imposizione() {
        let mut combo = combo();
        combo.override_next_move(Some(102));
        combo.override_next_move(None);
        assert_eq!(started(&combo.tick(press())), Some((5, 100, 1)));
    }

    #[test]
    fn interrompere_finisce_l_animazione() {
        let mut combo = combo();
        assert!(combo.interrupt().is_empty());
        combo.tick(press());
        wait_frame(&mut combo, 5);
        assert_eq!(combo.interrupt(), [Event::HitWindowClosed, Event::ComboEnded]);
        assert!(combo.current_move().is_none());
        combo.tick(press());
        assert_eq!(combo.interrupt(), [Event::ComboEnded]);
    }

    #[test]
    fn partire_con_una_zona_aperta_la_chiude() {
        let mut combo = combo();
        assert!(started(&combo.tick(command(11))).is_some());
        wait_frame(&mut combo, 5);
        assert!(combo.guarding());
        // Un colpo subito mentre la zona di parata è aperta.
        let events = combo.knocked(50, false);
        assert_eq!(events, [Event::HitWindowClosed, Event::ReactionStarted { motion: 50 }]);
    }

    #[test]
    fn colpo_aereo_senza_bersaglio_sale_del_suo_valore_e_va_alla_velocita_massima() {
        let mut combo = combo();
        let events = combo.tick(in_air(press()));
        assert_eq!(events[1], Event::Lunge { rise: 21.0, run: 0.0, speed: 8.0 });
    }

    #[test]
    fn un_colpo_senza_salto_non_da_slancio() {
        let mut combo = combo();
        let events = combo.tick(press());
        assert!(!events.iter().any(|event| matches!(event, Event::Lunge { .. })));
    }

    fn lunge_of(events: &[Event]) -> (f32, f32, f32) {
        events
            .iter()
            .find_map(|event| match event {
                Event::Lunge { rise, run, speed } => Some((*rise, *run, *speed)),
                _ => None,
            })
            .expect("serve un evento Lunge")
    }

    fn aerial_lunge(target: Body) -> (f32, f32, f32) {
        let mut combo = combo();
        let nearby = [target];
        let scene = Scene { main_target: Some(target), nearby: &nearby, ..Scene::default() };
        lunge_of(&combo.tick(TickInput { scene, ..in_air(press()) }))
    }

    #[test]
    fn colpo_aereo_con_bersaglio_la_salita_sta_tra_salto_minimo_e_massimo() {
        let at = |height| Body { position: [0.0, height, 100.0], radius: 0.0, size: 0.0 };
        assert_eq!(aerial_lunge(at(-40.0)).0, 21.0);
        assert_eq!(aerial_lunge(at(10.0)).0, 21.0);
        assert_eq!(aerial_lunge(at(50.0)).0, 50.0);
        assert_eq!(aerial_lunge(at(400.0)).0, 71.0);
    }

    #[test]
    fn la_salita_si_misura_dai_piedi_del_personaggio() {
        let mut combo = combo();
        let target = Body { position: [0.0, 150.0, 100.0], radius: 0.0, size: 0.0 };
        let scene = Scene { fighter: [0.0, 100.0, 0.0], main_target: Some(target), ..Scene::default() };
        let (rise, _, _) = lunge_of(&combo.tick(TickInput { scene, ..in_air(press()) }));
        assert_eq!(rise, 50.0);
    }

    #[test]
    fn lo_slancio_porta_al_bordo_del_bersaglio_quando_finisce_la_salita() {
        // Bersaglio a 50 unità con raggio 20: 30 di strada. Salita 21.
        let near = Body { position: [30.0, 0.0, 40.0], radius: 20.0, size: 0.0 };
        let (rise, run, speed) = aerial_lunge(near);
        assert_eq!(rise, 21.0);
        assert!((run - 30.0).abs() < 1e-4);
        let reach = air::drift_reach(air::rise_ticks(21.0));
        assert!((speed - 30.0 / reach).abs() < 1e-4);
        assert!(speed > 0.0 && speed < 8.0);
    }

    #[test]
    fn la_velocita_dello_slancio_non_supera_il_massimo_e_non_va_all_indietro() {
        let far = Body { position: [0.0, 0.0, 600.0], radius: 0.0, size: 0.0 };
        assert_eq!(aerial_lunge(far), (21.0, 600.0, 8.0));
        // Dentro il raggio del bersaglio: niente strada da fare, e la velocità minima
        // negativa della voce (-8) non fa andare all'indietro.
        let inside = Body { position: [0.0, 0.0, 10.0], radius: 50.0, size: 0.0 };
        assert_eq!(aerial_lunge(inside), (21.0, 0.0, 0.0));
    }

    #[test]
    fn la_velocita_dello_slancio_non_scende_sotto_il_minimo() {
        // Il finale aereo (voce 8) ha velocità tra 12 e 18.
        let mut combo = combo();
        let target = Body { position: [0.0, 0.0, 10.0], radius: 0.0, size: 0.0 };
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        combo.override_next_move(Some(111));
        let (_, run, speed) = lunge_of(&combo.tick(TickInput { scene, ..in_air(press()) }));
        assert_eq!((run, speed), (10.0, 12.0));
    }

    #[test]
    fn un_colpo_in_volo_da_terra_fa_ripartire_i_contatori() {
        let mut combo = combo();
        combo.tick(press());
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        // Un bersaglio in alto (150 più su) rende candidata la voce 20, che è in volo.
        let target = Body { position: [0.0, 150.0, 100.0], radius: 0.0, size: 0.0 };
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        let events = combo.tick(TickInput { scene, ..press() });
        assert_eq!(started(&events), Some((20, 120, 1)));
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
    }

    #[test]
    fn colpo_in_volo_da_terra_la_salita_sta_tra_i_limiti_di_dislivello() {
        let lunge_at = |height: f32, size: f32| {
            let mut combo = combo();
            let target = Body { position: [0.0, height, 100.0], radius: 0.0, size };
            let scene = Scene { main_target: Some(target), ..Scene::default() };
            combo.override_next_move(Some(120));
            lunge_of(&combo.tick(TickInput { scene, ..press() })).0
        };
        // Limiti della voce 20: tra 110 e 250 più in alto.
        assert_eq!(lunge_at(132.7, 0.0), 132.7);
        assert_eq!(lunge_at(40.0, 0.0), 110.0);
        assert_eq!(lunge_at(367.7, 0.0), 250.0);
        // La taglia allarga i limiti: 110 - 3 e 250 + 3.
        assert_eq!(lunge_at(367.7, 3.0), 253.0);
        assert_eq!(lunge_at(40.0, 3.0), 107.0);
    }

    #[test]
    fn respinto_a_terra_la_combo_riprende_e_il_colpo_non_conta() {
        let mut combo = combo();
        assert!(combo.deflected(false).is_empty());
        combo.tick(press());
        wait_frame(&mut combo, 5);
        let events = combo.deflected(false);
        assert_eq!(events, [Event::HitWindowClosed, Event::ReactionStarted { motion: 41 }]);
        assert_eq!(combo.current_move(), Some((41, 1)));
        assert!(combo.current_entry().is_none());
        assert_eq!(combo.step(), 0);
        // La reazione ha "combo permessa" dal frame 5 e "ha toccato" è già vero.
        wait_frame(&mut combo, 3);
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
        let events = combo.tick(press());
        assert_eq!(started(&events), Some((5, 100, 1)));
    }

    #[test]
    fn respinto_in_aria_senza_l_animazione_nel_catalogo_il_colpo_prosegue() {
        let mut combo = combo();
        combo.tick(in_air(press()));
        wait_frame(&mut combo, 3);
        // L'animazione 43 non è nel catalogo di prova.
        assert!(combo.deflected(true).is_empty());
        assert_eq!(combo.current_move(), Some((110, 3)));
        assert_eq!(combo.step(), 0);
        // "Ha toccato" è diventato vero lo stesso: alla finestra la combo prosegue.
        wait_frame(&mut combo, 7);
        let events = combo.tick(in_air(press()));
        assert_eq!(started(&events), Some((9, 110, 1)));
    }

    #[test]
    fn parata_riuscita_solo_mentre_la_zona_di_parata_e_attiva() {
        let mut combo = combo();
        combo.tick(command(11));
        wait_frame(&mut combo, 2);
        assert!(!combo.guarding());
        assert!(combo.guarded().is_empty());
        assert_eq!(combo.current_move(), Some((103, 2)));
        wait_frame(&mut combo, 3);
        assert!(combo.guarding());
        assert_eq!(combo.hit_attack(), Some(161));
        assert_eq!(combo.hit_power(), None);
        let events = combo.guarded();
        assert_eq!(
            events,
            [
                Event::HitWindowClosed,
                Event::ReactionStarted { motion: 42 },
                Event::HitWindowOpened { attack: 162, power: 5 },
            ]
        );
        assert_eq!(combo.step(), 0);
        // Nella parata riuscita l'arma continua a parare.
        assert!(combo.guarding());
        wait_frame(&mut combo, 7);
        assert!(!combo.guarding());
        assert!(combo.guarded().is_empty());
    }

    #[test]
    fn senza_counterguard_nella_parata_riuscita_si_aspetta_il_neutro() {
        let mut combo = combo();
        combo.tick(command(11));
        wait_frame(&mut combo, 4);
        combo.guarded();
        wait_frame(&mut combo, 7);
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
        // Frame 9: neutra, la pressione apre una combo nuova.
        assert_eq!(started(&combo.tick(press())), Some((5, 100, 1)));
    }

    #[test]
    fn counterguard_parte_subito_durante_la_parata_riuscita() {
        let mut loadout = Loadout::default();
        loadout.equip(96);
        let mut combo = combo_with(loadout);
        combo.tick(command(11));
        wait_frame(&mut combo, 4);
        combo.guarded();
        assert_eq!(combo.step(), 0);
        let events = combo.tick(press());
        // (Il contrattacco di prova comincia con una zona di parata totale, la 124.)
        assert_eq!(
            events,
            [
                Event::HitWindowClosed,
                Event::MoveStarted { entry: 39, motion: 105, step: 1 },
                Event::HitWindowOpened { attack: 124, power: 0 },
            ]
        );
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
    }

    #[test]
    fn counterguard_non_parte_se_il_personaggio_e_occupato() {
        let mut loadout = Loadout::default();
        loadout.equip(96);
        let mut combo = combo_with(loadout);
        combo.tick(command(11));
        wait_frame(&mut combo, 4);
        combo.guarded();
        let events = combo.tick(TickInput { busy: true, ..press() });
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::Busy)]);
    }

    #[test]
    fn subire_un_colpo_rompe_la_combo() {
        let mut combo = combo();
        combo.tick(press());
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        combo.tick(press());
        assert_eq!(combo.step(), 2);
        let events = combo.knocked(50, false);
        assert_eq!(events, [Event::ReactionStarted { motion: 50 }]);
        assert_eq!((combo.step(), combo.finishers()), (0, 1));
        assert!(!combo.launched());
        assert!(combo.current_entry().is_none());
        // L'animazione di danno non ha finestre: le pressioni sono ignorate.
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
        // "Ha toccato" è falso: nemmeno toccare la riapre.
        wait(&mut combo, 20);
        assert!(combo.current_move().is_none());
    }

    #[test]
    fn un_colpo_subito_con_un_animazione_sconosciuta_lascia_il_personaggio_libero() {
        let mut combo = combo();
        combo.tick(press());
        wait_frame(&mut combo, 5);
        let events = combo.knocked(777, true);
        assert_eq!(events, [Event::HitWindowClosed, Event::ComboEnded]);
        assert!(combo.current_move().is_none());
        // Lo stato "sbalzato" vale lo stesso.
        assert!(combo.launched());
        assert!(combo.knocked(777, false).is_empty());
        assert!(!combo.launched());
    }

    #[test]
    fn lo_sbalzo_finisce_quando_finisce_l_animazione() {
        let mut combo = combo();
        combo.knocked(50, true);
        assert!(combo.launched());
        wait(&mut combo, 10);
        assert!(combo.launched());
        let events = combo.tick(idle());
        assert_eq!(events, [Event::ComboEnded]);
        assert!(!combo.launched());
    }

    #[test]
    fn lo_sbalzo_finisce_con_interrupt_e_con_un_colpo() {
        let mut combo = combo();
        combo.knocked(50, true);
        combo.interrupt();
        assert!(!combo.launched());
        combo.knocked(777, true);
        assert!(combo.launched());
        // Niente in corso: la pressione apre una combo nuova e lo sbalzo finisce.
        assert_eq!(started(&combo.tick(press())), Some((5, 100, 1)));
        assert!(!combo.launched());
    }

    #[test]
    fn retaliating_slash_parte_subito_mentre_e_sbalzato() {
        let mut loadout = Loadout::default();
        loadout.equip(101);
        let mut combo = combo_with(loadout);
        combo.knocked(50, true);
        let events = combo.tick(in_air(press()));
        assert_eq!(started(&events), Some((40, 112, 1)));
        assert!(!combo.launched());
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
    }

    #[test]
    fn retaliating_slash_accetta_anche_il_comando_esplicito() {
        let mut loadout = Loadout::default();
        loadout.equip(101);
        let mut combo = combo_with(loadout);
        combo.knocked(50, true);
        let events = combo.tick(in_air(command(RETALIATING_SLASH)));
        assert_eq!(started(&events), Some((40, 112, 1)));
    }

    #[test]
    fn senza_l_abilita_la_pressione_da_sbalzato_e_una_pressione_qualsiasi() {
        let mut combo = combo();
        combo.knocked(50, true);
        let events = combo.tick(in_air(press()));
        assert_eq!(events, [Event::PressIgnored(IgnoreReason::TooEarly)]);
        assert_eq!((combo.step(), combo.finishers()), (0, 1));
        assert!(combo.launched());
    }

    #[test]
    fn se_valgono_entrambe_le_situazioni_vince_lo_sbalzo() {
        let mut loadout = Loadout::default();
        loadout.equip(96);
        loadout.equip(101);
        let mut combo = combo_with(loadout);
        // Sbalzato, e con l'animazione di parata riuscita in corso.
        combo.knocked(42, true);
        let events = combo.tick(in_air(press()));
        assert_eq!(started(&events).unwrap().0, 40);
    }

    #[test]
    fn ritorno_in_equilibrio() {
        let mut loadout = Loadout::default();
        loadout.equip(AERIAL_RECOVERY);
        let mut combo = combo_with(loadout);
        combo.knocked(50, true);
        // La pressione di Attacco nello stesso tick è persa, senza PressIgnored.
        let events = combo.tick(TickInput { recover_pressed: true, ..in_air(press()) });
        // (Nei primi frame il ritorno in equilibrio ha anche una zona di parata totale.)
        assert_eq!(
            events,
            [Event::ReactionStarted { motion: 40 }, Event::HitWindowOpened { attack: 124, power: 0 }]
        );
        assert!(!combo.launched());
        assert_eq!((combo.step(), combo.finishers()), (0, 1));
        assert!(combo.unhittable());
        wait_frame(&mut combo, 4);
        assert!(combo.unhittable() && !combo.free());
        wait_frame(&mut combo, 5);
        assert!(!combo.unhittable());
        wait_frame(&mut combo, 6);
        assert!(combo.free());
    }

    #[test]
    fn il_ritorno_in_equilibrio_vuole_lo_sbalzo_l_abilita_e_il_personaggio_libero() {
        let recover = TickInput { recover_pressed: true, airborne: true, ..TickInput::default() };
        // Senza abilità.
        let mut combo = combo();
        combo.knocked(50, true);
        assert!(combo.tick(recover).is_empty());
        assert!(combo.launched());
        // Con l'abilità ma non sbalzato.
        let mut loadout = Loadout::default();
        loadout.equip(AERIAL_RECOVERY);
        let mut combo = combo_with(loadout.clone());
        combo.knocked(50, false);
        assert!(combo.tick(recover).is_empty());
        // Sbalzato ma occupato.
        let mut combo = combo_with(loadout);
        combo.knocked(50, true);
        assert!(combo.tick(TickInput { busy: true, ..recover }).is_empty());
        assert!(combo.launched());
    }

    #[test]
    fn comando_di_reazione_permesso_e_non_colpibile() {
        let mut combo = combo();
        combo.tick(press());
        assert!(!combo.reaction_allowed());
        wait_frame(&mut combo, 2);
        assert!(combo.reaction_allowed());
        wait_frame(&mut combo, 13);
        assert!(!combo.reaction_allowed());
        let mut combo = at_finisher(Loadout::default());
        assert!(!combo.unhittable());
        wait_frame(&mut combo, 3);
        assert!(combo.unhittable());
        wait_frame(&mut combo, 6);
        assert!(!combo.unhittable());
    }

    #[test]
    fn i_contatori_non_superano_cento() {
        let mut loadout = Loadout::default();
        loadout.combo_master = true;
        loadout.combo_plus = 200;
        let mut combo = combo_with(loadout);
        combo.tick(press());
        for _ in 0..150 {
            // Si alternano le due animazioni, sempre dentro la finestra.
            let next = if combo.current_move().unwrap().0 == 100 { 101 } else { 100 };
            wait_frame(&mut combo, 15);
            combo.override_next_move(Some(next));
            assert!(started(&combo.tick(press())).is_some());
        }
        assert_eq!(combo.step(), 100);
    }

    #[test]
    fn berserk_charge_non_lascia_arrivare_il_finale() {
        let mut loadout = Loadout::default();
        loadout.berserk_charge = true;
        loadout.combo_master = true;
        let mut combo = combo_with(loadout);
        let raging = TickInput { berserk: true, ..press() };
        combo.tick(raging);
        let mut moves = vec![100];
        for _ in 0..6 {
            wait_frame(&mut combo, 15);
            let (_, motion, _) = started(&combo.tick(raging)).unwrap();
            moves.push(motion);
        }
        // Mai il finale (102): si alternano il primo colpo e la sua alternativa.
        assert_eq!(moves, [100, 101, 100, 101, 100, 101, 100]);
        assert_eq!(combo.step(), 7);
        // Quando la condizione smette di essere vera, il finale arriva.
        wait_frame(&mut combo, 10);
        assert_eq!(started(&combo.tick(press())), Some((3, 102, 8)));
    }

    #[test]
    fn zone_sovrapposte_una_zona_di_danno_e_la_parata_insieme() {
        let mut combo = combo();
        let events = combo.tick(command(14));
        // Frame 1: una zona il cui colpo a segno non è nel catalogo: di danno, potenza 0.
        assert_eq!(
            events,
            [
                Event::MoveStarted { entry: 14, motion: 106, step: 1 },
                Event::HitWindowOpened { attack: 77, power: 0 },
            ]
        );
        // Per ogni frame: (zona indicata, potenza, sta parando).
        let mut seen = vec![(combo.hit_attack(), combo.hit_power(), combo.guarding())];
        let mut log = Vec::new();
        for frame in 2..=22u16 {
            for event in combo.tick(idle()) {
                log.push((frame, event));
            }
            seen.push((combo.hit_attack(), combo.hit_power(), combo.guarding()));
        }
        let at = |frame: usize| seen[frame - 1];
        assert_eq!(at(1), (Some(77), Some(0), false));
        assert_eq!(at(2), (Some(77), Some(0), false));
        // Solo parata: è indicata la prima zona di parata, e non c'è nessuna potenza.
        assert_eq!(at(3), (Some(161), None, true));
        assert_eq!(at(7), (Some(161), None, true));
        // Danno e parata insieme: è indicata la zona di danno, e intanto para.
        assert_eq!(at(8), (Some(8), Some(33), true));
        assert_eq!(at(9), (Some(8), Some(33), true));
        // Due zone di danno insieme: vince la potenza più alta.
        assert_eq!(at(10), (Some(7), Some(100), true));
        assert_eq!(at(15), (Some(7), Some(100), true));
        assert_eq!(at(16), (Some(8), Some(33), true));
        assert_eq!(at(18), (Some(8), Some(33), true));
        assert_eq!(at(19), (Some(161), None, true));
        assert_eq!(at(20), (Some(161), None, true));
        assert_eq!(at(21), (None, None, false));
        // Gli eventi seguono la zona indicata.
        assert_eq!(
            log,
            [
                (3, Event::HitWindowClosed),
                (3, Event::HitWindowOpened { attack: 161, power: 0 }),
                (8, Event::HitWindowClosed),
                (8, Event::HitWindowOpened { attack: 8, power: 33 }),
                (10, Event::HitWindowClosed),
                (10, Event::HitWindowOpened { attack: 7, power: 100 }),
                (16, Event::HitWindowClosed),
                (16, Event::HitWindowOpened { attack: 8, power: 33 }),
                (19, Event::HitWindowClosed),
                (19, Event::HitWindowOpened { attack: 161, power: 0 }),
                (21, Event::HitWindowClosed),
            ]
        );
    }

    #[test]
    fn a_parita_di_potenza_vince_la_prima_zona_e_la_parata_non_da_eventi() {
        let mut combo = combo();
        let events = combo.tick(command(17));
        // Due zone di danno di potenza 33: quella dell'arma (8) viene prima di quella del
        // personaggio (88), anche se nel file è scritta dopo.
        assert_eq!(events[1], Event::HitWindowOpened { attack: 8, power: 33 });
        let mut guarding = Vec::new();
        let mut later = Vec::new();
        for frame in 2..=12u16 {
            for event in combo.tick(idle()) {
                later.push((frame, event));
            }
            if combo.guarding() {
                guarding.push(frame);
                assert_eq!((combo.hit_attack(), combo.hit_power()), (Some(8), Some(33)));
            }
        }
        // La parata comincia al frame 3 e finisce al 6 mentre resta indicata la stessa zona
        // di danno: non dà eventi, la si vede solo con `guarding`.
        assert_eq!(guarding, [3, 4, 5, 6]);
        assert_eq!(later, [(10, Event::HitWindowClosed)]);
    }

    #[test]
    fn la_sola_parata_totale_ferma_il_colpo_nemico_ma_non_cambia_niente() {
        let mut loadout = Loadout::default();
        loadout.equip(96);
        let mut combo = combo_with(loadout);
        // Fuori dalla situazione il comando 39 è un comando qualsiasi: apre la combo.
        assert_eq!(started(&combo.tick(command(COUNTERGUARD))), Some((39, 105, 1)));
        wait_frame(&mut combo, 3);
        assert!(combo.guarding());
        assert_eq!((combo.hit_attack(), combo.hit_power()), (Some(124), None));
        assert!(combo.guarded().is_empty());
        assert_eq!(combo.current_move(), Some((105, 3)));
        assert_eq!((combo.step(), combo.finishers()), (1, 1));
        // Nemmeno "ha toccato" è cambiato: nella finestra la pressione è persa.
        wait_frame(&mut combo, 6);
        assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::NotTouched));
    }

    #[test]
    fn con_una_parata_e_una_parata_totale_insieme_parte_la_parata_riuscita() {
        let mut combo = combo();
        combo.tick(command(14));
        wait_frame(&mut combo, 5);
        // Coprono il frame la zona 161 (parata) e la 124 (parata totale).
        let events = combo.guarded();
        assert_eq!(
            events,
            [
                Event::HitWindowClosed,
                Event::ReactionStarted { motion: 42 },
                Event::HitWindowOpened { attack: 162, power: 5 },
            ]
        );
        assert_eq!(combo.step(), 0);
    }

    /// Una combo arrivata alla parata riuscita, con queste abilità equipaggiate.
    fn in_guard_success(abilities: &[u16]) -> Combo {
        let mut loadout = Loadout::default();
        for ability in abilities {
            loadout.equip(*ability);
        }
        let mut combo = combo_with(loadout);
        combo.tick(command(11));
        wait_frame(&mut combo, 4);
        assert!(!combo.guarded().is_empty());
        assert_eq!(combo.current_move(), Some((42, 1)));
        combo
    }

    #[test]
    fn nella_parata_riuscita_un_altro_comando_senza_attacco_e_un_comando_qualsiasi() {
        let mut combo = in_guard_success(&[96]);
        // Il comando 11 (un'altra parata): la parata riuscita non ha una finestra "combo
        // permessa", quindi è ignorato finché l'animazione non diventa neutra.
        assert_eq!(combo.tick(command(11)), [Event::PressIgnored(IgnoreReason::TooEarly)]);
        assert_eq!(combo.current_move(), Some((42, 2)));
        // Il comando con proprio il numero della voce della situazione la fa partire subito.
        assert_eq!(started(&combo.tick(command(COUNTERGUARD))), Some((39, 105, 1)));
    }

    #[test]
    fn nella_parata_riuscita_attacco_conta_anche_insieme_a_un_altro_comando() {
        let mut combo = in_guard_success(&[96]);
        let events = combo.tick(TickInput { attack_pressed: true, ..command(11) });
        assert_eq!(started(&events), Some((39, 105, 1)));
    }

    #[test]
    fn se_la_voce_della_situazione_non_parte_con_attacco_e_comando_vale_il_comando() {
        // Senza Counterguard: la pressione viene giudicata come una qualsiasi, e tra
        // Attacco e comando vale il comando.
        let mut combo = in_guard_success(&[]);
        wait_frame(&mut combo, 8);
        // Frame 9: la parata riuscita è neutra, e parte la voce del comando (la 11).
        let events = combo.tick(TickInput { attack_pressed: true, ..command(11) });
        assert_eq!(started(&events), Some((11, 103, 1)));
    }

    #[test]
    fn fuori_dalla_situazione_i_comandi_39_e_40_sono_comandi_qualsiasi() {
        let mut loadout = Loadout::default();
        loadout.equip(96);
        loadout.equip(101);
        let mut combo = combo_with(loadout);
        // Un colpo in corso, prima della finestra: nessuna partenza immediata.
        combo.tick(press());
        wait_frame(&mut combo, 4);
        assert_eq!(ignored(&combo.tick(command(COUNTERGUARD))), Some(IgnoreReason::TooEarly));
        let retaliate = in_air(command(RETALIATING_SLASH));
        assert_eq!(ignored(&combo.tick(retaliate)), Some(IgnoreReason::TooEarly));
        assert_eq!(combo.current_move(), Some((100, 6)));
        // Nella finestra, avendo toccato, parte come ogni comando: i passi salgono a 2.
        combo.tick(hit());
        wait_frame(&mut combo, 10);
        assert_eq!(started(&combo.tick(command(COUNTERGUARD))), Some((39, 105, 2)));
    }

    #[test]
    fn da_sbalzato_un_altro_comando_senza_attacco_non_richiama_il_colpo_di_ritorno() {
        let mut loadout = Loadout::default();
        loadout.equip(101);
        let mut combo = combo_with(loadout);
        combo.knocked(50, true);
        let other = in_air(command(12));
        assert_eq!(combo.tick(other), [Event::PressIgnored(IgnoreReason::TooEarly)]);
        assert!(combo.launched());
        // Attacco insieme a un altro comando: conta Attacco, parte il colpo di ritorno.
        let events = combo.tick(TickInput { attack_pressed: true, ..other });
        assert_eq!(started(&events), Some((40, 112, 1)));
        assert!(!combo.launched());
    }

    /// Una combo in tutte e due le situazioni insieme (sbalzato, e con l'animazione di
    /// parata riuscita in corso), con queste abilità equipaggiate.
    fn in_both_situations(abilities: &[u16]) -> Combo {
        let mut loadout = Loadout::default();
        for ability in abilities {
            loadout.equip(*ability);
        }
        let mut combo = combo_with(loadout);
        combo.knocked(42, true);
        assert!(combo.launched());
        assert_eq!(combo.current_move(), Some((42, 1)));
        combo
    }

    #[test]
    fn con_tutte_e_due_le_situazioni_ogni_comando_richiama_la_sua_voce() {
        // Il comando 39, senza Attacco: Counterguard, subito (a terra).
        let mut combo = in_both_situations(&[96, 101]);
        assert_eq!(started(&combo.tick(command(COUNTERGUARD))), Some((39, 105, 1)));
        // Il comando 40, senza Attacco: il colpo di ritorno (in aria).
        let mut combo = in_both_situations(&[96, 101]);
        let events = combo.tick(in_air(command(RETALIATING_SLASH)));
        assert_eq!(started(&events), Some((40, 112, 1)));
        // Un altro comando, senza Attacco: nessuna delle due, è un comando qualsiasi.
        let mut combo = in_both_situations(&[96, 101]);
        assert_eq!(combo.tick(command(12)), [Event::PressIgnored(IgnoreReason::TooEarly)]);
        assert!(combo.launched());
    }

    #[test]
    fn con_tutte_e_due_le_situazioni_attacco_prova_solo_la_voce_dello_sbalzo() {
        // In aria, con tutte e due le abilità: parte la 40.
        let mut combo = in_both_situations(&[96, 101]);
        assert_eq!(started(&combo.tick(in_air(press()))), Some((40, 112, 1)));
        // Anche se insieme ad Attacco c'è il comando 39.
        let mut combo = in_both_situations(&[96, 101]);
        let both = TickInput { attack_pressed: true, ..in_air(command(COUNTERGUARD)) };
        assert_eq!(started(&combo.tick(both)), Some((40, 112, 1)));
        // Se la 40 non può partire (qui il personaggio è a terra), la 39 non viene provata:
        // la pressione viene giudicata come una qualsiasi, e la parata riuscita non è
        // ancora neutra.
        let mut combo = in_both_situations(&[96, 101]);
        assert_eq!(combo.tick(press()), [Event::PressIgnored(IgnoreReason::TooEarly)]);
        assert_eq!(combo.current_move(), Some((42, 2)));
        assert!(combo.launched());
    }

    #[test]
    fn con_tutte_e_due_le_situazioni_un_comando_non_disponibile_e_un_comando_qualsiasi() {
        // Senza Counterguard: il comando 39 non parte subito e viene poi giudicato come un
        // comando qualsiasi (qui: troppo presto).
        let mut combo = in_both_situations(&[101]);
        assert_eq!(
            combo.tick(command(COUNTERGUARD)),
            [Event::PressIgnored(IgnoreReason::TooEarly)]
        );
        assert_eq!((combo.step(), combo.finishers()), (0, 1));
    }

    #[test]
    fn una_voce_a_terra_che_non_guarda_il_dislivello_sale_come_una_voce_in_aria() {
        // Il colpo del bordo (voce 1) ha i due limiti di dislivello uguali, a zero: con
        // un bersaglio sale tra il salto minimo (21) e il massimo (71), qualunque taglia.
        let lunge_at = |height: f32| {
            let mut combo = combo();
            let target = Body { position: [0.0, height, 100.0], radius: 0.0, size: 5.0 };
            let scene = Scene { main_target: Some(target), ..Scene::default() };
            lunge_of(&combo.tick(TickInput { scene, ..command(1) })).0
        };
        assert_eq!(lunge_at(-30.0), 21.0);
        assert_eq!(lunge_at(0.0), 21.0);
        assert_eq!(lunge_at(50.0), 50.0);
        assert_eq!(lunge_at(300.0), 71.0);
        // Senza bersaglio sale del suo valore di salto.
        let mut combo = combo();
        assert_eq!(lunge_of(&combo.tick(command(1))).0, 21.0);
    }

    #[test]
    fn con_il_minimo_maggiore_del_massimo_la_salita_e_almeno_il_salto_minimo() {
        // La voce 15 (in aria) ha salto minimo 40 e massimo 10.
        let lunge_at = |height: f32| {
            let mut combo = combo();
            let target = Body { position: [0.0, height, 100.0], radius: 0.0, size: 0.0 };
            let scene = Scene { main_target: Some(target), ..Scene::default() };
            lunge_of(&combo.tick(TickInput { scene, ..in_air(command(15)) })).0
        };
        assert_eq!(lunge_at(100.0), 100.0);
        assert_eq!(lunge_at(5.0), 40.0);
        assert_eq!(lunge_at(-20.0), 40.0);
    }

    #[test]
    fn con_una_salita_nulla_lo_slancio_va_al_massimo_se_c_e_strada_da_fare() {
        // La voce 16 (in aria) ha salto minimo e massimo a zero e velocità tra 2 e 9.
        let lunge_to = |target: Body| {
            let mut combo = combo();
            let scene = Scene { main_target: Some(target), ..Scene::default() };
            lunge_of(&combo.tick(TickInput { scene, ..in_air(command(16)) }))
        };
        let far = Body { position: [0.0, 50.0, 100.0], radius: 0.0, size: 0.0 };
        assert_eq!(lunge_to(far), (0.0, 100.0, 9.0));
        // Già al bordo del bersaglio: la velocità che serve è zero, portata al minimo (2).
        let inside = Body { position: [0.0, 50.0, 10.0], radius: 30.0, size: 0.0 };
        assert_eq!(lunge_to(inside), (0.0, 0.0, 2.0));
    }

    #[test]
    fn se_la_velocita_minima_supera_la_massima_vale_la_piu_grande_delle_due() {
        // La voce 18 (in aria) ha velocità minima 10 e massima 4.
        let mut combo = combo();
        assert_eq!(lunge_of(&combo.tick(in_air(command(18)))), (20.0, 0.0, 10.0));
        let mut combo = self::combo();
        let target = Body { position: [0.0, 0.0, 30.0], radius: 0.0, size: 0.0 };
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        let events = combo.tick(TickInput { scene, ..in_air(command(18)) });
        assert_eq!(lunge_of(&events), (20.0, 30.0, 10.0));
    }
}
