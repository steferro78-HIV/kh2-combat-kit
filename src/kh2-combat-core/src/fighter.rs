//! Il personaggio intero: la combo, l'altezza e le regole che le legano (SPEC 7, 9 e 10).

use std::sync::Arc;

use crate::air::{Air, Drift};
use crate::catalog::{motion, trigger, Catalog};
use crate::combo::{Combo, Event, TickInput};
use crate::rules::{self, Loadout, Scene};

/// Ciò che chi usa la libreria dice al personaggio a ogni tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls<'a> {
    pub attack_pressed: bool,
    pub command: Option<u8>,
    /// Il tasto del salto è passato da rilasciato a premuto in questo tick.
    pub jump_pressed: bool,
    /// Il tasto del salto è giù.
    pub jump_held: bool,
    pub hit_landed: bool,
    pub busy: bool,
    pub berserk: bool,
    /// L'altezza del terreno sotto il personaggio.
    pub floor: f32,
    /// L'altezza del personaggio (`fighter[1]`) la mette `Fighter`.
    pub scene: Scene<'a>,
}

/// Un personaggio: la combo, l'altezza e le regole che le legano.
#[derive(Clone, Debug)]
pub struct Fighter {
    pub combo: Combo,
    air: Air,
    /// Lo slancio del colpo in volo in corso, se c'è.
    drift: Option<Drift>,
    /// Lo spostamento orizzontale dell'ultimo tick.
    slide: [f32; 2],
    /// Il frame del salto: 1 nel tick in cui parte; 0 quando non sta saltando.
    jump_frame: u16,
    /// Il frame della caduta da mostrare; 0 quando non si sta mostrando la caduta.
    fall_frame: u16,
    /// Il frame dell'atterraggio da mostrare, finché dura.
    land_frame: Option<u16>,
}

impl Fighter {
    /// Un personaggio appena creato è a terra all'altezza zero, fermo, senza slancio.
    pub fn new(catalog: Arc<Catalog>, loadout: Loadout) -> Self {
        Fighter {
            combo: Combo::new(catalog, loadout),
            air: Air::default(),
            drift: None,
            slide: [0.0, 0.0],
            jump_frame: 0,
            fall_frame: 0,
            land_frame: None,
        }
    }

    /// L'altezza attuale.
    pub fn height(&self) -> f32 {
        self.air.height()
    }

    /// Vero se non è a terra.
    pub fn airborne(&self) -> bool {
        self.air.airborne()
    }

    /// Lo spostamento orizzontale (x, z) dovuto allo slancio nell'ultimo `tick`: chi usa la
    /// libreria lo somma alla posizione.
    pub fn slide(&self) -> [f32; 2] {
        self.slide
    }

    /// Quale animazione mostrare: (numero, frame da 1); `None` quando il personaggio è
    /// fermo o cammina.
    pub fn shown(&self) -> Option<(u16, u16)> {
        if let Some(current) = self.combo.current_move() {
            return Some(current);
        }
        if self.air.airborne() {
            if self.air.jumping() {
                let last = self.duration(motion::JUMP).saturating_sub(1);
                return Some((motion::JUMP, self.jump_frame.min(last).max(1)));
            }
            return Some((motion::FALL, self.fall_frame.max(1)));
        }
        self.land_frame.map(|frame| (motion::LAND, frame))
    }

    /// L'arma è stata respinta da una parata nemica.
    pub fn deflected(&mut self) -> Vec<Event> {
        self.combo.deflected(self.air.airborne())
    }

    /// Un colpo nemico è finito sulla parata.
    pub fn guarded(&mut self) -> Vec<Event> {
        self.combo.guarded()
    }

    /// Il personaggio ha subito un colpo; se `rise` è maggiore di zero il colpo lo sbalza
    /// in aria di quell'altezza.
    pub fn knocked(&mut self, reaction: u16, rise: f32) -> Vec<Event> {
        if rise > 0.0 {
            self.air.hop(rise);
        }
        self.combo.knocked(reaction, self.air.airborne())
    }

