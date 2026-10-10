//! Le regole provate sul catalogo vero di Sora (`data\catalog\sora_base.catalog`).
//!
//! Il catalogo vero non sta in Git: se manca, questi test si saltano da soli.

mod common;

use std::sync::Arc;

use kh2_combat_core::catalog::{kind, motion, trigger, Catalog, Entry};
use kh2_combat_core::combo::{Combo, Event, IgnoreReason, TickInput};
use kh2_combat_core::fighter::{Controls, Fighter};
use kh2_combat_core::rules::{self, Body, FighterView, Loadout, Scene};

/// Le abilità con questi nomi, equipaggiate.
fn loadout(catalog: &Catalog, abilities: &[&str]) -> Loadout {
    let mut loadout = Loadout::default();
    for name in abilities {
        let number = catalog
            .ability_number(name)
            .unwrap_or_else(|| panic!("il catalogo non conosce l'abilità {name}"));
        loadout.equip(number);
    }
    loadout
}

/// Un nemico: di quanto sta di lato (x), più in alto, e in avanti (z) rispetto a Sora, che
/// sta nell'origine e guarda verso +z.
fn enemy(side: f32, up: f32, ahead: f32) -> Body {
    Body {
        position: [side, up, ahead],
        radius: 0.0,
        size: 0.0,
    }
}

fn press<'a>() -> TickInput<'a> {
    TickInput {
        attack_pressed: true,
        ..TickInput::default()
    }
}

fn hit<'a>() -> TickInput<'a> {
    TickInput {
        hit_landed: true,
        ..TickInput::default()
    }
}

/// Fa passare tick senza tasti finché l'animazione in corso arriva a quel frame.
fn wait_frame(combo: &mut Combo, frame: u16) {
    while combo.current_move().expect("serve un'animazione in corso").1 < frame {
        combo.tick(TickInput::default());
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

fn rise_of(events: &[Event]) -> Option<f32> {
    events.iter().find_map(|event| match event {
        Event::Lunge { rise, .. } => Some(*rise),
        _ => None,
    })
}

#[test]
fn il_catalogo_vero_si_legge_per_intero() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    assert_eq!(catalog.entries.len(), 37);
    assert_eq!(catalog.motions.len(), 82);
    assert_eq!(catalog.attacks.values().map(Vec::len).sum::<usize>(), 49);
    assert_eq!(catalog.abilities.len(), 20);
    assert_eq!(catalog.ability_number("Slide_Dash"), Some(92));
    // Ogni voce con un'animazione la trova nel catalogo, e ogni zona di un colpo rimanda a
    // un colpo a segno che esiste.
    for entry in &catalog.entries {
        let data = catalog
            .motion(entry.motion)
            .unwrap_or_else(|| panic!("manca l'animazione {} della voce {}", entry.motion, entry.id));
        for frame in 1..data.frames {
            if let Some(attack) = data.hitbox(frame) {
                assert!(catalog.attack(attack).is_some(), "manca il colpo a segno {attack}");
            }
        }
    }
}

#[test]
fn le_voci_richiamate_da_un_comando_sono_quelle_della_specifica() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut by_command: Vec<u8> = catalog
        .entries
        .iter()
        .filter(|entry| entry.kind == kind::BY_COMMAND)
        .map(|entry| entry.id)
        .collect();
    by_command.sort();
    assert_eq!(by_command, [1, 11, 12, 37, 38, 39, 40]);
    assert_eq!(rules::COUNTERGUARD, 39);
    assert_eq!(rules::RETALIATING_SLASH, 40);
    // Finishing Leap ha anticipo 1.
    let leap = &catalog.entries[catalog.index_of_id(37).unwrap()];
    assert_eq!(leap.step_advance, 1);
    assert!(leap.is_finisher());
}

#[test]
fn tempi_dei_cinque_colpi_a_terra_di_base() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    // (animazione, frame, "si prosegue dal frame")
    for (number, frames, chain) in [(151, 48, 23), (152, 56, 30), (153, 72, 44)] {
        let data = catalog.motion(number).unwrap();
        assert_eq!(data.frames, frames, "animazione {number}");
        assert_eq!(data.first_frame_of(trigger::COMBO_ALLOWED), Some(chain));
        assert!(!data.active(trigger::COMBO_ALLOWED, chain - 1));
        assert!(data.active(trigger::COMBO_ALLOWED, chain));
        assert!(data.active(trigger::COMBO_ALLOWED, frames - 1));
        // I colpi a terra di base non si dichiarano mai neutri.
        assert!((1..frames).all(|frame| !data.neutral(frame)));
    }
    // I due finali non hanno "combo permessa": hanno solo "finale permesso".
    for number in [154, 155] {
        let data = catalog.motion(number).unwrap();
        assert_eq!(data.frames, 80);
        assert_eq!(data.first_frame_of(trigger::COMBO_ALLOWED), None);
        assert_eq!(data.first_frame_of(trigger::FINISHER_ALLOWED), Some(35));
        assert!((1..80).all(|frame| !data.neutral(frame)));
    }
    // L'affondo ha due zone una dopo l'altra.
    let lunge = catalog.motion(153).unwrap();
    assert_eq!(lunge.hitbox(20), Some(128));
    assert_eq!(lunge.hitbox(35), None);
    assert_eq!(lunge.hitbox(38), Some(147));
}

