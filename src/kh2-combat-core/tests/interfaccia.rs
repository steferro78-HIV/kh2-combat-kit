//! La forma della libreria: nomi, campi, firme ed eventi di INTERFACCIA.md.
//!
//! Questo test serve soprattutto a **compilare**: se un nome, un campo o una firma non
//! corrisponde più all'interfaccia, qui il compilatore se ne accorge prima che se ne
//! accorga il resto del progetto.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kh2_combat_core::air::{self, Air, Drift};
use kh2_combat_core::catalog::{self, Attack, Catalog, Entry, Motion, Point, Span};
use kh2_combat_core::combo::{Combo, Event, IgnoreReason, TickInput};
use kh2_combat_core::fighter::{Controls, Fighter};
use kh2_combat_core::rules::{self, Body, FighterView, Limits, Loadout, Scene};

fn entry() -> Entry {
    Entry {
        id: 5,
        kind: catalog::kind::ALWAYS,
        alternative: -1,
        step_advance: 0,
        flags: 0,
        motion: 10,
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
        inner_min: 1.0,
        inner_max: 1.0,
        blend: 0.0,
        distance_adjust: 0.0,
        ability: 0,
        score: 0,
    }
}

fn small_catalog() -> Catalog {
    let window = Span {
        start: 1,
        end: -1,
        trigger: catalog::trigger::COMBO_ALLOWED,
        params: Vec::new(),
    };
    let zone = Span {
        start: 1,
        end: 9,
        trigger: catalog::trigger::HITBOX,
        params: vec![7, 1],
    };
    let point = Point {
        frame: 3,
        trigger: 19,
        params: Vec::new(),
    };
    let motion = Motion {
        id: 10,
        name: String::from("PROVA"),
        frames: 20,
        body: vec![window],
        weapon: vec![zone],
        body_points: vec![point.clone()],
        weapon_points: vec![point],
    };
    let attack = Attack {
        id: 7,
        sub: 0,
        power: 100,
        attack_type: 0,
        element: 0,
        reaction: 0,
        knock1: 0,
        knock2: 0,
        interval: 0,
        revenge: 0,
        combo_group: 0,
        kind: 0,
    };
    Catalog {
        entries: vec![entry()],
        motions: BTreeMap::from([(10u16, motion)]),
        attacks: BTreeMap::from([(7u16, vec![attack])]),
        abilities: BTreeMap::from([(92u16, String::from("Slide_Dash"))]),
    }
}

fn clone_debug_eq<T: Clone + Debug + PartialEq>() {}
fn copy_too<T: Copy>() {}
fn full_eq<T: Eq>() {}
fn has_default<T: Default>() {}
fn clone_debug<T: Clone + Debug>() {}

#[test]
fn radice_della_libreria() {
    let ticks: u32 = kh2_combat_core::TICKS_PER_SECOND;
    assert_eq!(ticks, 60);
    let version: fn() -> &'static str = kh2_combat_core::version;
    assert_eq!(version(), "0.4.0");
    assert!(!version().is_empty());
}

#[test]
fn costanti_del_catalogo() {
    use catalog::{flag, kind, motion, trigger};
    let flags: [u32; 5] = [flag::AERIAL, flag::REACH_GROWS, flag::FINISHER, flag::LOW_INVERTED, flag::HIGH_INVERTED];
    assert_eq!(flags, [1, 2, 4, 8, 16]);
    let kinds: [u8; 4] = [kind::BY_COMMAND, kind::MAIN_TARGET, kind::ANY_NEARBY, kind::ALWAYS];
    assert_eq!(kinds, [0, 1, 2, 3]);
    let triggers: [u8; 9] = [
        trigger::GROUNDED,
        trigger::AIRBORNE,
        trigger::BLEND_TO_IDLE,
        trigger::HITBOX,
        trigger::COMBO_ALLOWED,
        trigger::REACTION_ALLOWED,
        trigger::UNHITTABLE,
        trigger::HITBOX_COMBO,
        trigger::FINISHER_ALLOWED,
    ];
    assert_eq!(triggers, [0, 1, 2, 10, 11, 14, 27, 33, 50]);
    let motions: [u16; 7] = [
        motion::JUMP,
        motion::FALL,
        motion::LAND,
        motion::AERIAL_RECOVERY,
        motion::DEFLECT,
        motion::GUARD_SUCCESS,
        motion::DEFLECT_AIR,
    ];
    assert_eq!(motions, [3, 4, 5, 40, 41, 42, 43]);
}

