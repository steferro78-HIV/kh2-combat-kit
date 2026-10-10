//! La riproduzione delle registrazioni del gioco vero (INTERFACCIA.md, sezione 8; i
//! risultati attesi sono in SPEC-FUNZIONALE.md, sezione 14).
//!
//! Per vedere i numeri: `cargo test --manifest-path src\Cargo.toml -- --nocapture`.

mod common;

use std::sync::Arc;

use common::{attack_down, jump_down, Log};
use kh2_combat_core::air::{Air, Drift};
use kh2_combat_core::catalog::{kind, Catalog};
use kh2_combat_core::combo::{Combo, Event, IgnoreReason, TickInput};
use kh2_combat_core::rules::Loadout;

/// Le due registrazioni che hanno le colonne per la verifica 1; solo la seconda, la più
/// lunga, ha anche le posizioni (verifiche 2 e 3). Per queste i numeri attesi sono noti.
const SHORT_RECORDING: &str = "kh2_telemetry_20261007_210905.csv";
const LONG_RECORDING: &str = "kh2_telemetry_20261007_214302.csv";

/// Il risultato della verifica 1 su una registrazione.
#[derive(Debug, Default)]
struct ComboReplay {
    /// Combo del gioco affiancate da una combo della libreria.
    combos: usize,
    /// Prosecuzioni uguali: gioco e libreria fanno partire il colpo successivo della combo
    /// con lo stesso numero di passi (più di 1).
    continuations: usize,
    /// Ripartenze uguali: mentre si seguiva una combo, gioco e libreria ne aprono una nuova
    /// (passi = 1) nello stesso fotogramma. Non sono né combo nuove né prosecuzioni.
    /// Si contano a parte (R11): 8 in tutto nelle due registrazioni note, 3 e 5.
    restarts: usize,
    /// Pressioni ignorate dalla libreria (e dal gioco), per motivo.
    too_early: usize,
    not_touched: usize,
    combo_over: usize,
    other_ignored: usize,
    /// Le differenze, una riga di testo ciascuna.
    differences: Vec<String>,
}

