//! Altezza e slancio: salto, balzi dei colpi, caduta, atterraggio (SPEC, sezioni 7 e 9).
//!
//! SPEC 13 (da NON riprodurre): nel gioco l'altezza cambia a scatti ogni 2 tick. Qui salita
//! e caduta si calcolano a ogni tick con le formule della sezione 9; solo lo slancio
//! orizzontale cala ogni 2 tick.

/// Quanto sale un salto pieno.
pub const JUMP_HEIGHT: f32 = 185.0;
/// Accelerazione di caduta, per tick al quadrato.
pub const GRAVITY: f32 = 0.408;

/// Quanti tick dura la salita di un salto pieno.
const JUMP_RISE_TICKS: f32 = 28.0;
/// Fino a quale tick un salto pieno resta sospeso in cima, prima di cadere.
const JUMP_HANG_TICKS: f32 = 30.12;
/// Tick di salita che un salto fa comunque, anche se il tasto viene rilasciato subito.
const SHORTEST_JUMP_TICKS: u32 = 7;
/// La velocità verso l'alto più grande con cui si passa alla caduta dopo il rilascio.
const RELEASE_SPEED_CAP: f32 = 2.04;
/// Di quanto cala lo slancio a ogni calo: un ventesimo.
const DRIFT_KEEP: f32 = 0.95;

/// La curva di salita (SPEC 9): frazione dell'altezza raggiunta a 0/14, 1/14 ... 14/14
/// della durata. Tra un punto e l'altro si va in linea retta.
const RISE_CURVE: [f32; 15] = [
    0.0, 0.1627, 0.3195, 0.4649, 0.5946, 0.7054, 0.7957, 0.8665, 0.9184, 0.9546, 0.9773, 0.9903,
    0.9968, 0.9995, 1.0,
];

/// Frazione dell'altezza raggiunta a una frazione della durata.
fn rise_curve(fraction: f32) -> f32 {
    if fraction <= 0.0 {
        return 0.0;
    }
    if fraction >= 1.0 {
        return 1.0;
    }
    let position = fraction * (RISE_CURVE.len() - 1) as f32;
    let index = (position as usize).min(RISE_CURVE.len() - 2);
    let within = position - index as f32;
    RISE_CURVE[index] + (RISE_CURVE[index + 1] - RISE_CURVE[index]) * within
}

/// Quanti tick dura una salita alta `rise` (zero se `rise` è negativa): SPEC 9.
pub fn rise_ticks(rise: f32) -> f32 {
    if rise <= 0.0 {
        return 0.0;
    }
    JUMP_RISE_TICKS * (rise / JUMP_HEIGHT).sqrt()
}

/// Fino a quale tick, contando dall'inizio della salita, il personaggio resta sospeso.
fn hang_ticks(rise: f32) -> f32 {
    if rise <= 0.0 {
        return 0.0;
    }
    JUMP_HANG_TICKS * (rise / JUMP_HEIGHT).sqrt()
}

/// Quanta strada fa in `ticks` tick uno slancio che parte a velocità 1 (SPEC 7, la forma
/// che vale anche per tempi non interi; zero se `ticks` è negativo).
pub fn drift_reach(ticks: f32) -> f32 {
    if ticks <= 0.0 {
        return 0.0;
    }
    38.0 * (1.0 - DRIFT_KEEP.powf(ticks / 2.0))
}

/// Lo slancio orizzontale di un colpo in volo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drift {
    direction: [f32; 2],
    speed: f32,
    /// La prossima chiamata è una di quelle in cui la velocità cala (prima, terza, quinta...).
    slows_next: bool,
}

impl Drift {
    /// `direction`: verso dove (x, z), di lunghezza 1. `speed`: velocità di partenza, per tick.
    pub fn new(direction: [f32; 2], speed: f32) -> Self {
        Drift {
            direction,
            speed,
            slows_next: true,
        }
    }

    /// Avanza di un tick e dice lo spostamento (x, z) di questo tick. Alla prima chiamata e
    /// poi a una chiamata sì e una no (terza, quinta...), prima di rispondere la velocità
    /// perde un ventesimo.
    pub fn tick(&mut self) -> [f32; 2] {
        if self.slows_next {
            self.speed *= DRIFT_KEEP;
        }
        self.slows_next = !self.slows_next;
        [self.direction[0] * self.speed, self.direction[1] * self.speed]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// A terra.
    Ground,
    /// In salita o sospeso in cima: partito dall'altezza `base`, sale di `rise`; `ticks`
    /// sono i tick già passati. `jump`: è un salto (il rilascio del tasto lo accorcia) e
    /// non il balzo di un colpo.
    Rising {
        base: f32,
        rise: f32,
        ticks: u32,
        jump: bool,
    },
    /// In caduta, con questa velocità verso il basso (negativa: sta ancora salendo).
    Falling { speed: f32 },
}

/// L'altezza del personaggio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air {
    height: f32,
    phase: Phase,
}

