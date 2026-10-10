//! The smallest possible "game" that uses the library: one fighter, one enemy standing still,
//! and a player who presses attack as soon as the combo can go on. It prints what the library
//! decides, tick by tick.
//!
//! Run it with: cargo run --manifest-path src/Cargo.toml -p first-combo
//! It needs the catalog written by `python tools/kh2/build_catalog.py`.

use std::sync::Arc;

use kh2_combat_core::catalog::Catalog;
use kh2_combat_core::combo::Event;
use kh2_combat_core::fighter::{Controls, Fighter};
use kh2_combat_core::rules::{Body, Loadout, Scene};

fn main() {
    let catalog = match Catalog::load(&Catalog::default_path()) {
        Ok(catalog) => Arc::new(catalog),
        Err(problem) => {
            eprintln!("Catalog not found ({problem}).\nBuild it first: python tools/kh2/build_catalog.py");
            return;
        }
    };
    // No abilities: try `loadout.equip(catalog.ability_number("Slide_Dash").unwrap())`
    // or `loadout.combo_plus = 1` and see the combo change.
    let mut sora = Fighter::new(catalog, Loadout::default());

    // One enemy on the ground, 120 units in front of the fighter.
    let enemy = Body { position: [0.0, 60.0, 120.0], radius: 40.0, size: 30.0 };
    let nearby = [enemy];
    let scene = Scene {
        fighter: [0.0, 0.0, 0.0],
        facing: [0.0, 1.0],
        main_target: Some(enemy),
        nearby: &nearby,
        ledge_ahead: false,
    };

    let mut press = true; // the first press opens the combo
    let mut hit_done = false;
    for tick in 1..=600 {
        // A real game checks here whether the weapon touches an enemy. This one always
        // hits, once for every hit window.
        let hit_landed = sora.combo.hit_power().is_some() && !hit_done;
        hit_done |= hit_landed;

        let events =
            sora.tick(Controls { attack_pressed: std::mem::take(&mut press), hit_landed, scene, ..Controls::default() });

        for event in events {
            println!("tick {tick:3}: {event:?}");
            match event {
                Event::HitWindowOpened { .. } => hit_done = false,
                // As soon as the combo can go on, press attack again.
                Event::ChainOpened => press = true,
                Event::ComboEnded => return,
                _ => {}
            }
        }
    }
}