/// Verifica 1: le decisioni della combo. `None` se alla registrazione mancano le colonne.
fn replay_combos(log: &Log, catalog: &Arc<Catalog>) -> Option<ComboReplay> {
    if !log.has(&["frame", "input", "motion", "touch", "step"]) {
        return None;
    }
    let frame = log.column("frame")?;
    let input = log.column("input")?;
    let motion = log.column("motion")?;
    let touch = log.column("touch")?;
    let step = log.column("step")?;
    let speed = log.column("gamspd");

    // "È un colpo": c'è una voce con quell'animazione e di tipo diverso da 0.
    let is_move = |number: f64| {
        catalog
            .entries
            .iter()
            .any(|entry| f64::from(entry.motion) == number && entry.kind != kind::BY_COMMAND)
    };
    // "In aria": salto, caduta, oppure l'animazione di una voce "in aria" o di un colpo in volo.
    let in_air = |number: f64| {
        number == 3.0
            || number == 4.0
            || catalog.entries.iter().any(|entry| {
                f64::from(entry.motion) == number && (entry.is_aerial() || entry.lunges())
            })
    };

    let mut result = ComboReplay::default();
    let mut followed: Option<Combo> = None;
    for tick in 1..log.len() {
        let before = tick - 1;
        let now_motion = log.at(tick, motion);
        let now_step = log.at(tick, step);
        let press = attack_down(log.at(tick, input)) && !attack_down(log.at(before, input));
        let game_started = is_move(now_motion)
            && now_step != 0.0
            && (now_motion != log.at(before, motion) || now_step != log.at(before, step));
        let airborne = in_air(log.at(before, motion));
        let touched_now = log.at(tick, touch) == 1.0 && log.at(before, touch) != 1.0;
        let place = format!("{} fotogramma {}", log.name, log.at(tick, frame));

        let Some(combo) = followed.as_mut() else {
            // Non si sta seguendo una combo: se ne comincia una quando il gioco fa partire
            // un colpo con passi = 1.
            if game_started && now_step == 1.0 {
                let mut combo = Combo::new(Arc::clone(catalog), Loadout::default());
                combo.override_next_move(Some(now_motion as u16));
                let events = combo.tick(TickInput {
                    attack_pressed: true,
                    airborne,
                    ..TickInput::default()
                });
                combo.override_next_move(None);
                result.combos += 1;
                if started_step(&events) == Some(1) {
                    followed = Some(combo);
                } else {
                    result.differences.push(format!(
                        "{place}: la combo nuova non è partita nella libreria ({events:?})"
                    ));
                }
            }
            continue;
        };

        // Rallentatore: si smette di seguire, senza contarla come differenza.
        if speed.is_some_and(|speed| log.at(tick, speed) != 1.0) {
            followed = None;
            continue;
        }
        // "Il gioco ha lasciato il colpo": non ne ha fatto partire uno e l'animazione della
        // riga (solo se è un colpo) non è quella che la libreria ha in corso.
        let in_library = combo.current_move().map(|(number, _)| f64::from(number));
        let in_game = is_move(now_motion).then_some(now_motion);
        let left = !game_started && in_game != in_library;

        combo.override_next_move(game_started.then_some(now_motion as u16));
        let events = combo.tick(TickInput {
            attack_pressed: press && !left,
            hit_landed: touched_now,
            airborne,
            ..TickInput::default()
        });
        combo.override_next_move(None);

        if left {
            followed = None;
            continue;
        }
        let library_step = started_step(&events);
        if game_started {
            if library_step.map(f64::from) == Some(now_step) {
                if now_step == 1.0 {
                    result.restarts += 1;
                    println!(
                        "  ripartenza uguale in {place}: animazione {} -> {now_motion}",
                        log.at(before, motion)
                    );
                } else {
                    result.continuations += 1;
                }
                // "Passi ≥ 3" deve coincidere con "il colpo partito è un finale".
                let finisher = combo.current_entry().is_some_and(|entry| entry.is_finisher());
                if (now_step >= 3.0) != finisher {
                    result
                        .differences
                        .push(format!("{place}: passi {now_step} ma finale = {finisher}"));
                }
            } else {
                result.differences.push(format!(
                    "{place}: nel gioco parte un colpo con passi {now_step}, nella libreria {events:?}"
                ));
                followed = None;
            }
            continue;
        }
        if library_step.is_some() {
            result.differences.push(format!(
                "{place}: nel gioco non parte niente, nella libreria {events:?}"
            ));
            followed = None;
            continue;
        }
        if events.contains(&Event::ComboEnded) {
            result.differences.push(format!(
                "{place}: nel gioco il colpo continua, nella libreria è finito ({events:?})"
            ));
            followed = None;
            continue;
        }
        for event in &events {
            match event {
                Event::PressIgnored(IgnoreReason::TooEarly) => result.too_early += 1,
                Event::PressIgnored(IgnoreReason::NotTouched) => result.not_touched += 1,
                Event::PressIgnored(IgnoreReason::ComboOver) => result.combo_over += 1,
                Event::PressIgnored(_) => result.other_ignored += 1,
                _ => {}
            }
        }
    }
    Some(result)
}

/// I passi del colpo partito in questo tick, se ne è partito uno.
fn started_step(events: &[Event]) -> Option<u8> {
    events.iter().find_map(|event| match event {
        Event::MoveStarted { step, .. } => Some(*step),
        _ => None,
    })
}

/// Verifica 2: i salti. Per ogni salto confrontato, la differenza di altezza più grande.
/// `None` se alla registrazione mancano le colonne.
fn replay_jumps(log: &Log) -> Option<Vec<f64>> {
    if !log.has(&["input", "motion", "py"]) {
        return None;
    }
    let input = log.column("input")?;
    let motion = log.column("motion")?;
    let py = log.column("py")?;
    // L'asse `py` cresce verso il basso.
    let height = |tick: usize| -log.at(tick, py);

    let mut worst = Vec::new();
    for begin in 1..log.len() {
        if log.at(begin, motion) != 3.0 || log.at(begin - 1, motion) == 3.0 {
            continue;
        }
        let base = height(begin);
        // L'animazione parte un po' prima che Sora si stacchi: il confronto comincia dalla
        // prima riga, ancora con animazione 3, in cui l'altezza supera quella di partenza.
        let mut lift_off = begin;
        while lift_off < log.len() && log.at(lift_off, motion) == 3.0 && height(lift_off) <= base {
            lift_off += 1;
        }
        if lift_off >= log.len() || log.at(lift_off, motion) != 3.0 {
            continue;
        }
        let mut air = Air::default();
        air.jump();
        let mut largest = 0.0f64;
        let mut tick = lift_off;
        while tick < log.len() && (log.at(tick, motion) == 3.0 || log.at(tick, motion) == 4.0) {
            // Un terreno lontanissimo verso il basso: così non atterra mai.
            air.tick(jump_down(log.at(tick, input)), -1.0e9);
            let difference = (f64::from(air.height()) - (height(tick) - base)).abs();
            largest = largest.max(difference);
            tick += 1;
        }
        worst.push(largest);
    }
    Some(worst)
}

