//! Abilità, limiti della combo e scelta del colpo (SPEC, sezioni 5 e 6).
//!
//! Qui non c'è stato: ogni funzione risponde guardando solo ciò che riceve.

use std::collections::BTreeSet;

use crate::catalog::{flag, kind, Catalog, Entry};

/// Numero della voce richiamata durante la parata riuscita (SPEC 8).
pub const COUNTERGUARD: u8 = 39;
/// Numero della voce richiamata mentre il personaggio è sbalzato (SPEC 8).
pub const RETALIATING_SLASH: u8 = 40;
/// Numero dell'abilità Aerial Recovery.
pub const AERIAL_RECOVERY: u16 = 20;

/// Voce di ripiego a terra (SPEC 6.3).
const GROUND_FALLBACK: u8 = 5;
/// Voce di ripiego in aria (SPEC 6.3).
const AIR_FALLBACK: u8 = 9;
/// Voce del bordo (SPEC 6.2).
const LEDGE: u8 = 1;

/// Passi e finali senza abilità (SPEC 5).
const BASE_STEPS: u8 = 3;
const BASE_FINISHERS: u8 = 1;

/// Le abilità equipaggiate.
///
/// I numeri equipaggiati con `equip` servono solo per le abilità richieste dalle voci del
/// repertorio e per Aerial Recovery. Combo Master e Berserk Charge contano solo attraverso
/// i due campi `combo_master` e `berserk_charge`: equipaggiare il loro numero non ha
/// effetto sulle regole (R5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loadout {
    /// I numeri delle abilità equipaggiate con `equip`.
    equipped: BTreeSet<u16>,
    /// Quante copie di Combo Plus.
    pub combo_plus: u8,
    /// Quante copie di Air Combo Plus.
    pub air_combo_plus: u8,
    pub negative_combo: bool,
    /// Quante copie di Finishing Plus.
    pub finishing_plus: u8,
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
    /// Equipaggia un'abilità che ha un numero.
    pub fn equip(&mut self, ability: u16) {
        self.equipped.insert(ability);
    }

    pub fn unequip(&mut self, ability: u16) {
        self.equipped.remove(&ability);
    }

    pub fn has(&self, ability: u16) -> bool {
        self.equipped.contains(&ability)
    }

    /// I limiti secondo SPEC 5: passi a terra = 3 + combo_plus, in aria = 3 + air_combo_plus,
    /// ciascuno meno 1 con negative_combo e mai sotto 1; finali = 1 + finishing_plus.
    pub fn limits(&self) -> Limits {
        let steps = |copies: u8| {
            let total = BASE_STEPS.saturating_add(copies);
            if self.negative_combo {
                total.saturating_sub(1).max(1)
            } else {
                total
            }
        };
        Limits {
            ground_steps: steps(self.combo_plus),
            air_steps: steps(self.air_combo_plus),
            finishers: BASE_FINISHERS.saturating_add(self.finishing_plus),
        }
    }
}

/// Un nemico, come lo vedono le regole.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// Il punto a cui si mira: x, altezza, z.
    pub position: [f32; 3],
    /// Si toglie dalla distanza tra i centri.
    pub radius: f32,
    /// "Taglia": allarga i limiti di dislivello.
    pub size: f32,
}

/// La situazione attorno al personaggio quando parte un colpo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scene<'a> {
    /// Dove sta il personaggio (i piedi): x, altezza, z.
    pub fighter: [f32; 3],
    /// Dove guarda: x, z, di lunghezza 1.
    pub facing: [f32; 2],
    /// Il bersaglio principale.
    pub main_target: Option<Body>,
    /// Tutti i nemici nei dintorni, principale compreso.
    pub nearby: &'a [Body],
    /// A terra: subito davanti il terreno manca.
    pub ledge_ahead: bool,
}

impl Default for Scene<'_> {
    fn default() -> Self {
        Scene {
            fighter: [0.0, 0.0, 0.0],
            facing: [0.0, 1.0],
            main_target: None,
            nearby: &[],
            ledge_ahead: false,
        }
    }
}

/// Lo stato del personaggio che conta per la scelta.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FighterView {
    pub airborne: bool,
    /// L'animazione in corso, se c'è.
    pub current_motion: Option<u16>,
    /// I passi, già contando il colpo che sta per partire.
    pub steps: u8,
    pub finishers: u8,
    /// L'animazione in corso era nella finestra "finale permesso".
    pub finisher_window: bool,
    /// La "dote" (SPEC 13): oggi sempre zero.
    pub reach_bonus: f32,
    /// La condizione di Berserk Charge è vera adesso.
    pub berserk: bool,
}