#[test]
fn le_reazioni_hanno_gli_eventi_descritti_nella_specifica() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    // Il salto dichiara "in aria" dal frame 8.
    let jump = catalog.motion(motion::JUMP).unwrap();
    assert_eq!(jump.first_frame_of(trigger::AIRBORNE), Some(8));
    // La caduta è neutra.
    assert!(catalog.motion(motion::FALL).unwrap().neutral(1));
    // Arma respinta a terra: "combo permessa" dal frame 16.
    let deflect = catalog.motion(motion::DEFLECT).unwrap();
    assert_eq!(deflect.first_frame_of(trigger::COMBO_ALLOWED), Some(16));
    // Ritorno in equilibrio: non colpibile nei primi 16 frame, libero in aria dal 24.
    let recovery = catalog.motion(motion::AERIAL_RECOVERY).unwrap();
    assert!(recovery.active(trigger::UNHITTABLE, 1) && recovery.active(trigger::UNHITTABLE, 16));
    assert!(!recovery.active(trigger::UNHITTABLE, 17));
    assert!(!recovery.neutral(23) && recovery.neutral(24));
    // Parata riuscita: l'arma para nei frame 1-40, poi l'animazione torna neutra.
    let success = catalog.motion(motion::GUARD_SUCCESS).unwrap();
    for frame in [1, 40] {
        let zone = success.hitbox(frame).unwrap();
        assert!(catalog.attack(zone).unwrap().guards());
    }
    assert_eq!(success.hitbox(41), None);
    assert!(!success.neutral(39) && success.neutral(40));
    assert_eq!(success.first_frame_of(trigger::COMBO_ALLOWED), None);
    assert!(catalog.motion(motion::DEFLECT_AIR).is_some());
    assert!(catalog.motion(motion::LAND).is_some());
}

/// La situazione in cui una voce del repertorio deve uscire.
struct Case {
    /// La posizione della voce nel repertorio, con il suo numero e la sua animazione.
    index: usize,
    id: u8,
    motion: u16,
    /// Le abilità equipaggiate (solo quelle che servono).
    abilities: &'static [&'static str],
    airborne: bool,
    /// I passi, già contando il colpo che sta per partire.
    steps: u8,
    /// Il bersaglio principale: (di lato, più in alto, in avanti).
    target: Option<(f32, f32, f32)>,
    /// Un altro nemico nei dintorni.
    other: Option<(f32, f32, f32)>,
}

const fn case(index: usize, id: u8, motion: u16) -> Case {
    Case {
        index,
        id,
        motion,
        abilities: &[],
        airborne: false,
        steps: 1,
        target: None,
        other: None,
    }
}

/// Ogni voce che entra nella scelta (tipi 1, 2 e 3), nella sua situazione.
const CASES: [Case; 30] = [
    // A terra, verso un bersaglio in alto (colpi in volo).
    Case { abilities: &["Aerial_Sweep"], target: Some((0.0, 150.0, 100.0)), ..case(0, 33, 191) },
    Case { abilities: &["Aerial_Dive"], target: Some((0.0, 150.0, 400.0)), ..case(1, 48, 196) },
    Case { target: Some((0.0, 150.0, 100.0)), ..case(2, 0, 182) },
    // I finali a terra, al terzo passo.
    Case { abilities: &["Explosion"], steps: 3, other: Some((100.0, 0.0, 0.0)), ..case(4, 26, 166) },
    Case { steps: 3, other: Some((100.0, 50.0, 0.0)), ..case(5, 2, 154) },
    Case { abilities: &["Guard_Break"], steps: 3, ..case(6, 27, 165) },
    Case { abilities: &["Explosion"], steps: 3, ..case(7, 42, 166) },
    Case { steps: 3, ..case(8, 3, 155) },
    // I colpi normali a terra.
    Case { abilities: &["Vicinity_Break"], other: Some((-100.0, 50.0, -100.0)), ..case(9, 51, 170) },
    Case { abilities: &["Flash_Step"], target: Some((0.0, 50.0, 300.0)), ..case(10, 46, 169) },
    Case { abilities: &["Dodge_Slash"], target: Some((0.0, 50.0, 200.0)), ..case(11, 44, 163) },
    Case { abilities: &["Slide_Dash"], target: Some((0.0, 50.0, 500.0)), ..case(12, 28, 164) },
    Case { target: Some((0.0, 50.0, 200.0)), ..case(13, 6, 153) },
    Case { abilities: &["Dodge_Slash"], other: Some((70.0, 50.0, 70.0)), ..case(14, 34, 163) },
    Case { other: Some((70.0, 50.0, 70.0)), ..case(15, 4, 152) },
    Case { abilities: &["Slapshot"], target: Some((0.0, 50.0, 60.0)), ..case(16, 35, 162) },
    Case { abilities: &["Flash_Step"], target: Some((0.0, 50.0, 150.0)), ..case(17, 47, 169) },
    Case { abilities: &["Dodge_Slash"], target: Some((0.0, 50.0, 150.0)), ..case(18, 45, 163) },
    Case { abilities: &["Flash_Step"], other: Some((0.0, 50.0, 300.0)), ..case(19, 59, 169) },
    Case { abilities: &["Slide_Dash"], other: Some((0.0, 50.0, 500.0)), ..case(20, 43, 164) },
    case(21, 5, 151),
    // I finali in aria, al terzo passo.
    Case { abilities: &["Magnet_Burst"], airborne: true, steps: 3, other: Some((300.0, 0.0, 0.0)), ..case(22, 49, 197) },
    Case { airborne: true, steps: 3, other: Some((100.0, 0.0, 0.0)), ..case(23, 7, 184) },
    Case { abilities: &["Aerial_Finish"], airborne: true, steps: 3, ..case(24, 29, 194) },
    Case { abilities: &["Magnet_Burst"], airborne: true, steps: 3, target: Some((0.0, 0.0, 400.0)), ..case(25, 50, 197) },
    Case { airborne: true, steps: 3, ..case(26, 8, 183) },
    // I colpi normali in aria.
    Case { abilities: &["Aerial_Spiral"], airborne: true, target: Some((0.0, 0.0, 200.0)), ..case(27, 36, 192) },
    Case { abilities: &["Aerial_Spiral"], airborne: true, other: Some((0.0, 0.0, 400.0)), ..case(28, 36, 192) },
    Case { airborne: true, target: Some((0.0, 100.0, 100.0)), ..case(29, 10, 182) },
    Case { airborne: true, ..case(30, 9, 181) },
];