/// Verifica 3: lo slancio dei colpi aerei. Per ogni colpo confrontato: (strada nel gioco,
/// strada dello slancio della libreria). `None` se alla registrazione mancano le colonne.
fn replay_drifts(log: &Log, catalog: &Catalog) -> Option<Vec<(f64, f64)>> {
    if !log.has(&["motion", "px", "pz", "tgt", "tx", "tz"]) {
        return None;
    }
    let motion = log.column("motion")?;
    let px = log.column("px")?;
    let pz = log.column("pz")?;
    let tgt = log.column("tgt")?;
    let tx = log.column("tx")?;
    let tz = log.column("tz")?;

    let mut roads = Vec::new();
    for begin in 1..log.len() {
        let before = begin - 1;
        let number = log.at(begin, motion);
        let previous = log.at(before, motion);
        if !(number == 181.0 || number == 182.0) || previous == number {
            continue;
        }
        // Sora era già in aria.
        if ![3.0, 4.0, 181.0, 182.0].contains(&previous) {
            continue;
        }
        // Nella riga precedente c'è un bersaglio ad almeno 150 unità in orizzontale.
        if !(log.at(before, tgt) > 0.0) {
            continue;
        }
        let away = (log.at(before, tx) - log.at(before, px))
            .hypot(log.at(before, tz) - log.at(before, pz));
        if !(away >= 150.0) {
            continue;
        }
        // La voce è la prima "in aria" con quell'animazione.
        let entry = catalog
            .entries
            .iter()
            .find(|entry| f64::from(entry.motion) == number && entry.is_aerial())
            .expect("il catalogo vero ha i colpi aerei di base");

        // Strada di Sora per tutta la durata dell'animazione, contro uno slancio partito
        // alla velocità massima della voce per lo stesso numero di tick.
        let mut drift = Drift::new([0.0, 1.0], entry.speed_max);
        let mut in_game = 0.0;
        let mut in_library = 0.0;
        let mut tick = begin;
        while tick < log.len() && log.at(tick, motion) == number {
            in_game += (log.at(tick, px) - log.at(tick - 1, px))
                .hypot(log.at(tick, pz) - log.at(tick - 1, pz));
            let step = drift.tick();
            in_library += f64::from(step[0]).hypot(f64::from(step[1]));
            tick += 1;
        }
        roads.push((in_game, in_library));
    }
    Some(roads)
}

#[test]
fn verifica_1_le_decisioni_della_combo() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let catalog = Arc::new(catalog);
    let recordings = common::recordings();
    if recordings.is_empty() {
        return;
    }
    let mut useful = Vec::new();
    for log in &recordings {
        let Some(result) = replay_combos(log, &catalog) else {
            println!("{}: mancano le colonne per la verifica 1, saltata", log.name);
            continue;
        };
        println!(
            "{}: combo rigiocate {}, prosecuzioni uguali {}, ripartenze uguali {}, differenze {}; ignorate: {} troppo presto, {} non ha toccato, {} combo finita, {} altro",
            log.name,
            result.combos,
            result.continuations,
            result.restarts,
            result.differences.len(),
            result.too_early,
            result.not_touched,
            result.combo_over,
            result.other_ignored,
        );
        for difference in &result.differences {
            println!("  DIFFERENZA {difference}");
        }
        useful.push((log.name.clone(), result));
    }
    let total = |count: fn(&ComboReplay) -> usize| -> usize {
        useful.iter().map(|(_, result)| count(result)).sum()
    };
    let differences = total(|result| result.differences.len());
    println!(
        "VERIFICA 1: {} registrazioni utili, {} combo rigiocate, {} prosecuzioni uguali, {} ripartenze uguali, {} differenze",
        useful.len(),
        total(|result| result.combos),
        total(|result| result.continuations),
        total(|result| result.restarts),
        differences,
    );
    assert_eq!(differences, 0, "la riproduzione delle registrazioni deve dare 0 differenze");

    // I numeri attesi per le due registrazioni note (SPEC 14).
    for (name, result) in &useful {
        assert_eq!(result.other_ignored, 0, "{name}");
        if name == SHORT_RECORDING {
            assert_eq!((result.combos, result.continuations), (36, 26), "{name}");
        }
        if name == LONG_RECORDING {
            assert_eq!((result.combos, result.continuations), (92, 107), "{name}");
            assert_eq!(
                (result.too_early, result.not_touched, result.combo_over),
                (286, 22, 66),
                "pressioni ignorate in {name}"
            );
        }
    }
}