/// SPEC 6.1: la voce può partire, guardando solo il personaggio e le sue abilità.
pub fn available(entry: &Entry, loadout: &Loadout, view: &FighterView) -> bool {
    if entry.motion == 0 {
        return false;
    }
    if entry.ability != 0 && !loadout.has(entry.ability) {
        return false;
    }
    if entry.is_aerial() != view.airborne {
        return false;
    }
    // Berserk Charge: finché la sua condizione è vera il finale non arriva mai.
    if loadout.berserk_charge && view.berserk {
        return !entry.is_finisher();
    }
    let limits = loadout.limits();
    let limit = if view.airborne {
        limits.air_steps
    } else {
        limits.ground_steps
    };
    if entry.is_finisher() {
        view.finisher_window
            || i32::from(view.steps) + i32::from(entry.step_advance) >= i32::from(limit)
    } else {
        !(view.finishers > 1 && (view.finisher_window || view.steps >= limit))
    }
}

/// SPEC 6.2: una casella per ogni voce del repertorio, nello stesso ordine; vera se la voce
/// è candidata. (Il ripiego di SPEC 6.3 qui non c'entra.)
pub fn candidates(
    catalog: &Catalog,
    loadout: &Loadout,
    view: &FighterView,
    scene: &Scene,
) -> Vec<bool> {
    let reference = reference_direction(scene);
    // Il bordo: a terra, con il terreno che manca subito davanti, la voce del bordo è
    // candidata senza guardare la sua disponibilità.
    let ledge = if !view.airborne && scene.ledge_ahead {
        catalog.index_of_id(LEDGE)
    } else {
        None
    };
    catalog
        .entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            if ledge == Some(index) {
                return true;
            }
            if !available(entry, loadout, view) {
                return false;
            }
            // SPEC 13 (da NON riprodurre): il punteggio (`score`) e la correzione della
            // distanza (`distance_adjust`) della voce qui non si guardano: conta solo
            // l'ordine del repertorio.
            match entry.kind {
                kind::MAIN_TARGET => scene
                    .main_target
                    .is_some_and(|body| in_reach(entry, view, scene, &body, reference)),
                kind::ANY_NEARBY => scene
                    .nearby
                    .iter()
                    .any(|body| in_reach(entry, view, scene, body, reference)),
                kind::ALWAYS => true,
                // Tipo 0: mai candidata, la richiama un comando (SPEC 8). Lo stesso vale
                // per un tipo sconosciuto, oltre 3 (R2).
                _ => false,
            }
        })
        .collect()
}

/// SPEC 6.2 + 6.3: il colpo che parte. `None` solo se manca perfino la voce di ripiego.
pub fn choose<'a>(
    catalog: &'a Catalog,
    loadout: &Loadout,
    view: &FighterView,
    scene: &Scene,
) -> Option<&'a Entry> {
    choose_index(catalog, loadout, view, scene).map(|index| &catalog.entries[index])
}

/// SPEC 8: la prima voce con quel numero, se è disponibile (SPEC 6.1); altrimenti `None`.
pub fn by_command<'a>(
    catalog: &'a Catalog,
    loadout: &Loadout,
    view: &FighterView,
    id: u8,
) -> Option<&'a Entry> {
    by_command_index(catalog, loadout, view, id).map(|index| &catalog.entries[index])
}

/// Come `choose`, ma dà la posizione della voce nel repertorio.
pub(crate) fn choose_index(
    catalog: &Catalog,
    loadout: &Loadout,
    view: &FighterView,
    scene: &Scene,
) -> Option<usize> {
    let mut candidate = candidates(catalog, loadout, view, scene);
    if let Some(index) = first_usable(catalog, view, &candidate) {
        return Some(index);
    }
    // Nessuna candidata e nessuna alternativa: si aggiunge il ripiego e si ripete.
    let fallback = catalog.index_of_id(if view.airborne {
        AIR_FALLBACK
    } else {
        GROUND_FALLBACK
    })?;
    candidate[fallback] = true;
    // Se nemmeno così esce una voce (il ripiego ripeterebbe l'animazione in corso e non ha
    // un'alternativa), parte lo stesso il ripiego (R4).
    first_usable(catalog, view, &candidate).or(Some(fallback))
}

/// Come `by_command`, ma dà la posizione della voce nel repertorio.
pub(crate) fn by_command_index(
    catalog: &Catalog,
    loadout: &Loadout,
    view: &FighterView,
    id: u8,
) -> Option<usize> {
    let index = catalog.index_of_id(id)?;
    available(&catalog.entries[index], loadout, view).then_some(index)
}

/// SPEC 6.3, punti 1 e 2: la prima candidata nell'ordine del repertorio, saltando quelle
/// che ripeterebbero l'animazione in corso; se non ne resta nessuna, l'alternativa della
/// prima saltata che ne ha una.
fn first_usable(catalog: &Catalog, view: &FighterView, candidate: &[bool]) -> Option<usize> {
    let mut alternative = None;
    for (index, entry) in catalog.entries.iter().enumerate() {
        if !candidate[index] {
            continue;
        }
        let repeats = view.current_motion == Some(entry.motion);
        let own_alternative = i16::from(entry.alternative) == i16::from(entry.id);
        if repeats && !own_alternative {
            if alternative.is_none() {
                // Un'alternativa conta solo se è una voce che esiste.
                alternative = u8::try_from(entry.alternative)
                    .ok()
                    .and_then(|id| catalog.index_of_id(id));
            }
            continue;
        }
        return Some(index);
    }
    // L'alternativa parte senza controllare se sarebbe stata disponibile o candidata.
    alternative
}