#[test]
fn ogni_voce_del_repertorio_esce_nella_sua_situazione() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    for case in &CASES {
        let label = format!("voce {} (posizione {})", case.id, case.index);
        let expected = &catalog.entries[case.index];
        assert_eq!((expected.id, expected.motion), (case.id, case.motion), "{label}");

        let target = case.target.map(|(side, up, ahead)| enemy(side, up, ahead));
        let nearby: Vec<Body> = target
            .into_iter()
            .chain(case.other.map(|(side, up, ahead)| enemy(side, up, ahead)))
            .collect();
        let scene = Scene {
            main_target: target,
            nearby: &nearby,
            ..Scene::default()
        };
        let view = FighterView {
            airborne: case.airborne,
            steps: case.steps,
            finishers: 1,
            ..FighterView::default()
        };
        let chosen = rules::choose(&catalog, &loadout(&catalog, case.abilities), &view, &scene)
            .unwrap_or_else(|| panic!("{label}: non è uscito niente"));
        assert!(
            std::ptr::eq(chosen, expected),
            "{label}: è uscita la voce {} (animazione {})",
            chosen.id,
            chosen.motion
        );
        // Senza le sue abilità una voce che ne richiede una non può uscire.
        if expected.ability != 0 {
            let without = rules::choose(&catalog, &Loadout::default(), &view, &scene).unwrap();
            assert!(!std::ptr::eq(without, expected), "{label}: è uscita senza abilità");
        }
    }
}

#[test]
fn le_voci_di_tipo_zero_partono_solo_con_il_loro_comando() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    // (numero, animazione, abilità, in aria, passi)
    let commands: [(u8, u16, &[&str], bool, u8); 7] = [
        (1, 181, &[], false, 1),
        (11, 173, &["Guard"], false, 1),
        (12, 161, &["Upper_Slash"], false, 1),
        (37, 167, &["Finishing_Leap"], false, 2),
        (38, 193, &["Horizontal_Slash"], true, 1),
        (39, 171, &["Counterguard"], false, 1),
        (40, 172, &["Retaliating_Slash"], true, 1),
    ];
    for (id, number, abilities, airborne, steps) in commands {
        let view = FighterView {
            airborne,
            steps,
            finishers: 1,
            ..FighterView::default()
        };
        let equipped = loadout(&catalog, abilities);
        let called = rules::by_command(&catalog, &equipped, &view, id)
            .unwrap_or_else(|| panic!("il comando {id} non è partito"));
        assert_eq!((called.id, called.motion, called.kind), (id, number, kind::BY_COMMAND));
        // Senza l'abilità, o dalla parte sbagliata (terra/aria), non parte.
        if !abilities.is_empty() {
            assert!(rules::by_command(&catalog, &Loadout::default(), &view, id).is_none());
        }
        let wrong_side = FighterView { airborne: !airborne, ..view };
        assert!(rules::by_command(&catalog, &equipped, &wrong_side, id).is_none());
        // E la scelta normale non le fa mai uscire, nemmeno con tutte le abilità.
        let everything: Vec<&str> = catalog.abilities.values().map(String::as_str).collect();
        let chosen = rules::choose(&catalog, &loadout(&catalog, &everything), &view, &Scene::default());
        assert_ne!(chosen.map(|entry| entry.kind), Some(kind::BY_COMMAND));
    }
    // Finishing Leap è disponibile un passo prima degli altri finali, non due.
    let leap = loadout(&catalog, &["Finishing_Leap"]);
    let first_step = FighterView { steps: 1, finishers: 1, ..FighterView::default() };
    assert!(rules::by_command(&catalog, &leap, &first_step, 37).is_none());
}