#[test]
fn tipi_e_firme_del_catalogo() {
    clone_debug_eq::<Entry>();
    clone_debug_eq::<Span>();
    full_eq::<Span>();
    clone_debug_eq::<Point>();
    full_eq::<Point>();
    clone_debug_eq::<Motion>();
    has_default::<Motion>();
    clone_debug_eq::<Attack>();
    full_eq::<Attack>();
    has_default::<Attack>();
    clone_debug_eq::<Catalog>();
    has_default::<Catalog>();

    let _: fn(&Entry) -> bool = Entry::is_aerial;
    let _: fn(&Entry) -> bool = Entry::is_finisher;
    let _: fn(&Entry) -> bool = Entry::lunges;
    let _: fn(&Span, u16) -> bool = Span::covers;
    let _: fn(&Motion, u8, u16) -> bool = Motion::active;
    let _: fn(&Motion, u8) -> Option<u16> = Motion::first_frame_of;
    let _: fn(&Motion, u16) -> bool = Motion::neutral;
    let _: fn(&Motion, u16) -> Option<u16> = Motion::hitbox;
    let _: fn(&Attack) -> bool = Attack::guards;
    let _: fn() -> PathBuf = Catalog::default_path;
    let _: fn(&Path) -> Result<Catalog, String> = Catalog::load;
    let _: fn(&str) -> Result<Catalog, String> = Catalog::parse;
    let _: fn(&Catalog, u8) -> Option<usize> = Catalog::index_of_id;
    let _: for<'a> fn(&'a Catalog, u16) -> Option<&'a Motion> = Catalog::motion;
    let _: for<'a> fn(&'a Catalog, u16) -> Option<&'a Attack> = Catalog::attack;
    let _: fn(&Catalog, &str) -> Option<u16> = Catalog::ability_number;

    let catalog = small_catalog();
    assert_eq!(catalog.index_of_id(5), Some(0));
    assert_eq!(catalog.ability_number("Slide_Dash"), Some(92));
    assert_eq!(catalog.attack(7).map(|attack| attack.power), Some(100));
    assert_eq!(catalog.motion(10).map(|motion| motion.frames), Some(20));
}