    /// Avanza di un tick.
    pub fn tick(&mut self, controls: Controls) -> Vec<Event> {
        let mut events = Vec::new();

        // 1. Salto: solo da terra e solo quando il personaggio non è impegnato.
        if controls.jump_pressed && !controls.busy && !self.air.airborne() && self.combo.free() {
            events.extend(self.combo.interrupt());
            self.air.jump();
            self.jump_frame = 0;
            events.push(Event::Jumped);
        }

        // 2. Altezza. Toccare terra interrompe l'animazione in corso.
        let landed = self.air.tick(controls.jump_held, controls.floor);
        if landed {
            events.extend(self.combo.interrupt());
            events.push(Event::Landed);
        }
        self.jump_frame = if self.air.jumping() {
            self.jump_frame.saturating_add(1)
        } else {
            0
        };

        // 3. Combo.
        let mut scene = controls.scene;
        scene.fighter[1] = self.air.height();
        let from_combo = self.combo.tick(TickInput {
            attack_pressed: controls.attack_pressed,
            command: controls.command,
            recover_pressed: controls.jump_pressed,
            hit_landed: controls.hit_landed,
            busy: controls.busy || self.jump_starting(),
            airborne: self.air.airborne(),
            berserk: controls.berserk,
            scene,
        });

        // 4. Balzo e slancio dei colpi in volo partiti in questo tick.
        for event in &from_combo {
            if let Event::Lunge { rise, speed, .. } = *event {
                self.air.hop(rise);
                self.drift = Some(Drift::new(rules::reference_direction(&scene), speed));
            }
        }
        events.extend(from_combo);

        // 5. Lo slancio dura quanto il colpo in volo che lo ha dato.
        if !self.combo.current_entry().is_some_and(|entry| entry.lunges()) {
            self.drift = None;
        }
        self.slide = match self.drift.as_mut() {
            Some(drift) => drift.tick(),
            None => [0.0, 0.0],
        };

        self.update_shown(landed);
        events
    }

    /// Il salto è ancora nei suoi primi frame: l'animazione del salto non dichiara ancora
    /// il personaggio "in aria", quindi è occupato (SPEC 9).
    fn jump_starting(&self) -> bool {
        if self.jump_frame == 0 {
            return false;
        }
        self.combo
            .catalog()
            .motion(motion::JUMP)
            .and_then(|jump| jump.first_frame_of(trigger::AIRBORNE))
            .is_some_and(|first| self.jump_frame < first)
    }

    /// La durata di un'animazione; se non è nel catalogo vale 1.
    fn duration(&self, id: u16) -> u16 {
        self.combo.catalog().motion(id).map_or(1, |data| data.frames)
    }