#[test]
fn il_bordo_fa_partire_il_colpo_in_salto() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let view = FighterView { steps: 1, finishers: 1, ..FighterView::default() };
    let edge = Scene { ledge_ahead: true, ..Scene::default() };
    let chosen = rules::choose(&catalog, &Loadout::default(), &view, &edge).unwrap();
    assert_eq!((chosen.id, chosen.motion), (1, 181));
    assert!(chosen.lunges());
    // In aria il bordo non conta: esce il primo colpo aereo.
    let air_view = FighterView { airborne: true, ..view };
    assert_eq!(rules::choose(&catalog, &Loadout::default(), &air_view, &edge).unwrap().id, 9);
}

/// Vero se la voce con quel numero è candidata con un nemico a quell'angolo (in gradi)
/// dalla direzione del bersaglio principale, che sta dritto davanti e lontano.
fn candidate_at_angle(catalog: &Catalog, loadout: &Loadout, id: u8, degrees: f32) -> bool {
    let radians = degrees.to_radians();
    let nearby = [enemy(100.0 * radians.sin(), 50.0, 100.0 * radians.cos())];
    let scene = Scene {
        main_target: Some(enemy(0.0, 50.0, 2000.0)),
        nearby: &nearby,
        ..Scene::default()
    };
    let view = FighterView { steps: 3, finishers: 1, ..FighterView::default() };
    rules::candidates(catalog, loadout, &view, &scene)[catalog.index_of_id(id).unwrap()]
}

#[test]
fn gli_angoli_dei_colpi_di_lato() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let none = Loadout::default();
    // Il colpo B (voce 4) e il finale 1 (voce 2): un nemico di lato, tra circa 22 e 135 gradi.
    for id in [4, 2] {
        for (degrees, expected) in [(0.0, false), (20.0, false), (25.0, true), (90.0, true), (130.0, true), (140.0, false), (180.0, false)] {
            assert_eq!(candidate_at_angle(&catalog, &none, id, degrees), expected, "voce {id} a {degrees} gradi");
            assert_eq!(candidate_at_angle(&catalog, &none, id, -degrees), expected, "voce {id} a -{degrees} gradi");
        }
    }
    // Vicinity Break (voce 51): un nemico di fianco o dietro.
    let vicinity = loadout(&catalog, &["Vicinity_Break"]);
    for (degrees, expected) in [(0.0, false), (90.0, false), (95.0, false), (105.0, true), (170.0, true)] {
        assert_eq!(candidate_at_angle(&catalog, &vicinity, 51, degrees), expected, "a {degrees} gradi");
    }
    // Le voci di tipo 1 chiedono "proprio il bersaglio": coseno sopra 0,99996.
    for entry in catalog.entries.iter().filter(|entry| entry.kind == kind::MAIN_TARGET) {
        assert!(entry.inner_min > 0.9999 && entry.inner_max == 1.0, "voce {}", entry.id);
    }
}

#[test]
fn senza_bersagli_la_combo_a_terra_e_a_c_finale() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut combo = Combo::new(Arc::new(catalog), Loadout::default());
    // Primo colpo: A (il ripiego).
    assert_eq!(started(&combo.tick(press())), Some((5, 151, 1)));
    combo.tick(hit());
    // Frame 22: troppo presto. Frame 23: accettata, e non può essere di nuovo A: parte C.
    wait_frame(&mut combo, 21);
    assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
    assert_eq!(combo.current_move(), Some((151, 22)));
    assert_eq!(started(&combo.tick(press())), Some((6, 153, 2)));
    // Il terzo è un finale.
    combo.tick(hit());
    wait_frame(&mut combo, 43);
    let events = combo.tick(press());
    assert_eq!(started(&events), Some((3, 155, 3)));
    assert!(combo.current_entry().unwrap().is_finisher());
    // Dopo il finale la combo è finita: 80 frame, 79 tick, e ogni pressione è persa.
    combo.tick(hit());
    for _ in 2..79 {
        let events = combo.tick(press());
        assert_eq!(ignored(&events), Some(IgnoreReason::ComboOver));
    }
    assert_eq!(combo.current_move(), Some((155, 79)));
    let events = combo.tick(press());
    assert_eq!(events[0], Event::ComboEnded);
    assert_eq!(started(&events), Some((5, 151, 1)));
}