#[test]
fn verifica_2_i_salti() {
    let recordings = common::recordings();
    let mut compared = 0;
    for log in &recordings {
        let Some(mut worst) = replay_jumps(log) else {
            println!("{}: mancano le colonne per la verifica 2, saltata", log.name);
            continue;
        };
        compared += 1;
        let within = |limit: f64| worst.iter().filter(|difference| **difference <= limit).count();
        let summary = (worst.len(), within(2.0), within(5.0));
        println!(
            "VERIFICA 2, {}: {} salti, {} entro 2 unità, {} entro 5",
            log.name, summary.0, summary.1, summary.2
        );
        worst.sort_by(|a, b| a.total_cmp(b));
        println!("  differenza più grande di ogni salto, in ordine: {worst:.1?}");
        // SPEC 14: 45 salti, 36 entro 2 unità, 42 entro 5; i 3 che non tornano finiscono
        // contro un gradino o un nemico.
        if log.name == LONG_RECORDING {
            assert_eq!(summary, (45, 36, 42), "{}", log.name);
        }
    }
    if compared == 0 {
        println!("SALTATO: nessuna registrazione con le colonne dei salti");
    }
}

#[test]
fn verifica_3_lo_slancio_dei_colpi_aerei() {
    let Some(catalog) = common::real_catalog() else {
        return;
    };
    let recordings = common::recordings();
    let mut compared = 0;
    for log in &recordings {
        let Some(roads) = replay_drifts(log, &catalog) else {
            println!("{}: mancano le colonne per la verifica 3, saltata", log.name);
            continue;
        };
        compared += 1;
        let close = roads
            .iter()
            .filter(|(in_game, in_library)| (in_game - in_library).abs() <= 0.08 * in_library)
            .count();
        println!(
            "VERIFICA 3, {}: {} colpi aerei con il bersaglio lontano, {} con la stessa strada entro l'8%",
            log.name,
            roads.len(),
            close
        );
        for (in_game, in_library) in &roads {
            println!(
                "  gioco {in_game:.1}, libreria {in_library:.1}, rapporto {:.3}",
                in_game / in_library
            );
        }
        // SPEC 14: 30 colpi, 28 fanno la stessa strada entro l'8%.
        if log.name == LONG_RECORDING {
            assert_eq!((roads.len(), close), (30, 28), "{}", log.name);
        }
    }
    if compared == 0 {
        println!("SALTATO: nessuna registrazione con le colonne delle posizioni");
    }
}

#[test]
fn le_righe_mancanti_di_una_registrazione_valgono_come_la_precedente() {
    // Questo non usa i dati veri: controlla solo come si legge un file.
    let log = Log::parse(
        "prova.csv",
        "# un commento\nframe,input,motion,tgt\n10,08000004,151,-1\n13,00000100,3,84F1A840\n14,00000000,4,0\n",
    );
    assert_eq!(log.len(), 5);
    assert!(log.has(&["frame", "motion"]) && !log.has(&["frame", "py"]));
    let motion = log.column("motion").unwrap();
    let input = log.column("input").unwrap();
    let tgt = log.column("tgt").unwrap();
    let motions: Vec<f64> = (0..log.len()).map(|tick| log.at(tick, motion)).collect();
    assert_eq!(motions, [151.0, 151.0, 151.0, 3.0, 4.0]);
    assert!(attack_down(log.at(2, input)) && !attack_down(log.at(3, input)));
    assert!(jump_down(log.at(3, input)) && !jump_down(log.at(4, input)));
    assert!(log.at(0, tgt) < 0.0 && log.at(3, tgt) > 0.0 && log.at(4, tgt) == 0.0);
}