    /// Tiene il conto dei frame di caduta e di atterraggio per `shown`.
    fn update_shown(&mut self, landed: bool) {
        let airborne = self.air.airborne();
        let animating = self.combo.current_move().is_some();

        // Il frame della caduta gira in tondo da 1 alla durata compresa, e riparte da 1 ogni
        // volta che la caduta comincia a essere mostrata, cioè quando dopo il tick
        // precedente `shown` rispondeva un'altra cosa (R6).
        if airborne && !animating && !self.air.jumping() {
            let length = self.duration(motion::FALL);
            self.fall_frame = if self.fall_frame == 0 || self.fall_frame >= length {
                1
            } else {
                self.fall_frame + 1
            };
        } else {
            self.fall_frame = 0;
        }

        // L'atterraggio si mostra da quando tocca terra finché la sua animazione non è
        // finita (l'ultimo frame non si vede), o finché non comincia qualcos'altro.
        if airborne || animating {
            self.land_frame = None;
        } else {
            let frame = if landed {
                Some(1)
            } else {
                self.land_frame.map(|frame| frame.saturating_add(1))
            };
            let length = self.duration(motion::LAND);
            self.land_frame = frame.filter(|frame| *frame < length);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combo::IgnoreReason;
    use crate::rules::Body;

    /// Catalogo inventato: un colpo a terra (voce 5, animazione 100, neutra dal frame 14),
    /// un colpo in volo da terra (voce 20, animazione 120, verso un bersaglio in alto), un
    /// colpo aereo (voce 9, animazione 110, 12 frame), più salto, caduta e atterraggio.
    const TEXT: &str = "\
entry id=20 type=1 sub=-1 combo_offset=0 flag=0 motion=120 next_motion=4 jump=17 jump_max=67 jump_min=17 speed_min=0 speed_max=8 near=0 far=400 low=-60 high=-250 inner_min=0.9 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=5 type=3 sub=5 combo_offset=0 flag=0 motion=100 next_motion=0 jump=0 jump_max=0 jump_min=0 speed_min=0 speed_max=0 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
entry id=9 type=3 sub=9 combo_offset=0 flag=1 motion=110 next_motion=4 jump=21 jump_max=71 jump_min=21 speed_min=-8 speed_max=8 near=0 far=0 low=0 high=0 inner_min=1 inner_max=1 blend=0 dist_adjust=0 ability=0 score=0
motion id=3 name=SALTO frames=12
span motion=3 who=body start=4 end=-1 trigger=1 params=
motion id=4 name=CADUTA frames=6
span motion=4 who=body start=0 end=-1 trigger=1 params=
motion id=5 name=ATTERRAGGIO frames=5
motion id=100 name=A frames=20
span motion=100 who=body start=8 end=-1 trigger=11 params=
span motion=100 who=body start=14 end=-1 trigger=2 params=
span motion=100 who=weapon start=3 end=5 trigger=10 params=7
motion id=110 name=ARIA frames=12
span motion=110 who=body start=6 end=-1 trigger=11 params=
motion id=120 name=VERSO_L_ALTO frames=30
motion id=41 name=RESPINTA frames=10
motion id=43 name=RESPINTA_IN_ARIA frames=10
motion id=50 name=DANNO frames=40
attack sub_id=0 id=7 type=0 power=100 element=0 reaction=0 knock1=0 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
";

    fn fighter() -> Fighter {
        Fighter::new(Arc::new(Catalog::parse(TEXT).unwrap()), Loadout::default())
    }

    fn idle<'a>() -> Controls<'a> {
        Controls::default()
    }

    fn attack<'a>() -> Controls<'a> {
        Controls { attack_pressed: true, ..Controls::default() }
    }

    fn jump<'a>() -> Controls<'a> {
        Controls { jump_pressed: true, jump_held: true, ..Controls::default() }
    }

    fn hold<'a>() -> Controls<'a> {
        Controls { jump_held: true, ..Controls::default() }
    }

    fn started(events: &[Event]) -> Option<u8> {
        events.iter().find_map(|event| match event {
            Event::MoveStarted { entry, .. } => Some(*entry),
            _ => None,
        })
    }

    #[test]
    fn appena_creato_e_fermo_a_terra() {
        let fighter = fighter();
        assert_eq!(fighter.height(), 0.0);
        assert!(!fighter.airborne());
        assert_eq!(fighter.slide(), [0.0, 0.0]);
        assert_eq!(fighter.shown(), None);
    }

    #[test]
    fn nel_tick_del_salto_e_gia_salito_del_primo_passo() {
        let mut fighter = fighter();
        let events = fighter.tick(jump());
        assert_eq!(events, [Event::Jumped]);
        assert!(fighter.airborne());
        assert!((fighter.height() - 15.05).abs() < 0.01);
        assert_eq!(fighter.shown(), Some((motion::JUMP, 1)));
    }

    #[test]
    fn si_salta_solo_da_terra() {
        let mut fighter = fighter();
        fighter.tick(jump());
        for _ in 0..5 {
            fighter.tick(hold());
        }
        let events = fighter.tick(jump());
        assert!(!events.contains(&Event::Jumped));
    }

    #[test]
    fn non_si_salta_a_meta_di_un_colpo_ma_da_neutro_si_e_il_colpo_si_interrompe() {
        let mut fighter = fighter();
        assert_eq!(started(&fighter.tick(attack())), Some(5));
        for _ in 0..5 {
            fighter.tick(idle());
        }
        assert!(!fighter.tick(jump()).contains(&Event::Jumped));
        assert!(!fighter.airborne());
        // Fino al frame 14, da cui l'animazione è neutra.
        while fighter.combo.current_move().unwrap().1 < 14 {
            fighter.tick(idle());
        }
        let events = fighter.tick(jump());
        assert_eq!(events, [Event::ComboEnded, Event::Jumped]);
        assert!(fighter.airborne());
        assert!(fighter.combo.current_move().is_none());
    }

    #[test]
    fn occupato_non_salta() {
        let mut fighter = fighter();
        let events = fighter.tick(Controls { busy: true, ..jump() });
        assert!(events.is_empty());
        assert!(!fighter.airborne());
    }

    #[test]
    fn nei_primi_frame_del_salto_attacco_non_vale() {
        let mut fighter = fighter();
        // Frame 1 del salto: l'animazione dichiara "in aria" dal frame 4.
        let events = fighter.tick(Controls { attack_pressed: true, ..jump() });
        assert_eq!(events, [Event::Jumped, Event::PressIgnored(IgnoreReason::Busy)]);
        for _ in 2..=3 {
            let events = fighter.tick(Controls { attack_pressed: true, ..hold() });
            assert_eq!(events, [Event::PressIgnored(IgnoreReason::Busy)]);
        }
        // Frame 4: parte il colpo aereo.
        let events = fighter.tick(Controls { attack_pressed: true, ..hold() });
        assert_eq!(started(&events), Some(9));
    }

    #[test]
    fn salto_pieno_e_atterraggio() {
        let mut fighter = fighter();
        fighter.tick(jump());
        let mut top = 0.0f32;
        let mut ticks = 1;
        loop {
            let events = fighter.tick(hold());
            ticks += 1;
            top = top.max(fighter.height());
            if events.contains(&Event::Landed) {
                assert_eq!(events, [Event::Landed]);
                break;
            }
            assert!(fighter.airborne());
            assert!(ticks < 200);
        }
        assert!((top - 185.0).abs() < 1e-3);
        assert!((60..=62).contains(&ticks));
        assert_eq!(fighter.height(), 0.0);
        assert!(!fighter.airborne());
    }

    #[test]
    fn cosa_mostrare_durante_un_salto() {
        let mut fighter = fighter();
        fighter.tick(jump());
        let mut shown = vec![fighter.shown()];
        for _ in 0..80 {
            fighter.tick(hold());
            shown.push(fighter.shown());
        }
        // Salita e sospensione (tick 1-30): il salto, al più fino al frame 11 (durata 12).
        for (index, entry) in shown.iter().enumerate().take(30) {
            let frame = (index as u16 + 1).min(11);
            assert_eq!(*entry, Some((motion::JUMP, frame)), "tick {}", index + 1);
        }
        // Dal tick 31 la caduta, che gira in tondo da 1 a 6.
        for (index, entry) in shown.iter().enumerate().skip(30).take(20) {
            let frame = ((index - 30) % 6) as u16 + 1;
            assert_eq!(*entry, Some((motion::FALL, frame)), "tick {}", index + 1);
        }
        // Atterraggio: frame 1-4 (durata 5: l'ultimo non si vede), poi niente.
        let landing = shown.iter().position(|entry| matches!(entry, Some((motion::LAND, _)))).unwrap();
        for frame in 1..=4u16 {
            assert_eq!(shown[landing + frame as usize - 1], Some((motion::LAND, frame)));
        }
        assert_eq!(shown[landing + 4], None);
        assert_eq!(*shown.last().unwrap(), None);
    }

    #[test]
    fn dopo_l_atterraggio_si_puo_attaccare_e_saltare_subito() {
        let mut fighter = fighter();
        fighter.tick(jump());
        while !fighter.tick(hold()).contains(&Event::Landed) {}
        assert_eq!(fighter.shown(), Some((motion::LAND, 1)));
        assert_eq!(started(&fighter.tick(attack())), Some(5));
        assert_eq!(fighter.shown(), Some((100, 1)));

        let mut fighter = self::fighter();
        fighter.tick(jump());
        while !fighter.tick(hold()).contains(&Event::Landed) {}
        assert_eq!(fighter.tick(jump()), [Event::Jumped]);
    }

    #[test]
    fn attaccare_nel_tick_dell_atterraggio() {
        let mut fighter = fighter();
        fighter.tick(jump());
        loop {
            let events = fighter.tick(Controls { attack_pressed: fighter.height() < 12.0, ..hold() });
            if events.contains(&Event::Landed) {
                // A terra nello stesso tick: parte il colpo di terra.
                assert_eq!(started(&events), Some(5));
                break;
            }
            assert!(started(&events).is_none() || fighter.airborne());
            if started(&events).is_some() {
                // Era ancora in aria: è partito il colpo aereo; si riprova da capo.
                panic!("la pressione è arrivata prima dell'atterraggio");
            }
        }
        assert_eq!(fighter.shown(), Some((100, 1)));
    }

    #[test]
    fn toccare_terra_interrompe_il_colpo_aereo() {
        let mut fighter = fighter();
        fighter.tick(jump());
        for _ in 0..44 {
            fighter.tick(hold());
        }
        // In caduta, vicino a terra: parte un colpo aereo, che dà un balzo di 21.
        let events = fighter.tick(attack());
        assert_eq!(started(&events), Some(9));
        let mut landed = Vec::new();
        for _ in 0..60 {
            let events = fighter.tick(idle());
            if events.contains(&Event::Landed) {
                landed = events;
                break;
            }
        }
        // Il colpo aereo (12 frame) è finito prima di toccare terra: solo l'atterraggio.
        assert_eq!(landed, [Event::Landed]);

        // Con un colpo ancora in corso al momento di toccare terra: finisce lì.
        let mut fighter = self::fighter();
        fighter.tick(Controls { floor: 3.0, ..idle() });
        fighter.tick(idle());
        assert!(fighter.airborne());
        let events = fighter.tick(attack());
        assert_eq!(started(&events), Some(9));
        // Il terreno sale sotto i piedi mentre il colpo è in corso.
        let mut ended = Vec::new();
        for _ in 0..40 {
            let events = fighter.tick(Controls { floor: 1000.0, ..idle() });
            if events.contains(&Event::Landed) {
                ended = events;
                break;
            }
        }
        assert_eq!(ended, [Event::ComboEnded, Event::Landed]);
        assert_eq!(fighter.height(), 1000.0);
        assert!(fighter.combo.current_move().is_none());
    }

    #[test]
    fn il_colpo_aereo_da_balzo_e_slancio_nella_direzione_in_cui_guarda() {
        let mut fighter = fighter();
        fighter.tick(jump());
        for _ in 0..20 {
            fighter.tick(hold());
        }
        let before = fighter.height();
        let scene = Scene { facing: [1.0, 0.0], ..Scene::default() };
        let events = fighter.tick(Controls { attack_pressed: true, scene, ..hold() });
        assert_eq!(started(&events), Some(9));
        assert!(events.contains(&Event::Lunge { rise: 21.0, run: 0.0, speed: 8.0 }));
        // Il primo passo di slancio si applica nel tick stesso in cui parte il colpo.
        let slide = fighter.slide();
        assert!((slide[0] - 7.6).abs() < 1e-4 && slide[1] == 0.0);
        // Il balzo comincia da questo tick: la prima variazione si vede al tick dopo.
        let at_start = fighter.height();
        assert!(at_start > before);
        fighter.tick(idle());
        assert!(fighter.height() > at_start);
        assert!((fighter.slide()[0] - 7.6).abs() < 1e-4);
        fighter.tick(idle());
        assert!((fighter.slide()[0] - 7.22).abs() < 1e-4);
        // Il balzo porta 21 unità sopra il punto in cui è partito.
        let mut top = fighter.height();
        for _ in 0..8 {
            fighter.tick(idle());
            top = top.max(fighter.height());
        }
        assert!((top - (at_start + 21.0)).abs() < 1e-3);
    }

    #[test]
    fn lo_slancio_finisce_con_il_colpo() {
        let mut fighter = fighter();
        fighter.tick(jump());
        for _ in 0..20 {
            fighter.tick(hold());
        }
        fighter.tick(attack());
        // 12 frame: il colpo occupa 11 tick, questo compreso.
        for _ in 0..10 {
            fighter.tick(idle());
            assert!(fighter.slide()[1] > 0.0);
        }
        let events = fighter.tick(idle());
        assert_eq!(events, [Event::ComboEnded]);
        assert_eq!(fighter.slide(), [0.0, 0.0]);
        assert_eq!(fighter.shown(), Some((motion::FALL, 1)));
    }

    #[test]
    fn lo_slancio_va_verso_il_bersaglio_principale() {
        let mut fighter = fighter();
        fighter.tick(jump());
        for _ in 0..20 {
            fighter.tick(hold());
        }
        let target = Body { position: [300.0, 0.0, -400.0], radius: 0.0, size: 0.0 };
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        fighter.tick(Controls { attack_pressed: true, scene, ..hold() });
        let slide = fighter.slide();
        assert!((slide[0] - 7.6 * 0.6).abs() < 1e-3);
        assert!((slide[1] + 7.6 * 0.8).abs() < 1e-3);
    }

    #[test]
    fn un_colpo_a_terra_non_da_slancio_e_non_stacca_da_terra() {
        let mut fighter = fighter();
        fighter.tick(attack());
        for _ in 0..10 {
            fighter.tick(idle());
            assert_eq!(fighter.slide(), [0.0, 0.0]);
            assert!(!fighter.airborne());
        }
    }

    #[test]
    fn un_colpo_in_volo_da_terra_stacca_il_personaggio() {
        let mut fighter = fighter();
        let target = Body { position: [0.0, 132.7, 100.0], radius: 0.0, size: 0.0 };
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        let events = fighter.tick(Controls { attack_pressed: true, scene, ..idle() });
        assert_eq!(started(&events), Some(20));
        assert!(fighter.airborne());
        assert_eq!(fighter.height(), 0.0);
        let mut top = 0.0f32;
        for _ in 0..28 {
            fighter.tick(idle());
            top = top.max(fighter.height());
        }
        assert!((top - 132.7).abs() < 1e-3);
        // È un balzo, non un salto: si mostra il colpo, e poi la caduta.
        assert_eq!(fighter.shown().unwrap().0, 120);
    }

    #[test]
    fn l_altezza_del_personaggio_nella_scena_la_mette_fighter() {
        let mut fighter = fighter();
        fighter.tick(jump());
        for _ in 0..27 {
            fighter.tick(hold());
        }
        assert!((fighter.height() - 185.0).abs() < 1e-3);
        // Il bersaglio è 100 unità sopra il terreno: dall'alto del salto è più in basso
        // dei piedi, quindi il balzo vale il salto minimo (21) e non 71.
        let target = Body { position: [0.0, 100.0, 100.0], radius: 0.0, size: 0.0 };
        let scene = Scene { fighter: [0.0, -999.0, 0.0], main_target: Some(target), ..Scene::default() };
        let events = fighter.tick(Controls { attack_pressed: true, scene, ..hold() });
        assert!(events.iter().any(|event| matches!(event, Event::Lunge { rise, .. } if *rise == 21.0)));
    }

    #[test]
    fn respinto_si_sceglie_l_animazione_di_terra_o_di_aria() {
        let mut fighter = fighter();
        fighter.tick(attack());
        assert_eq!(fighter.deflected(), [Event::ReactionStarted { motion: 41 }]);
        let mut fighter = self::fighter();
        fighter.tick(jump());
        for _ in 0..20 {
            fighter.tick(hold());
        }
        fighter.tick(attack());
        assert_eq!(fighter.deflected(), [Event::ReactionStarted { motion: 43 }]);
        // La reazione non è un colpo in volo: lo slancio finisce al tick dopo.
        fighter.tick(idle());
        assert_eq!(fighter.slide(), [0.0, 0.0]);
    }

    #[test]
    fn un_colpo_subito_puo_sbalzare_in_aria() {
        let mut fighter = fighter();
        let events = fighter.knocked(50, 0.0);
        assert_eq!(events, [Event::ReactionStarted { motion: 50 }]);
        assert!(!fighter.airborne() && !fighter.combo.launched());
        let mut fighter = self::fighter();
        fighter.knocked(50, 40.0);
        assert!(fighter.airborne() && fighter.combo.launched());
        assert_eq!(fighter.shown(), Some((50, 1)));
        let mut top = 0.0f32;
        let mut landed = false;
        for _ in 0..80 {
            let events = fighter.tick(idle());
            top = top.max(fighter.height());
            if events.contains(&Event::Landed) {
                // Toccare terra interrompe l'animazione di danno e finisce lo sbalzo.
                assert_eq!(events, [Event::ComboEnded, Event::Landed]);
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert!((top - 40.0).abs() < 1e-3);
        assert!(!fighter.combo.launched());
    }

    #[test]
    fn il_tasto_del_salto_da_sbalzato_e_il_ritorno_in_equilibrio() {
        let text = format!("{TEXT}motion id=40 name=EQUILIBRIO frames=10\nspan motion=40 who=body start=6 end=-1 trigger=1 params=\n");
        let mut loadout = Loadout::default();
        loadout.equip(rules::AERIAL_RECOVERY);
        let mut fighter = Fighter::new(Arc::new(Catalog::parse(&text).unwrap()), loadout);
        fighter.knocked(50, 60.0);
        fighter.tick(idle());
        let events = fighter.tick(jump());
        assert_eq!(events, [Event::ReactionStarted { motion: motion::AERIAL_RECOVERY }]);
        assert!(!fighter.combo.launched());
        assert_eq!(fighter.shown(), Some((motion::AERIAL_RECOVERY, 1)));
    }

    #[test]
    fn uscire_da_un_bordo_fa_cadere() {
        let mut fighter = fighter();
        fighter.tick(Controls { floor: 30.0, ..idle() });
        assert_eq!(fighter.height(), 30.0);
        fighter.tick(idle());
        assert!(fighter.airborne());
        assert_eq!(fighter.height(), 30.0);
        assert_eq!(fighter.shown(), Some((motion::FALL, 1)));
        fighter.tick(idle());
        assert!(fighter.height() < 30.0);
        assert_eq!(fighter.shown(), Some((motion::FALL, 2)));
    }

    #[test]
    fn senza_le_animazioni_nel_catalogo_la_durata_vale_uno() {
        let entry = TEXT.lines().nth(1).unwrap();
        let mut fighter = Fighter::new(Arc::new(Catalog::parse(entry).unwrap()), Loadout::default());
        // Senza l'animazione del salto il personaggio non è mai occupato per quel motivo.
        let events = fighter.tick(Controls { attack_pressed: true, ..jump() });
        assert_eq!(events[0], Event::Jumped);
        assert!(!events.contains(&Event::PressIgnored(IgnoreReason::Busy)));
        fighter.combo.interrupt();
        fighter.tick(hold());
        assert_eq!(fighter.shown(), Some((motion::JUMP, 1)));
        for _ in 0..10 {
            fighter.tick(hold());
        }
        assert_eq!(fighter.shown(), Some((motion::JUMP, 1)));
        for _ in 0..25 {
            fighter.tick(hold());
        }
        assert_eq!(fighter.shown(), Some((motion::FALL, 1)));
        while !fighter.tick(hold()).contains(&Event::Landed) {
            assert_eq!(fighter.shown(), Some((motion::FALL, 1)));
        }
        // Un atterraggio di durata 1 non si vede mai.
        assert_eq!(fighter.shown(), None);
    }
}