#[test]
fn tipi_e_firme_delle_regole() {
    let counter: u8 = rules::COUNTERGUARD;
    let retaliate: u8 = rules::RETALIATING_SLASH;
    let recovery: u16 = rules::AERIAL_RECOVERY;
    assert_eq!((counter, retaliate, recovery), (39, 40, 20));

    clone_debug_eq::<Loadout>();
    full_eq::<Loadout>();
    has_default::<Loadout>();
    clone_debug_eq::<Limits>();
    copy_too::<Limits>();
    full_eq::<Limits>();
    clone_debug_eq::<Body>();
    copy_too::<Body>();
    has_default::<Body>();
    clone_debug_eq::<Scene>();
    copy_too::<Scene>();
    has_default::<Scene>();
    clone_debug_eq::<FighterView>();
    copy_too::<FighterView>();
    has_default::<FighterView>();

    let _: fn(&mut Loadout, u16) = Loadout::equip;
    let _: fn(&mut Loadout, u16) = Loadout::unequip;
    let _: fn(&Loadout, u16) -> bool = Loadout::has;
    let _: fn(&Loadout) -> Limits = Loadout::limits;
    let _: fn(&Entry, &Loadout, &FighterView) -> bool = rules::available;
    let _: fn(&Catalog, &Loadout, &FighterView, &Scene) -> Vec<bool> = rules::candidates;
    let _: for<'a> fn(&'a Catalog, &Loadout, &FighterView, &Scene) -> Option<&'a Entry> = rules::choose;
    let _: for<'a> fn(&'a Catalog, &Loadout, &FighterView, u8) -> Option<&'a Entry> = rules::by_command;

    let mut loadout = Loadout::default();
    loadout.combo_plus = 1;
    loadout.air_combo_plus = 2;
    loadout.negative_combo = false;
    loadout.finishing_plus = 1;
    loadout.combo_master = true;
    loadout.berserk_charge = true;
    let limits = loadout.limits();
    let Limits { ground_steps, air_steps, finishers } = limits;
    assert_eq!((ground_steps, air_steps, finishers), (4, 5, 2));

    let body = Body { position: [0.0, 0.0, 50.0], radius: 10.0, size: 0.0 };
    let nearby = [body];
    let scene = Scene {
        fighter: [0.0, 0.0, 0.0],
        facing: [0.0, 1.0],
        main_target: Some(body),
        nearby: &nearby,
        ledge_ahead: false,
    };
    let view = FighterView {
        airborne: false,
        current_motion: None,
        steps: 1,
        finishers: 1,
        finisher_window: false,
        reach_bonus: 0.0,
        berserk: false,
    };
    let catalog = small_catalog();
    assert_eq!(rules::candidates(&catalog, &loadout, &view, &scene), [true]);
    assert_eq!(rules::choose(&catalog, &loadout, &view, &scene).map(|entry| entry.id), Some(5));
    assert_eq!(rules::by_command(&catalog, &loadout, &view, 5).map(|entry| entry.id), Some(5));
}

#[test]
fn tipi_e_firme_della_combo() {
    clone_debug_eq::<TickInput>();
    copy_too::<TickInput>();
    has_default::<TickInput>();
    clone_debug_eq::<IgnoreReason>();
    copy_too::<IgnoreReason>();
    full_eq::<IgnoreReason>();
    clone_debug_eq::<Event>();
    copy_too::<Event>();
    clone_debug::<Combo>();

    let _: fn(Arc<Catalog>, Loadout) -> Combo = Combo::new;
    let _: for<'a> fn(&'a Combo) -> &'a Catalog = Combo::catalog;
    let _: fn(&mut Combo, Option<u16>) = Combo::override_next_move;
    let _: for<'a> fn(&'a Combo) -> Option<&'a Entry> = Combo::current_entry;
    let _: fn(&Combo) -> Option<(u16, u16)> = Combo::current_move;
    let _: fn(&Combo) -> Option<u16> = Combo::hit_power;
    let _: fn(&Combo) -> Option<u16> = Combo::hit_attack;
    let _: fn(&Combo) -> bool = Combo::guarding;
    let _: fn(&Combo) -> bool = Combo::unhittable;
    let _: fn(&Combo) -> bool = Combo::reaction_allowed;
    let _: fn(&Combo) -> bool = Combo::free;
    let _: fn(&Combo) -> bool = Combo::launched;
    let _: fn(&Combo) -> u8 = Combo::step;
    let _: fn(&Combo) -> u8 = Combo::finishers;
    let _: fn(&mut Combo) -> Vec<Event> = Combo::interrupt;
    let _: fn(&mut Combo, bool) -> Vec<Event> = Combo::deflected;
    let _: fn(&mut Combo) -> Vec<Event> = Combo::guarded;
    let _: fn(&mut Combo, u16, bool) -> Vec<Event> = Combo::knocked;
    let _: fn(&mut Combo, TickInput) -> Vec<Event> = Combo::tick;

    // Tutti gli eventi e tutti i motivi, con i loro campi.
    let events = [
        Event::MoveStarted { entry: 5, motion: 10, step: 1 },
        Event::ReactionStarted { motion: 41 },
        Event::Lunge { rise: 21.0, run: 0.0, speed: 8.0 },
        Event::HitWindowOpened { attack: 7, power: 100 },
        Event::HitWindowClosed,
        Event::ChainOpened,
        Event::PressIgnored(IgnoreReason::Busy),
        Event::PressIgnored(IgnoreReason::TooEarly),
        Event::PressIgnored(IgnoreReason::NotTouched),
        Event::PressIgnored(IgnoreReason::ComboOver),
        Event::PressIgnored(IgnoreReason::Unavailable),
        Event::ComboEnded,
        Event::Jumped,
        Event::Landed,
    ];
    assert_eq!(events.len(), 14);

    let mut combo = Combo::new(Arc::new(small_catalog()), Loadout::default());
    combo.loadout.combo_master = true;
    let events = combo.tick(TickInput {
        attack_pressed: true,
        command: None,
        recover_pressed: false,
        hit_landed: false,
        busy: false,
        airborne: false,
        berserk: false,
        scene: Scene::default(),
    });
    // Al frame 1 l'animazione di prova ha già una zona e la finestra aperta.
    assert_eq!(
        events,
        [
            Event::MoveStarted { entry: 5, motion: 10, step: 1 },
            Event::HitWindowOpened { attack: 7, power: 100 },
            Event::ChainOpened,
        ]
    );
}