impl Default for Air {
    /// A terra, all'altezza zero.
    fn default() -> Self {
        Air {
            height: 0.0,
            phase: Phase::Ground,
        }
    }
}

impl Air {
    pub fn height(&self) -> f32 {
        self.height
    }

    /// Falso solo quando è a terra.
    pub fn airborne(&self) -> bool {
        self.phase != Phase::Ground
    }

    /// Vero solo durante la salita (sospensione compresa) di un **salto**, non di un balzo.
    pub fn jumping(&self) -> bool {
        matches!(self.phase, Phase::Rising { jump: true, .. })
    }

    /// Comincia un salto (una salita di JUMP_HEIGHT dall'altezza attuale). Se è già in aria
    /// non fa niente e restituisce falso.
    pub fn jump(&mut self) -> bool {
        if self.airborne() {
            return false;
        }
        self.phase = Phase::Rising {
            base: self.height,
            rise: JUMP_HEIGHT,
            ticks: 0,
            jump: true,
        };
        true
    }

    /// Il balzo di un colpo: comincia una salita di `rise` dall'altezza attuale, da terra o in
    /// aria, al posto di ciò che stava facendo. Con `rise` zero o negativa comincia a cadere
    /// da fermo.
    pub fn hop(&mut self, rise: f32) {
        // Scritto così perché anche un valore che non è un numero finisca nella caduta.
        self.phase = if rise > 0.0 {
            Phase::Rising {
                base: self.height,
                rise,
                ticks: 0,
                jump: false,
            }
        } else {
            Phase::Falling { speed: 0.0 }
        };
    }

    /// Avanza di un tick (SPEC 9). `jump_held`: il tasto del salto è giù. `floor`: l'altezza
    /// del terreno sotto il personaggio. Restituisce vero nel tick in cui tocca terra.
    pub fn tick(&mut self, jump_held: bool, floor: f32) -> bool {
        match self.phase {
            Phase::Ground => {
                if floor < self.height {
                    // È uscito da un bordo: comincia a cadere da fermo, e in questo tick
                    // l'altezza non cambia.
                    self.phase = Phase::Falling { speed: 0.0 };
                } else {
                    self.height = floor;
                }
                false
            }
            Phase::Rising {
                base,
                rise,
                ticks,
                jump,
            } => {
                // Durante una salita o una sospensione il terreno non viene guardato.
                let now = ticks + 1;
                let elapsed = now as f32;
                let duration = rise_ticks(rise);
                if elapsed <= duration {
                    let target = base + rise * rise_curve(elapsed / duration);
                    // Conta solo se il tasto è giù in questo tick: un rilascio in uno dei
                    // tick prima non viene ricordato (R7).
                    if jump && !jump_held && now > SHORTEST_JUMP_TICKS {
                        // Tasto rilasciato: si passa alla caduta con la velocità verso
                        // l'alto che la curva avrebbe dato in questo tick (con un tetto),
                        // e in questo stesso tick si fa già il primo passo di caduta.
                        let upward = (target - self.height).min(RELEASE_SPEED_CAP);
                        self.phase = Phase::Falling { speed: -upward };
                        return self.fall(floor);
                    }
                    self.height = target;
                    self.phase = Phase::Rising {
                        base,
                        rise,
                        ticks: now,
                        jump,
                    };
                    return false;
                }
                let hang = hang_ticks(rise);
                if elapsed <= hang {
                    self.height = base + rise;
                    self.phase = Phase::Rising {
                        base,
                        rise,
                        ticks: now,
                        jump,
                    };
                    return false;
                }
                // Primo tick di caduta: conta la frazione di tick passata oltre la fine
                // della sospensione.
                let beyond = elapsed - hang;
                self.height = base + rise - GRAVITY / 2.0 * beyond * beyond;
                self.phase = Phase::Falling {
                    speed: GRAVITY * beyond,
                };
                self.touch_down(floor)
            }
            Phase::Falling { .. } => self.fall(floor),
        }
    }