#[test]
fn un_colpo_a_vuoto_bisogna_aspettare_che_finisca() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut combo = Combo::new(Arc::new(catalog), Loadout::default());
    combo.tick(press());
    // A dura 48 frame: occupa 47 tick. Senza toccare, nessuna pressione vale.
    for frame in 2..=47u16 {
        let events = combo.tick(press());
        let expected = if frame < 23 { IgnoreReason::TooEarly } else { IgnoreReason::NotTouched };
        assert_eq!(ignored(&events), Some(expected), "frame {frame}");
    }
    // Al tick dopo il personaggio è libero: la pressione apre subito una combo nuova.
    let events = combo.tick(press());
    assert_eq!(events[0], Event::ComboEnded);
    assert_eq!(started(&events), Some((5, 151, 1)));
}

#[test]
fn potenza_della_zona_del_primo_colpo() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut combo = Combo::new(Arc::new(catalog), Loadout::default());
    combo.tick(press());
    wait_frame(&mut combo, 14);
    assert_eq!(combo.hit_power(), None);
    let events = combo.tick(TickInput::default());
    assert_eq!(events, [Event::HitWindowOpened { attack: 126, power: 100 }]);
    assert_eq!((combo.hit_attack(), combo.hit_power()), (Some(126), Some(100)));
    wait_frame(&mut combo, 23);
    assert_eq!(combo.hit_power(), Some(100));
    // Al frame 24 la zona si chiude; la finestra della combo si era aperta al 23.
    assert_eq!(combo.tick(TickInput::default()), [Event::HitWindowClosed]);
}

#[test]
fn con_finishing_plus_il_secondo_finale_e_l_alternativa_del_primo() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut abilities = Loadout::default();
    abilities.finishing_plus = 1;
    abilities.combo_master = true;
    let mut combo = Combo::new(Arc::new(catalog), abilities);
    combo.tick(press());
    wait_frame(&mut combo, 22);
    assert_eq!(started(&combo.tick(press())), Some((6, 153, 2)));
    wait_frame(&mut combo, 43);
    assert_eq!(started(&combo.tick(press())), Some((3, 155, 3)));
    // Finestra "finale permesso" dal frame 35 al 64.
    wait_frame(&mut combo, 33);
    assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
    let events = combo.tick(press());
    // Il finale 2 non può ripetersi: parte la sua alternativa, il finale 1.
    assert_eq!(started(&events), Some((2, 154, 4)));
    assert_eq!((combo.step(), combo.finishers()), (4, 2));
    // I finali sono al massimo (2): da qui la combo è finita.
    wait_frame(&mut combo, 40);
    assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::ComboOver));
}

#[test]
fn combo_plus_allunga_la_combo_a_terra() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut abilities = Loadout::default();
    abilities.combo_plus = 1;
    abilities.combo_master = true;
    let mut combo = Combo::new(Arc::new(catalog), abilities);
    let mut moves = vec![started(&combo.tick(press())).unwrap()];
    for _ in 0..3 {
        // Al frame 44 sono aperte le finestre sia di A (dal 23) sia di C (dal 44).
        wait_frame(&mut combo, 44);
        moves.push(started(&combo.tick(press())).unwrap());
    }
    // Quattro passi: A, C, di nuovo A, e il finale al quarto.
    assert_eq!(moves, [(5, 151, 1), (6, 153, 2), (5, 151, 3), (3, 155, 4)]);
}

#[test]
fn la_parata_vera_e_il_contrattacco() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    let guard = TickInput { command: Some(11), ..TickInput::default() };

    // Senza l'abilità Guard il comando non ha effetto.
    let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
    assert_eq!(combo.tick(guard), [Event::PressIgnored(IgnoreReason::Unavailable)]);

    // Con l'abilità: animazione 173, 60 frame, zona di parata dal frame 6 al 37.
    let abilities = loadout(&catalog, &["Guard", "Counterguard"]);
    let mut combo = Combo::new(Arc::clone(&catalog), abilities.clone());
    assert_eq!(started(&combo.tick(guard)), Some((11, 173, 1)));
    let mut guarding = Vec::new();
    for frame in 1..=59u16 {
        assert_eq!(combo.current_move(), Some((173, frame)));
        if combo.guarding() {
            guarding.push(frame);
            assert_eq!(combo.hit_power(), None);
        }
        combo.tick(TickInput::default());
    }
    assert_eq!(guarding, (6..=37).collect::<Vec<u16>>());
    assert!(combo.current_move().is_none());

    // Un colpo nemico fuori dalla zona di parata non viene fermato.
    let mut combo = Combo::new(Arc::clone(&catalog), abilities.clone());
    combo.tick(guard);
    wait_frame(&mut combo, 5);
    assert!(combo.guarded().is_empty());
    // Dentro la zona: parata riuscita, i passi scendono di 1.
    wait_frame(&mut combo, 6);
    let events = combo.guarded();
    assert_eq!(events[1], Event::ReactionStarted { motion: motion::GUARD_SUCCESS });
    assert_eq!(combo.current_move(), Some((motion::GUARD_SUCCESS, 1)));
    assert_eq!(combo.step(), 0);
    assert!(combo.guarding());
    // Attacco durante la parata riuscita: Counterguard, subito.
    wait_frame(&mut combo, 10);
    assert_eq!(started(&combo.tick(press())), Some((39, 171, 1)));
    assert!(combo.unhittable());

    // Senza Counterguard: ignorata finché l'animazione non diventa neutra (frame 40).
    let mut combo = Combo::new(Arc::clone(&catalog), loadout(&catalog, &["Guard"]));
    combo.tick(guard);
    wait_frame(&mut combo, 6);
    combo.guarded();
    wait_frame(&mut combo, 38);
    assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
    assert_eq!(started(&combo.tick(press())), Some((5, 151, 1)));
}