#[test]
fn tipi_e_firme_dell_aria() {
    let jump: f32 = air::JUMP_HEIGHT;
    let gravity: f32 = air::GRAVITY;
    assert_eq!((jump, gravity), (185.0, 0.408));
    let _: fn(f32) -> f32 = air::rise_ticks;
    let _: fn(f32) -> f32 = air::drift_reach;

    clone_debug_eq::<Drift>();
    copy_too::<Drift>();
    clone_debug_eq::<Air>();
    copy_too::<Air>();
    has_default::<Air>();

    let _: fn([f32; 2], f32) -> Drift = Drift::new;
    let _: fn(&mut Drift) -> [f32; 2] = Drift::tick;
    let _: fn(&Air) -> f32 = Air::height;
    let _: fn(&Air) -> bool = Air::airborne;
    let _: fn(&Air) -> bool = Air::jumping;
    let _: fn(&mut Air) -> bool = Air::jump;
    let _: fn(&mut Air, f32) = Air::hop;
    let _: fn(&mut Air, bool, f32) -> bool = Air::tick;
}

#[test]
fn tipi_e_firme_del_personaggio() {
    clone_debug_eq::<Controls>();
    copy_too::<Controls>();
    has_default::<Controls>();
    clone_debug::<Fighter>();

    let _: fn(Arc<Catalog>, Loadout) -> Fighter = Fighter::new;
    let _: fn(&Fighter) -> f32 = Fighter::height;
    let _: fn(&Fighter) -> bool = Fighter::airborne;
    let _: fn(&Fighter) -> [f32; 2] = Fighter::slide;
    let _: fn(&Fighter) -> Option<(u16, u16)> = Fighter::shown;
    let _: fn(&mut Fighter) -> Vec<Event> = Fighter::deflected;
    let _: fn(&mut Fighter) -> Vec<Event> = Fighter::guarded;
    let _: fn(&mut Fighter, u16, f32) -> Vec<Event> = Fighter::knocked;
    let _: fn(&mut Fighter, Controls) -> Vec<Event> = Fighter::tick;

    let mut fighter = Fighter::new(Arc::new(small_catalog()), Loadout::default());
    // Il campo `combo` è pubblico.
    fighter.combo.loadout.combo_plus = 1;
    let events = fighter.tick(Controls {
        attack_pressed: false,
        command: None,
        jump_pressed: true,
        jump_held: true,
        hit_landed: false,
        busy: false,
        berserk: false,
        floor: 0.0,
        scene: Scene::default(),
    });
    assert_eq!(events, [Event::Jumped]);
    assert!(fighter.airborne());
    assert!(fighter.height() > 0.0);
    assert_eq!(fighter.slide(), [0.0, 0.0]);
    assert_eq!(fighter.shown(), Some((catalog::motion::JUMP, 1)));
}