    /// Un passo di caduta: l'altezza cambia della velocità che aveva, meno metà
    /// dell'accelerazione; poi la velocità di caduta cresce.
    fn fall(&mut self, floor: f32) -> bool {
        if let Phase::Falling { speed } = self.phase {
            self.height -= speed + GRAVITY / 2.0;
            self.phase = Phase::Falling {
                speed: speed + GRAVITY,
            };
        }
        self.touch_down(floor)
    }

    /// Se l'altezza è arrivata al terreno o sotto, il personaggio è a terra, esattamente
    /// all'altezza del terreno.
    fn touch_down(&mut self, floor: f32) -> bool {
        if self.height <= floor {
            self.height = floor;
            self.phase = Phase::Ground;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un terreno così in basso che non ci si arriva mai.
    const NO_FLOOR: f32 = -1.0e9;

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn durata_della_salita() {
        assert_eq!(rise_ticks(JUMP_HEIGHT), 28.0);
        assert!(close(rise_ticks(21.0), 28.0 * (21.0f32 / 185.0).sqrt(), 1e-4));
        assert!(close(rise_ticks(46.25), 14.0, 1e-4));
        assert_eq!(rise_ticks(0.0), 0.0);
        assert_eq!(rise_ticks(-5.0), 0.0);
    }

    #[test]
    fn strada_dello_slancio() {
        // Con v = 8: circa 155 unità in 28 tick, mai più di 304.
        assert!(close(8.0 * drift_reach(28.0), 155.7, 0.5));
        assert!(8.0 * drift_reach(10_000.0) <= 304.0);
        assert!(close(8.0 * drift_reach(10_000.0), 304.0, 0.01));
        assert_eq!(drift_reach(0.0), 0.0);
        assert_eq!(drift_reach(-3.0), 0.0);
        // In un numero pari di tick è la somma delle coppie: 2·(0,95 + 0,95²) in 4 tick.
        assert!(close(drift_reach(4.0), 2.0 * (0.95 + 0.95 * 0.95), 1e-4));
    }

    #[test]
    fn lo_slancio_perde_un_ventesimo_ogni_due_tick_e_il_primo_calo_e_subito() {
        let mut drift = Drift::new([0.0, 1.0], 8.0);
        let steps: Vec<f32> = (0..5).map(|_| drift.tick()[1]).collect();
        for (step, expected) in steps.iter().zip([7.6, 7.6, 7.22, 7.22, 6.859]) {
            assert!(close(*step, expected, 1e-4), "{steps:?}");
        }
    }

    #[test]
    fn lo_slancio_segue_la_direzione_e_torna_con_la_formula() {
        let mut drift = Drift::new([0.6, -0.8], 8.0);
        let mut x = 0.0;
        let mut z = 0.0;
        for _ in 0..28 {
            let step = drift.tick();
            x += step[0];
            z += step[1];
        }
        let road = (x * x + z * z).sqrt();
        assert!(close(road, 8.0 * drift_reach(28.0), 0.01));
        assert!(close(x / road, 0.6, 1e-4) && close(z / road, -0.8, 1e-4));
    }

    #[test]
    fn appena_creato_e_a_terra_all_altezza_zero() {
        let air = Air::default();
        assert_eq!(air.height(), 0.0);
        assert!(!air.airborne() && !air.jumping());
    }

    #[test]
    fn a_terra_segue_il_terreno_se_non_scende() {
        let mut air = Air::default();
        assert!(!air.tick(false, 12.5));
        assert_eq!(air.height(), 12.5);
        assert!(!air.airborne());
    }

    #[test]
    fn uscito_da_un_bordo_comincia_a_cadere_da_fermo() {
        let mut air = Air::default();
        air.tick(false, 50.0);
        // Il terreno scende a 10: in questo tick l'altezza non cambia.
        assert!(!air.tick(false, 10.0));
        assert_eq!(air.height(), 50.0);
        assert!(air.airborne() && !air.jumping());
        assert!(!air.tick(false, 10.0));
        assert!(close(air.height(), 50.0 - 0.204, 1e-4));
        assert!(!air.tick(false, 10.0));
        assert!(close(air.height(), 50.0 - 0.204 - 0.612, 1e-4));
    }

    #[test]
    fn la_caduta_accelera_e_finisce_esattamente_sul_terreno() {
        let mut air = Air::default();
        air.tick(false, 20.0);
        air.tick(false, 0.0);
        let mut ticks = 0;
        while !air.tick(false, 0.0) {
            ticks += 1;
            // Dopo n passi di caduta da fermo: 0,204 · n².
            assert!(close(air.height(), 20.0 - 0.204 * (ticks * ticks) as f32, 1e-3));
            assert!(ticks < 100);
        }
        // 0,204 · n² supera 20 al decimo passo.
        assert_eq!(ticks, 9);
        assert_eq!(air.height(), 0.0);
        assert!(!air.airborne());
        // Da lì in poi resta a terra, senza altri atterraggi.
        assert!(!air.tick(false, 0.0));
    }

    #[test]
    fn il_salto_non_cambia_l_altezza_finche_non_passa_un_tick() {
        let mut air = Air::default();
        assert!(air.jump());
        assert_eq!(air.height(), 0.0);
        assert!(air.airborne() && air.jumping());
        // Già in aria: un secondo salto non parte.
        assert!(!air.jump());
        air.tick(true, 0.0);
        // Primo passo della curva: 15 unità circa.
        assert!(close(air.height(), 185.0 * 0.1627 / 2.0, 1e-3));
    }

    #[test]
    fn il_salto_tenuto_segue_la_curva_fino_a_185() {
        let mut air = Air::default();
        air.jump();
        let mut heights = vec![0.0];
        for _ in 0..28 {
            assert!(!air.tick(true, 0.0));
            heights.push(air.height());
        }
        // I punti della tabella cadono sui tick pari.
        assert!(close(heights[2], 185.0 * 0.1627, 1e-3));
        assert!(close(heights[14], 185.0 * 0.8665, 1e-3));
        assert!(close(heights[26], 185.0 * 0.9995, 1e-3));
        assert!(close(heights[28], 185.0, 1e-3));
        // Tra un punto e l'altro, in linea retta.
        assert!(close(heights[7], 185.0 * (0.4649 + 0.5946) / 2.0, 1e-3));
        assert!(heights.windows(2).all(|pair| pair[1] > pair[0]));
    }

    #[test]
    fn dopo_la_salita_resta_sospeso_poi_cade() {
        let mut air = Air::default();
        air.jump();
        for _ in 0..28 {
            air.tick(true, 0.0);
        }
        // Tick 29 e 30: sospeso in cima, ed è ancora "salto".
        for _ in 0..2 {
            air.tick(true, 0.0);
            assert!(close(air.height(), 185.0, 1e-3));
            assert!(air.jumping());
        }
        // Tick 31: è passato 0,88 di tick oltre il 30,12.
        air.tick(true, 0.0);
        assert!(!air.jumping() && air.airborne());
        assert!(close(air.height(), 185.0 - 0.204 * 0.88 * 0.88, 1e-3));
        // Tick 32: scende della velocità che aveva (0,408 · 0,88) più 0,204.
        let before = air.height();
        air.tick(true, 0.0);
        assert!(close(before - air.height(), 0.408 * 0.88 + 0.204, 1e-3));
    }

    #[test]
    fn un_tocco_da_un_salto_con_l_apice_a_103_circa() {
        let mut air = Air::default();
        air.jump();
        let mut heights = Vec::new();
        let mut apex = 0.0f32;
        for tick in 1..=40 {
            air.tick(false, NO_FLOOR);
            heights.push(air.height());
            apex = apex.max(air.height());
            // Fino al settimo tick sale lungo la curva, come se il tasto fosse giù.
            assert_eq!(air.jumping(), tick <= 7, "tick {tick}");
        }
        assert!(close(heights[6], 185.0 * (0.4649 + 0.5946) / 2.0, 1e-3));
        // Ottavo tick: primo di caduta, con velocità verso l'alto 2,04 (il tetto).
        assert!(close(heights[7] - heights[6], 2.04 - 0.204, 1e-3));
        assert!(close(heights[8] - heights[7], 2.04 - 0.408 - 0.204, 1e-3));
        assert!(close(apex, 103.1, 0.2), "apice {apex}");
    }

    #[test]
    fn il_rilascio_piu_tardi_interrompe_la_salita_in_quel_tick() {
        let mut air = Air::default();
        air.jump();
        for _ in 0..20 {
            air.tick(true, NO_FLOOR);
        }
        let before = air.height();
        // Al tick 21 la curva salirebbe di poco più di un'unità: sotto il tetto.
        let would_rise = 185.0 * ((0.9773 + 0.9903) / 2.0 - 0.9773);
        air.tick(false, NO_FLOOR);
        assert!(!air.jumping());
        assert!(close(air.height() - before, would_rise - 0.204, 1e-3));
    }

    #[test]
    fn durante_la_sospensione_il_rilascio_non_cambia_niente() {
        let mut held = Air::default();
        let mut released = Air::default();
        held.jump();
        released.jump();
        for tick in 1..=45 {
            held.tick(true, NO_FLOOR);
            released.tick(tick <= 28, NO_FLOOR);
            assert_eq!(held.height(), released.height(), "tick {tick}");
        }
    }

    #[test]
    fn il_balzo_di_un_colpo_sale_dall_altezza_attuale() {
        let mut air = Air::default();
        air.jump();
        for _ in 0..10 {
            air.tick(true, 0.0);
        }
        let base = air.height();
        air.hop(21.0);
        assert_eq!(air.height(), base);
        assert!(air.airborne() && !air.jumping());
        let duration = rise_ticks(21.0);
        assert!(duration > 9.0 && duration < 10.0);
        for _ in 0..9 {
            air.tick(false, 0.0);
            assert!(air.height() > base && air.height() < base + 21.0);
        }
        // Il tick 10 è oltre la fine della salita: è in cima, sospeso (fino al tick 10,15).
        air.tick(false, 0.0);
        assert!(close(air.height(), base + 21.0, 1e-3));
        // Il tick 11 è il primo di caduta.
        air.tick(false, 0.0);
        let beyond = 11.0 - 30.12 * (21.0f32 / 185.0).sqrt();
        assert!(close(air.height(), base + 21.0 - 0.204 * beyond * beyond, 1e-3));
    }

    #[test]
    fn il_rilascio_non_accorcia_i_balzi() {
        let mut air = Air::default();
        air.hop(67.0);
        let mut top = 0.0f32;
        for _ in 0..30 {
            air.tick(false, 0.0);
            top = top.max(air.height());
        }
        assert!(close(top, 67.0, 1e-3));
    }

    #[test]
    fn un_balzo_da_terra_stacca_il_personaggio() {
        let mut air = Air::default();
        air.hop(130.0);
        assert!(air.airborne());
        // Durante la salita il terreno non viene guardato, nemmeno se è più in alto.
        assert!(!air.tick(false, 500.0));
        assert!(air.airborne());
        assert!(air.height() > 0.0 && air.height() < 130.0);
    }

    #[test]
    fn un_balzo_nullo_o_negativo_comincia_a_cadere_da_fermo() {
        for rise in [0.0, -12.0] {
            let mut air = Air::default();
            air.tick(false, 100.0);
            air.jump();
            for _ in 0..10 {
                air.tick(true, 100.0);
            }
            let base = air.height();
            air.hop(rise);
            assert!(air.airborne() && !air.jumping());
            air.tick(true, 100.0);
            assert!(close(air.height(), base - 0.204, 1e-3));
        }
        // Da terra: un tick dopo è di nuovo a terra.
        let mut air = Air::default();
        air.hop(0.0);
        assert!(air.airborne());
        assert!(air.tick(false, 0.0));
        assert_eq!(air.height(), 0.0);
    }

    #[test]
    fn atterra_nel_tick_in_cui_arriva_al_terreno() {
        let mut air = Air::default();
        air.jump();
        let mut landed_at = None;
        for tick in 1..=200 {
            if air.tick(true, 0.0) {
                landed_at = Some(tick);
                break;
            }
            assert!(air.airborne());
        }
        // 28 tick di salita, 2 sospeso, poi la caduta da 185: radice(185 / 0,204) = 30,1 tick.
        let landed_at = landed_at.expect("deve atterrare");
        assert!((60..=62).contains(&landed_at), "atterrato al tick {landed_at}");
        assert_eq!(air.height(), 0.0);
        assert!(!air.airborne());
    }

    #[test]
    fn atterra_anche_su_un_terreno_piu_alto_di_quello_di_partenza() {
        let mut air = Air::default();
        air.jump();
        for _ in 0..31 {
            air.tick(true, 0.0);
        }
        // Un gradino alto 150 sotto i piedi mentre cade da 185.
        let mut landed = false;
        for _ in 0..40 {
            if air.tick(true, 150.0) {
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert_eq!(air.height(), 150.0);
    }
}