/// Distanza orizzontale tra i centri, meno il raggio del nemico, mai sotto zero.
pub(crate) fn gap(fighter: [f32; 3], body: &Body) -> f32 {
    let dx = body.position[0] - fighter[0];
    let dz = body.position[2] - fighter[2];
    ((dx * dx + dz * dz).sqrt() - body.radius).max(0.0)
}

/// La direzione orizzontale dal personaggio a un punto; `None` se il punto è esattamente
/// sopra (o sotto) il personaggio.
pub(crate) fn direction_to(fighter: [f32; 3], position: [f32; 3]) -> Option<[f32; 2]> {
    let dx = position[0] - fighter[0];
    let dz = position[2] - fighter[2];
    let length = (dx * dx + dz * dz).sqrt();
    (length > 0.0).then(|| [dx / length, dz / length])
}

/// La direzione di riferimento: verso il bersaglio principale; se non c'è, o sta
/// esattamente sopra il personaggio, quella in cui il personaggio guarda.
pub(crate) fn reference_direction(scene: &Scene) -> [f32; 2] {
    scene
        .main_target
        .and_then(|target| direction_to(scene.fighter, target.position))
        .unwrap_or(scene.facing)
}

/// I due limiti di salita per un nemico di taglia `size` (INTERFACCIA, "Dislivello"):
/// la salita deve valere almeno il primo e meno del secondo.
pub(crate) fn rise_limits(entry: &Entry, size: f32, reach_bonus: f32) -> (f32, f32) {
    let mut high = entry.high;
    if entry.flags & flag::REACH_GROWS != 0 {
        high -= reach_bonus;
    }
    let low = if entry.flags & flag::LOW_INVERTED != 0 {
        entry.low - size
    } else {
        entry.low + size
    };
    let high = if entry.flags & flag::HIGH_INVERTED != 0 {
        high + size
    } else {
        high - size
    };
    (-low, -high)
}