#[test]
fn l_arma_respinta_non_conta_come_passo_e_la_combo_riprende() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut combo = Combo::new(Arc::new(catalog), Loadout::default());
    combo.tick(press());
    wait_frame(&mut combo, 16);
    let events = combo.deflected(false);
    assert_eq!(events, [Event::HitWindowClosed, Event::ReactionStarted { motion: motion::DEFLECT }]);
    assert_eq!(combo.step(), 0);
    // "Combo permessa" dal frame 16 della reazione.
    wait_frame(&mut combo, 14);
    assert_eq!(ignored(&combo.tick(press())), Some(IgnoreReason::TooEarly));
    assert_eq!(started(&combo.tick(press())), Some((5, 151, 1)));
}

#[test]
fn sbalzato_ritorno_in_equilibrio_e_colpo_di_ritorno() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    // Un'animazione di danno qualsiasi tra quelle del catalogo.
    let damage = 56;
    assert!(catalog.motion(damage).is_some());

    // Aerial Recovery: non colpibile nei primi 16 frame, libero in aria dal 24.
    let mut abilities = Loadout::default();
    abilities.equip(rules::AERIAL_RECOVERY);
    let mut combo = Combo::new(Arc::clone(&catalog), abilities);
    combo.knocked(damage, true);
    assert!(combo.launched());
    let recover = TickInput { recover_pressed: true, airborne: true, ..TickInput::default() };
    // Negli stessi 16 frame l'animazione ha anche una zona di "parata totale" (il 124).
    assert_eq!(
        combo.tick(recover),
        [
            Event::ReactionStarted { motion: motion::AERIAL_RECOVERY },
            Event::HitWindowOpened { attack: 124, power: 0 },
        ]
    );
    assert!(!combo.launched());
    assert!(combo.guarding() && combo.hit_power().is_none());
    // È una parata totale: un colpo nemico viene fermato, ma per il personaggio non cambia
    // niente e il ritorno in equilibrio prosegue.
    wait_frame(&mut combo, 8);
    assert!(combo.guarded().is_empty());
    assert_eq!(combo.current_move(), Some((motion::AERIAL_RECOVERY, 8)));
    assert_eq!((combo.step(), combo.finishers()), (0, 1));
    wait_frame(&mut combo, 16);
    assert!(combo.unhittable() && combo.guarding() && !combo.free());
    wait_frame(&mut combo, 17);
    assert!(!combo.unhittable() && !combo.guarding());
    wait_frame(&mut combo, 23);
    assert!(!combo.free());
    wait_frame(&mut combo, 24);
    assert!(combo.free());

    // Retaliating Slash: Attacco mentre è sbalzato in aria.
    let mut combo = Combo::new(Arc::clone(&catalog), loadout(&catalog, &["Retaliating_Slash"]));
    combo.knocked(damage, true);
    let events = combo.tick(TickInput { airborne: true, ..press() });
    assert_eq!(started(&events), Some((40, 172, 1)));
    assert_eq!(rise_of(&events), Some(21.0));
    assert!(!combo.launched());
}

#[test]
fn la_combo_aerea_vera() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let mut fighter = Fighter::new(Arc::new(catalog), Loadout::default());
    let jump = Controls { jump_pressed: true, jump_held: true, attack_pressed: true, ..Controls::default() };
    let hold = Controls { jump_held: true, attack_pressed: true, ..Controls::default() };
    // Nei primi 7 frame del salto Attacco non vale: il colpo aereo più precoce parte 7
    // tick dopo l'inizio del salto.
    let events = fighter.tick(jump);
    assert_eq!(events, [Event::Jumped, Event::PressIgnored(IgnoreReason::Busy)]);
    for _ in 2..=7 {
        assert_eq!(fighter.tick(hold), [Event::PressIgnored(IgnoreReason::Busy)]);
    }
    let events = fighter.tick(hold);
    assert_eq!(started(&events), Some((9, 181, 1)));
    // Senza bersaglio: sale di 21 il primo colpo aereo, di 17 il secondo, e lo slancio
    // parte alla velocità massima della voce (8).
    assert_eq!(rise_of(&events), Some(21.0));
    assert!(events.contains(&Event::Lunge { rise: 21.0, run: 0.0, speed: 8.0 }));
    assert!((fighter.slide()[1] - 7.6).abs() < 1e-4);

    let touched = Controls { hit_landed: true, ..Controls::default() };
    let attack = Controls { attack_pressed: true, ..Controls::default() };
    fighter.tick(touched);
    while fighter.combo.current_move().unwrap().1 < 21 {
        fighter.tick(Controls::default());
    }
    // Il secondo non può essere di nuovo lo stesso: parte la sua alternativa (voce 10).
    let events = fighter.tick(attack);
    assert_eq!(started(&events), Some((10, 182, 2)));
    assert_eq!(rise_of(&events), Some(17.0));
    fighter.tick(touched);
    while fighter.combo.current_move().unwrap().1 < 19 {
        fighter.tick(Controls::default());
    }
    // Il terzo è il finale aereo.
    let events = fighter.tick(attack);
    assert_eq!(started(&events), Some((8, 183, 3)));
    assert!(fighter.airborne());
    // Una combo aerea tiene il personaggio quasi alla stessa quota.
    assert!(fighter.height() > 90.0, "altezza {}", fighter.height());
}

#[test]
fn le_salite_misurate_dei_colpi_in_volo() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    let lunge = |airborne: bool, number: u16, target: Body| {
        let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
        let scene = Scene { main_target: Some(target), ..Scene::default() };
        combo.override_next_move(Some(number));
        let events = combo.tick(TickInput { airborne, scene, ..press() });
        rise_of(&events).expect("serve un colpo in volo")
    };
    // In aria, con il bersaglio molto più in alto: il massimo della voce 10 è 67.
    assert_eq!(lunge(true, 182, enemy(0.0, 400.0, 100.0)), 67.0);
    assert_eq!(lunge(true, 182, enemy(0.0, -50.0, 100.0)), 17.0);
    // Da terra (voce 0): tra 110 e 250 più la taglia del bersaglio.
    let sized = |up: f32| Body { size: 3.0, ..enemy(0.0, up, 100.0) };
    assert_eq!(lunge(false, 182, sized(132.7)), 132.7);
    assert_eq!(lunge(false, 182, sized(148.8)), 148.8);
    assert_eq!(lunge(false, 182, sized(367.7)), 253.0);
    assert_eq!(lunge(false, 182, sized(20.0)), 113.0);
}

/// Fa partire, come primo colpo, la prima voce con quell'animazione, e per ogni frame
/// dell'animazione dice (zona indicata, potenza, sta parando) e gli eventi usciti.
fn zones_of(
    catalog: &Arc<Catalog>,
    number: u16,
) -> (Vec<(Option<u16>, Option<u16>, bool)>, Vec<(u16, Event)>) {
    let mut combo = Combo::new(Arc::clone(catalog), Loadout::default());
    combo.override_next_move(Some(number));
    let mut log: Vec<(u16, Event)> = Vec::new();
    for event in combo.tick(press()) {
        log.push((1, event));
    }
    let mut seen = Vec::new();
    while let Some((_, frame)) = combo.current_move() {
        seen.push((combo.hit_attack(), combo.hit_power(), combo.guarding()));
        for event in combo.tick(TickInput::default()) {
            log.push((frame + 1, event));
        }
    }
    let zone_events = log
        .into_iter()
        .filter(|(_, event)| {
            matches!(event, Event::HitWindowOpened { .. } | Event::HitWindowClosed)
        })
        .collect();
    (seen, zone_events)
}

#[test]
fn guard_break_para_mentre_colpisce() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let (seen, events) = zones_of(&Arc::new(catalog), 165);
    // Apre al frame 5 con la parata (potenza 0), al 28 chiude e riapre con il colpo da 250,
    // e intanto continua a parare fino al 42.
    assert_eq!(
        events,
        [
            (5, Event::HitWindowOpened { attack: 162, power: 0 }),
            (28, Event::HitWindowClosed),
            (28, Event::HitWindowOpened { attack: 876, power: 250 }),
            (43, Event::HitWindowClosed),
        ]
    );
    let at = |frame: usize| seen[frame - 1];
    assert_eq!(at(4), (None, None, false));
    assert_eq!(at(5), (Some(162), None, true));
    assert_eq!(at(27), (Some(162), None, true));
    assert_eq!(at(28), (Some(876), Some(250), true));
    assert_eq!(at(42), (Some(876), Some(250), true));
    assert_eq!(at(43), (None, None, false));
}

#[test]
fn magnet_burst_mostra_il_colpo_vero_e_non_la_zona_di_potenza_zero() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let (seen, events) = zones_of(&Arc::new(catalog), 197);
    assert_eq!(
        events,
        [
            (16, Event::HitWindowOpened { attack: 1900, power: 0 }),
            (20, Event::HitWindowClosed),
            (20, Event::HitWindowOpened { attack: 1901, power: 25 }),
            (57, Event::HitWindowClosed),
            (57, Event::HitWindowOpened { attack: 406, power: 0 }),
            (64, Event::HitWindowClosed),
            (64, Event::HitWindowOpened { attack: 1903, power: 150 }),
            (71, Event::HitWindowClosed),
        ]
    );
    let at = |frame: usize| seen[frame - 1];
    assert_eq!(at(18), (Some(1900), Some(0), false));
    assert_eq!(at(30), (Some(1901), Some(25), true));
    assert_eq!(at(60), (Some(406), None, true));
    assert_eq!(at(66), (Some(1903), Some(150), true));
}