/// SPEC 6.2: il nemico soddisfa distanza, dislivello e angolo della voce.
fn in_reach(
    entry: &Entry,
    view: &FighterView,
    scene: &Scene,
    body: &Body,
    reference: [f32; 2],
) -> bool {
    let distance = gap(scene.fighter, body);
    if !(entry.near <= distance && distance < entry.far) {
        return false;
    }
    let climb = body.position[1] - scene.fighter[1];
    let (lowest, highest) = rise_limits(entry, body.size, view.reach_bonus);
    if !(lowest <= climb && climb < highest) {
        return false;
    }
    // Il coseno non supera mai 1; un nemico esattamente sopra il personaggio ha coseno 1.
    let cosine = match direction_to(scene.fighter, body.position) {
        Some(toward) => (toward[0] * reference[0] + toward[1] * reference[1]).min(1.0),
        None => 1.0,
    };
    entry.inner_min < cosine && cosine <= entry.inner_max
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una voce a terra, sempre candidata, senza condizioni.
    fn entry(id: u8, motion: u16) -> Entry {
        Entry {
            id,
            kind: kind::ALWAYS,
            alternative: -1,
            step_advance: 0,
            flags: 0,
            motion,
            next_motion: 0,
            jump: 0.0,
            jump_max: 0.0,
            jump_min: 0.0,
            speed_min: 0.0,
            speed_max: 0.0,
            near: 0.0,
            far: 0.0,
            low: 0.0,
            high: 0.0,
            inner_min: -1.0,
            inner_max: 1.0,
            blend: 0.0,
            distance_adjust: 0.0,
            ability: 0,
            score: 0,
        }
    }

    /// Una voce che guarda i nemici: distanza 0..100, dislivello -50..50, qualsiasi angolo.
    fn aimed(id: u8, motion: u16, kind: u8) -> Entry {
        Entry {
            kind,
            far: 100.0,
            low: 50.0,
            high: -50.0,
            inner_min: -2.0,
            ..entry(id, motion)
        }
    }

    fn catalog(entries: Vec<Entry>) -> Catalog {
        Catalog {
            entries,
            ..Catalog::default()
        }
    }

    fn body(x: f32, height: f32, z: f32) -> Body {
        Body {
            position: [x, height, z],
            radius: 0.0,
            size: 0.0,
        }
    }

    fn ground(steps: u8) -> FighterView {
        FighterView {
            steps,
            finishers: 1,
            ..FighterView::default()
        }
    }

    #[test]
    fn limiti_senza_abilita() {
        let limits = Loadout::default().limits();
        assert_eq!(limits, Limits { ground_steps: 3, air_steps: 3, finishers: 1 });
    }

    #[test]
    fn limiti_con_le_abilita() {
        let mut loadout = Loadout::default();
        loadout.combo_plus = 2;
        loadout.air_combo_plus = 1;
        loadout.finishing_plus = 3;
        assert_eq!(loadout.limits(), Limits { ground_steps: 5, air_steps: 4, finishers: 4 });
        loadout.negative_combo = true;
        assert_eq!(loadout.limits(), Limits { ground_steps: 4, air_steps: 3, finishers: 4 });
        let mut negative = Loadout::default();
        negative.negative_combo = true;
        assert_eq!(negative.limits(), Limits { ground_steps: 2, air_steps: 2, finishers: 1 });
        // Tante copie non fanno uscire i numeri dal loro tipo.
        negative.combo_plus = 255;
        negative.finishing_plus = 255;
        assert_eq!(negative.limits().ground_steps, 254);
        assert_eq!(negative.limits().finishers, 255);
    }

    #[test]
    fn equipaggiare_e_togliere() {
        let mut loadout = Loadout::default();
        assert!(!loadout.has(92));
        loadout.equip(92);
        loadout.equip(92);
        assert!(loadout.has(92));
        loadout.unequip(92);
        assert!(!loadout.has(92));
        loadout.unequip(92);
        assert_eq!(loadout, Loadout::default());
    }

    #[test]
    fn scena_predefinita() {
        let scene = Scene::default();
        assert_eq!(scene.fighter, [0.0, 0.0, 0.0]);
        assert_eq!(scene.facing, [0.0, 1.0]);
        assert!(scene.main_target.is_none() && scene.nearby.is_empty() && !scene.ledge_ahead);
    }

    #[test]
    fn disponibile_solo_con_un_animazione() {
        let loadout = Loadout::default();
        assert!(available(&entry(1, 10), &loadout, &ground(1)));
        assert!(!available(&entry(1, 0), &loadout, &ground(1)));
    }

    #[test]
    fn disponibile_solo_con_l_abilita_richiesta() {
        let mut loadout = Loadout::default();
        let needs = Entry { ability: 92, ..entry(1, 10) };
        assert!(!available(&needs, &loadout, &ground(1)));
        loadout.equip(92);
        assert!(available(&needs, &loadout, &ground(1)));
    }

    #[test]
    fn disponibile_solo_se_terra_o_aria_corrispondono() {
        let loadout = Loadout::default();
        let on_ground = entry(1, 10);
        let in_air = Entry { flags: flag::AERIAL, ..entry(2, 11) };
        let air_view = FighterView { airborne: true, ..ground(1) };
        assert!(available(&on_ground, &loadout, &ground(1)));
        assert!(!available(&on_ground, &loadout, &air_view));
        assert!(!available(&in_air, &loadout, &ground(1)));
        assert!(available(&in_air, &loadout, &air_view));
    }

    #[test]
    fn un_finale_e_disponibile_al_limite_dei_passi_o_nella_finestra_finale() {
        let loadout = Loadout::default();
        let finisher = Entry { flags: flag::FINISHER, ..entry(3, 12) };
        assert!(!available(&finisher, &loadout, &ground(1)));
        assert!(!available(&finisher, &loadout, &ground(2)));
        assert!(available(&finisher, &loadout, &ground(3)));
        assert!(available(&finisher, &loadout, &ground(4)));
        let window = FighterView { finisher_window: true, ..ground(1) };
        assert!(available(&finisher, &loadout, &window));
    }

    #[test]
    fn l_anticipo_rende_un_finale_disponibile_prima() {
        let loadout = Loadout::default();
        let leap = Entry { flags: flag::FINISHER, step_advance: 1, ..entry(37, 12) };
        assert!(!available(&leap, &loadout, &ground(1)));
        assert!(available(&leap, &loadout, &ground(2)));
        let late = Entry { flags: flag::FINISHER, step_advance: -1, ..entry(38, 12) };
        assert!(!available(&late, &loadout, &ground(3)));
        assert!(available(&late, &loadout, &ground(4)));
    }

    #[test]
    fn il_limite_usato_e_quello_di_terra_o_di_aria() {
        let mut loadout = Loadout::default();
        loadout.combo_plus = 1;
        let ground_finisher = Entry { flags: flag::FINISHER, ..entry(3, 12) };
        let air_finisher = Entry { flags: flag::FINISHER | flag::AERIAL, ..entry(8, 13) };
        assert!(!available(&ground_finisher, &loadout, &ground(3)));
        assert!(available(&ground_finisher, &loadout, &ground(4)));
        let air_view = FighterView { airborne: true, ..ground(3) };
        assert!(available(&air_finisher, &loadout, &air_view));
    }

    #[test]
    fn un_colpo_normale_non_e_disponibile_dopo_un_finale_concatenato() {
        let loadout = Loadout::default();
        let normal = entry(5, 10);
        // Al passo del finale un normale resta disponibile finché i finali valgono 1.
        assert!(available(&normal, &loadout, &ground(3)));
        let chained_at_limit = FighterView { finishers: 2, ..ground(3) };
        assert!(!available(&normal, &loadout, &chained_at_limit));
        let chained_in_window = FighterView { finishers: 2, finisher_window: true, ..ground(1) };
        assert!(!available(&normal, &loadout, &chained_in_window));
        let chained_early = FighterView { finishers: 2, ..ground(2) };
        assert!(available(&normal, &loadout, &chained_early));
    }

    #[test]
    fn berserk_charge_toglie_i_finali_e_lascia_sempre_i_normali() {
        let mut loadout = Loadout::default();
        loadout.berserk_charge = true;
        let finisher = Entry { flags: flag::FINISHER, ..entry(3, 12) };
        let normal = entry(5, 10);
        let raging = FighterView { berserk: true, finishers: 2, ..ground(3) };
        assert!(!available(&finisher, &loadout, &raging));
        assert!(available(&normal, &loadout, &raging));
        // Senza la condizione vera, o senza l'abilità, tutto torna normale.
        let calm = FighterView { berserk: false, ..raging };
        assert!(available(&finisher, &loadout, &calm));
        assert!(!available(&normal, &loadout, &calm));
        assert!(available(&finisher, &Loadout::default(), &raging));
        // Le altre condizioni valgono lo stesso.
        let air_normal = Entry { flags: flag::AERIAL, ..entry(9, 20) };
        assert!(!available(&air_normal, &loadout, &raging));
    }

    #[test]
    fn candidate_per_tipo() {
        let cat = catalog(vec![
            aimed(1, 10, kind::MAIN_TARGET),
            aimed(2, 11, kind::ANY_NEARBY),
            entry(3, 12),
            Entry { kind: kind::BY_COMMAND, ..entry(4, 13) },
        ]);
        let loadout = Loadout::default();
        let view = ground(1);
        // Nessun nemico: solo la voce "sempre candidata".
        let empty = Scene::default();
        assert_eq!(candidates(&cat, &loadout, &view, &empty), [false, false, true, false]);
        // Un nemico vicino che non è il bersaglio principale: tipo 2 sì, tipo 1 no.
        let nearby = [body(0.0, 0.0, 50.0)];
        let around = Scene { nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &loadout, &view, &around), [false, true, true, false]);
        // Il bersaglio principale a tiro.
        let locked = Scene { main_target: Some(nearby[0]), nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &loadout, &view, &locked), [true, true, true, false]);
        // Il bersaglio principale fuori tiro, un altro nemico a tiro.
        let far = Scene { main_target: Some(body(0.0, 0.0, 500.0)), nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &loadout, &view, &far), [false, true, true, false]);
    }

    #[test]
    fn una_voce_di_tipo_sconosciuto_non_e_mai_candidata_ma_un_comando_la_richiama() {
        let cat = catalog(vec![Entry { kind: 7, ..entry(60, 20) }, entry(5, 10)]);
        let loadout = Loadout::default();
        assert_eq!(candidates(&cat, &loadout, &ground(1), &Scene::default()), [false, true]);
        assert_eq!(choose(&cat, &loadout, &ground(1), &Scene::default()).unwrap().id, 5);
        // Il comando non guarda il tipo.
        assert_eq!(by_command(&cat, &loadout, &ground(1), 60).unwrap().motion, 20);
    }

    #[test]
    fn il_ripiego_senza_alternativa_parte_anche_se_ripete_l_animazione() {
        // Il ripiego (voce 5) non è disponibile, userebbe l'animazione in corso e non ha
        // un'alternativa: parte lo stesso.
        let cat = catalog(vec![Entry { ability: 70, ..entry(5, 10) }]);
        let view = FighterView { current_motion: Some(10), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 5);
    }

    #[test]
    fn una_voce_non_disponibile_non_e_candidata() {
        let cat = catalog(vec![Entry { ability: 9, ..entry(3, 12) }]);
        assert_eq!(candidates(&cat, &Loadout::default(), &ground(1), &Scene::default()), [false]);
    }

    fn one_target(entry: Entry, target: Body) -> bool {
        let cat = catalog(vec![entry]);
        let nearby = [target];
        let scene = Scene { main_target: Some(target), nearby: &nearby, ..Scene::default() };
        candidates(&cat, &Loadout::default(), &ground(1), &scene)[0]
    }

    #[test]
    fn distanza_vicino_compreso_lontano_escluso() {
        let ranged = Entry { near: 180.0, far: 300.0, ..aimed(6, 10, kind::MAIN_TARGET) };
        assert!(!one_target(ranged.clone(), body(0.0, 0.0, 179.0)));
        assert!(one_target(ranged.clone(), body(0.0, 0.0, 180.0)));
        assert!(one_target(ranged.clone(), body(0.0, 0.0, 299.0)));
        assert!(!one_target(ranged.clone(), body(0.0, 0.0, 300.0)));
        // La distanza è orizzontale: l'altezza non conta.
        assert!(one_target(ranged.clone(), body(180.0, 40.0, 0.0)));
    }

    #[test]
    fn dalla_distanza_si_toglie_il_raggio_e_non_si_va_sotto_zero() {
        let ranged = Entry { near: 180.0, far: 300.0, ..aimed(6, 10, kind::MAIN_TARGET) };
        let wide = Body { radius: 30.0, ..body(0.0, 0.0, 320.0) };
        assert!(one_target(ranged.clone(), wide));
        let wide_near = Body { radius: 30.0, ..body(0.0, 0.0, 200.0) };
        assert!(!one_target(ranged, wide_near));
        // Dentro il raggio la distanza vale zero, non un numero negativo.
        let close = aimed(6, 10, kind::MAIN_TARGET);
        let huge = Body { radius: 500.0, ..body(0.0, 0.0, 20.0) };
        assert!(one_target(close, huge));
    }

    #[test]
    fn dislivello_come_nell_esempio_dell_interfaccia() {
        // low = -20, high = -110: tra 20 (compreso) e 110 (escluso) unità più in alto.
        let high_up = Entry { low: -20.0, high: -110.0, ..aimed(6, 10, kind::MAIN_TARGET) };
        assert!(!one_target(high_up.clone(), body(0.0, 19.0, 50.0)));
        assert!(one_target(high_up.clone(), body(0.0, 20.0, 50.0)));
        assert!(one_target(high_up.clone(), body(0.0, 109.0, 50.0)));
        assert!(!one_target(high_up.clone(), body(0.0, 110.0, 50.0)));
    }

    #[test]
    fn il_dislivello_si_misura_dai_piedi_del_personaggio() {
        let high_up = Entry { low: -20.0, high: -110.0, ..aimed(6, 10, kind::MAIN_TARGET) };
        let cat = catalog(vec![high_up]);
        let target = body(0.0, 150.0, 50.0);
        let scene = Scene { fighter: [0.0, 100.0, 0.0], main_target: Some(target), ..Scene::default() };
        assert_eq!(candidates(&cat, &Loadout::default(), &ground(1), &scene), [true]);
    }

    #[test]
    fn la_taglia_allarga_i_limiti_di_dislivello() {
        let high_up = Entry { low: -20.0, high: -110.0, ..aimed(6, 10, kind::MAIN_TARGET) };
        let big = |height| Body { size: 10.0, ..body(0.0, height, 50.0) };
        // Limite basso corretto = -20 + 10 = -10: basta stare 10 più in alto.
        assert!(!one_target(high_up.clone(), big(9.0)));
        assert!(one_target(high_up.clone(), big(10.0)));
        // Limite alto corretto = -110 - 10 = -120.
        assert!(one_target(high_up.clone(), big(119.0)));
        assert!(!one_target(high_up.clone(), big(120.0)));
    }

    #[test]
    fn gli_interruttori_invertono_la_correzione_della_taglia() {
        let big = |height| Body { size: 10.0, ..body(0.0, height, 50.0) };
        let low_inverted = Entry { low: -20.0, high: -110.0, flags: flag::LOW_INVERTED, ..aimed(6, 10, kind::MAIN_TARGET) };
        // Limite basso corretto = -20 - 10 = -30.
        assert!(!one_target(low_inverted.clone(), big(29.0)));
        assert!(one_target(low_inverted.clone(), big(30.0)));
        assert!(one_target(low_inverted, big(119.0)));
        let high_inverted = Entry { low: -20.0, high: -110.0, flags: flag::HIGH_INVERTED, ..aimed(6, 10, kind::MAIN_TARGET) };
        // Limite alto corretto = -110 + 10 = -100.
        assert!(one_target(high_inverted.clone(), big(99.0)));
        assert!(!one_target(high_inverted.clone(), big(100.0)));
        assert!(one_target(high_inverted, big(10.0)));
    }

    #[test]
    fn la_dote_alza_il_limite_alto_solo_con_il_suo_interruttore() {
        let target = body(0.0, 130.0, 50.0);
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        let gifted = FighterView { reach_bonus: 30.0, ..ground(1) };
        let grows = catalog(vec![Entry { low: -20.0, high: -110.0, flags: flag::REACH_GROWS, ..aimed(6, 10, kind::MAIN_TARGET) }]);
        assert_eq!(candidates(&grows, &Loadout::default(), &ground(1), &scene), [false]);
        // Con la dote il limite alto diventa -110 - 30 = -140.
        assert_eq!(candidates(&grows, &Loadout::default(), &gifted, &scene), [true]);
        let fixed = catalog(vec![Entry { low: -20.0, high: -110.0, ..aimed(6, 10, kind::MAIN_TARGET) }]);
        assert_eq!(candidates(&fixed, &Loadout::default(), &gifted, &scene), [false]);
    }

    /// Una voce di tipo 2 con un intervallo di coseno, un bersaglio principale davanti
    /// (verso +z) e un secondo nemico nella direzione data.
    fn angle_passes(inner_min: f32, inner_max: f32, x: f32, z: f32) -> bool {
        let cat = catalog(vec![Entry { inner_min, inner_max, ..aimed(4, 10, kind::ANY_NEARBY) }]);
        let nearby = [body(x, 0.0, z)];
        let scene = Scene {
            main_target: Some(body(0.0, 0.0, 500.0)),
            nearby: &nearby,
            ..Scene::default()
        };
        candidates(&cat, &Loadout::default(), &ground(1), &scene)[0]
    }

    #[test]
    fn angolo_minimo_escluso_massimo_compreso() {
        // Coseno 0 (di lato) con limiti -0,5..0: il massimo è compreso.
        assert!(angle_passes(-0.5, 0.0, 50.0, 0.0));
        // Coseno 0 con limiti 0..0,5: il minimo è escluso.
        assert!(!angle_passes(0.0, 0.5, 50.0, 0.0));
        // Davanti: coseno 1, che non supera mai 1.
        assert!(angle_passes(0.99, 1.0, 0.0, 50.0));
        assert!(!angle_passes(-1.0, 0.9239, 0.0, 50.0));
        // Dietro: coseno -1; con minimo -1 resta escluso.
        assert!(!angle_passes(-1.0, 0.0, 0.0, -50.0));
        assert!(angle_passes(-1.5, 0.0, 0.0, -50.0));
    }

    #[test]
    fn l_angolo_si_misura_dalla_direzione_del_bersaglio_principale() {
        // Il bersaglio principale sta verso +x: un nemico verso +x è "davanti".
        let cat = catalog(vec![Entry { inner_min: 0.99, inner_max: 1.0, ..aimed(4, 10, kind::ANY_NEARBY) }]);
        let nearby = [body(50.0, 0.0, 0.0)];
        let toward_x = Scene { main_target: Some(body(400.0, 0.0, 0.0)), nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &Loadout::default(), &ground(1), &toward_x), [true]);
        // Senza bersaglio principale conta dove guarda il personaggio (qui +z).
        let no_target = Scene { nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &Loadout::default(), &ground(1), &no_target), [false]);
        let facing_x = Scene { facing: [1.0, 0.0], nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &Loadout::default(), &ground(1), &facing_x), [true]);
    }

    #[test]
    fn bersaglio_principale_esattamente_sopra_si_usa_dove_guarda_il_personaggio() {
        let cat = catalog(vec![Entry { inner_min: 0.99, inner_max: 1.0, ..aimed(4, 10, kind::ANY_NEARBY) }]);
        let nearby = [body(50.0, 0.0, 0.0)];
        let above = Scene { facing: [1.0, 0.0], main_target: Some(body(0.0, 30.0, 0.0)), nearby: &nearby, ..Scene::default() };
        assert_eq!(candidates(&cat, &Loadout::default(), &ground(1), &above), [true]);
    }

    #[test]
    fn un_nemico_esattamente_sopra_ha_coseno_uno() {
        let strict = Entry { inner_min: 0.99996, inner_max: 1.0, ..aimed(1, 10, kind::MAIN_TARGET) };
        assert!(one_target(strict.clone(), body(0.0, 30.0, 0.0)));
        // E il bersaglio principale, visto da sé stesso, è sempre "proprio il bersaglio".
        assert!(one_target(strict, body(37.3, 0.0, -61.9)));
    }

    #[test]
    fn il_bordo_rende_candidata_la_voce_uno_anche_se_non_disponibile() {
        let cat = catalog(vec![
            entry(5, 10),
            Entry { kind: kind::BY_COMMAND, ability: 99, ..entry(1, 11) },
        ]);
        let loadout = Loadout::default();
        let edge = Scene { ledge_ahead: true, ..Scene::default() };
        assert_eq!(candidates(&cat, &loadout, &ground(1), &Scene::default()), [true, false]);
        assert_eq!(candidates(&cat, &loadout, &ground(1), &edge), [true, true]);
        // In aria il bordo non conta.
        let air_view = FighterView { airborne: true, ..ground(1) };
        assert_eq!(candidates(&cat, &loadout, &air_view, &edge), [false, false]);
    }

    #[test]
    fn vince_la_prima_candidata_nell_ordine_del_repertorio() {
        let cat = catalog(vec![
            Entry { ability: 50, ..entry(7, 20) },
            entry(8, 21),
            entry(5, 10),
        ]);
        let mut loadout = Loadout::default();
        let chosen = choose(&cat, &loadout, &ground(1), &Scene::default()).unwrap();
        assert_eq!(chosen.id, 8);
        loadout.equip(50);
        assert_eq!(choose(&cat, &loadout, &ground(1), &Scene::default()).unwrap().id, 7);
    }

    #[test]
    fn mai_la_stessa_animazione_due_volte_di_fila() {
        let cat = catalog(vec![entry(8, 21), entry(5, 10)]);
        let view = FighterView { current_motion: Some(21), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 5);
    }

    #[test]
    fn se_non_resta_nessuna_candidata_parte_l_alternativa_della_prima_saltata() {
        // La voce 6 non sarebbe né disponibile (abilità) né candidata (tipo 0): parte lo stesso.
        let cat = catalog(vec![
            Entry { alternative: 6, ..entry(5, 10) },
            Entry { kind: kind::BY_COMMAND, ability: 77, ..entry(6, 30) },
        ]);
        let view = FighterView { current_motion: Some(10), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 6);
    }

    #[test]
    fn un_altra_candidata_vince_sull_alternativa() {
        let cat = catalog(vec![
            Entry { alternative: 6, ..entry(5, 10) },
            Entry { kind: kind::BY_COMMAND, ..entry(6, 30) },
            entry(7, 40),
        ]);
        let view = FighterView { current_motion: Some(10), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 7);
    }

    #[test]
    fn ci_si_ricorda_l_alternativa_della_prima_saltata_che_ne_ha_una() {
        // Tre voci con la stessa animazione: la prima non ha alternativa, la seconda ne
        // indica una che non esiste, la terza e la quarta ne hanno una vera.
        let cat = catalog(vec![
            entry(1, 10),
            Entry { alternative: 99, ..entry(2, 10) },
            Entry { alternative: 21, ..entry(3, 10) },
            Entry { alternative: 22, ..entry(4, 10) },
            Entry { kind: kind::BY_COMMAND, ..entry(21, 31) },
            Entry { kind: kind::BY_COMMAND, ..entry(22, 32) },
        ]);
        let view = FighterView { current_motion: Some(10), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 21);
    }

    #[test]
    fn una_voce_che_indica_se_stessa_puo_ripetersi() {
        let cat = catalog(vec![Entry { alternative: 5, ..entry(5, 10) }, entry(7, 40)]);
        let view = FighterView { current_motion: Some(10), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 5);
    }

    #[test]
    fn senza_animazione_in_corso_niente_viene_saltato() {
        let cat = catalog(vec![entry(8, 21), entry(5, 10)]);
        assert_eq!(choose(&cat, &Loadout::default(), &ground(1), &Scene::default()).unwrap().id, 8);
    }

    #[test]
    fn senza_candidate_parte_il_ripiego_di_terra_o_di_aria() {
        // Nessuna delle due voci di ripiego è disponibile (abilità mancante).
        let cat = catalog(vec![
            aimed(4, 20, kind::ANY_NEARBY),
            Entry { ability: 70, ..entry(5, 10) },
            Entry { ability: 70, flags: flag::AERIAL, ..entry(9, 11) },
        ]);
        let loadout = Loadout::default();
        assert_eq!(choose(&cat, &loadout, &ground(1), &Scene::default()).unwrap().id, 5);
        let air_view = FighterView { airborne: true, ..ground(1) };
        assert_eq!(choose(&cat, &loadout, &air_view, &Scene::default()).unwrap().id, 9);
    }

    #[test]
    fn il_ripiego_che_ripeterebbe_l_animazione_lascia_il_posto_alla_sua_alternativa() {
        let cat = catalog(vec![
            Entry { ability: 70, alternative: 6, ..entry(5, 10) },
            Entry { kind: kind::BY_COMMAND, ..entry(6, 30) },
        ]);
        let view = FighterView { current_motion: Some(10), ..ground(2) };
        assert_eq!(choose(&cat, &Loadout::default(), &view, &Scene::default()).unwrap().id, 6);
    }

    #[test]
    fn senza_nemmeno_il_ripiego_non_parte_niente() {
        let cat = catalog(vec![Entry { ability: 70, ..entry(7, 10) }]);
        assert!(choose(&cat, &Loadout::default(), &ground(1), &Scene::default()).is_none());
    }

    #[test]
    fn il_bordo_fa_partire_la_voce_uno_se_viene_prima_nel_repertorio() {
        let cat = catalog(vec![
            Entry { kind: kind::BY_COMMAND, jump: 21.0, ..entry(1, 11) },
            entry(5, 10),
        ]);
        let edge = Scene { ledge_ahead: true, ..Scene::default() };
        assert_eq!(choose(&cat, &Loadout::default(), &ground(1), &edge).unwrap().id, 1);
        assert_eq!(choose(&cat, &Loadout::default(), &ground(1), &Scene::default()).unwrap().id, 5);
    }

    #[test]
    fn un_comando_richiama_la_prima_voce_con_quel_numero_se_disponibile() {
        let cat = catalog(vec![
            entry(5, 10),
            Entry { kind: kind::BY_COMMAND, ability: 1, ..entry(11, 40) },
            Entry { kind: kind::BY_COMMAND, ..entry(11, 41) },
        ]);
        let mut loadout = Loadout::default();
        assert!(by_command(&cat, &loadout, &ground(1), 11).is_none());
        loadout.equip(1);
        assert_eq!(by_command(&cat, &loadout, &ground(1), 11).unwrap().motion, 40);
        assert!(by_command(&cat, &loadout, &ground(1), 12).is_none());
        // Un comando può richiamare anche una voce che non è di tipo 0.
        assert_eq!(by_command(&cat, &loadout, &ground(1), 5).unwrap().motion, 10);
    }
}