#[test]
fn le_zone_di_danno_coperte_da_una_parata_adesso_si_vedono() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    // (animazione, colpo a segno, potenza, primo e ultimo frame in cui è la zona indicata)
    let cases = [
        (165, 876, 250, 28, 42),  // Guard Break
        (170, 1869, 125, 20, 30), // Vicinity Break
        (167, 854, 250, 34, 38),  // Finishing Leap
        (166, 911, 25, 32, 56),   // Explosion: i colpi piccoli...
        (166, 912, 150, 57, 64),  // ...e quello finale
        (197, 1901, 25, 20, 56),  // Magnet Burst: i colpi piccoli...
        (197, 1903, 150, 64, 70), // ...e quello finale
        (171, 1581, 100, 20, 28), // Counterguard (al frame 20 c'è ancora anche la parata)
    ];
    for (number, attack, power, first, last) in cases {
        let (seen, _) = zones_of(&catalog, number);
        let frames: Vec<usize> = (1..=seen.len())
            .filter(|frame| seen[frame - 1].0 == Some(attack))
            .collect();
        let expected: Vec<usize> = (first..=last).collect();
        assert_eq!(frames, expected, "animazione {number}, colpo a segno {attack}");
        for frame in frames {
            assert_eq!(seen[frame - 1].1, Some(power), "animazione {number} frame {frame}");
        }
    }
    // Ogni zona di danno con una potenza, di ogni colpo del repertorio, è la zona indicata
    // in almeno un frame: nessuna resta più nascosta.
    for entry in &catalog.entries {
        let data = catalog.motion(entry.motion).unwrap();
        let (seen, _) = zones_of(&catalog, entry.motion);
        for span in data.weapon.iter().chain(data.body.iter()) {
            if span.trigger != trigger::HITBOX && span.trigger != trigger::HITBOX_COMBO {
                continue;
            }
            let attack = u16::try_from(span.params[0]).unwrap();
            let row = catalog.attack(attack).unwrap();
            if row.guards() || row.power == 0 {
                continue;
            }
            assert!(
                seen.iter().any(|(indicated, _, _)| *indicated == Some(attack)),
                "animazione {}: la zona {attack} non è mai indicata",
                entry.motion
            );
        }
    }
}

#[test]
fn una_parata_dentro_un_colpo_ferma_il_colpo_nemico_secondo_il_suo_tipo() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    // Guard Break para con una zona di tipo "parata" (la 162): parte la parata riuscita.
    let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
    combo.override_next_move(Some(165));
    combo.tick(press());
    wait_frame(&mut combo, 30);
    assert!(combo.guarding());
    let events = combo.guarded();
    assert!(events.contains(&Event::ReactionStarted { motion: motion::GUARD_SUCCESS }));
    // Counterguard ha solo una "parata totale" (la 124) nei frame 1-20: non cambia niente.
    let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
    combo.override_next_move(Some(171));
    combo.tick(press());
    wait_frame(&mut combo, 10);
    assert!(combo.guarding() && combo.unhittable());
    assert!(combo.guarded().is_empty());
    assert_eq!(combo.current_move(), Some((171, 10)));
    assert_eq!(combo.step(), 1);
}

#[test]
fn il_colpo_del_bordo_con_un_bersaglio_sale_tra_21_e_71() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    let rise_with = |target: Option<Body>| {
        let mut combo = Combo::new(Arc::clone(&catalog), Loadout::default());
        let scene = Scene { main_target: target, ..Scene::default() };
        let events = combo.tick(TickInput { command: Some(1), scene, ..TickInput::default() });
        assert_eq!(started(&events), Some((1, 181, 1)));
        rise_of(&events).unwrap()
    };
    assert_eq!(rise_with(None), 21.0);
    // I due limiti di dislivello della voce sono uguali (zero): non guarda il dislivello, e
    // la salita va come per una voce in aria, qualunque sia la taglia del bersaglio.
    let sized = |up: f32| Some(Body { size: 30.0, ..enemy(0.0, up, 100.0) });
    assert_eq!(rise_with(sized(-50.0)), 21.0);
    assert_eq!(rise_with(sized(0.0)), 21.0);
    assert_eq!(rise_with(sized(48.5)), 48.5);
    assert_eq!(rise_with(sized(200.0)), 71.0);
}

#[test]
fn nessuna_voce_ha_un_campo_che_farebbe_andare_lo_slancio_all_indietro() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    // SPEC 13: una velocità di partenza sotto zero può uscire solo se il massimo della voce
    // è negativo. Nel catalogo vero non capita mai: lo slancio non va mai all'indietro.
    let lunging: Vec<&Entry> = catalog.entries.iter().filter(|entry| entry.lunges()).collect();
    assert!(!lunging.is_empty());
    assert!(lunging.iter().all(|entry| entry.speed_max >= 0.0));
    assert!(lunging.iter().any(|entry| entry.speed_min < 0.0));
}
