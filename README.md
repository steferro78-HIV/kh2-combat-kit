# KH2 Combat Kit

The combat rules of **Kingdom Hearts II Final Mix** (Sora, base form), rebuilt as a small
Rust library you can plug into another game.

*Leggi in italiano: [README.it.md](README.it.md).*

- **No game files are included.** The library reads a "catalog" that you generate from your
  own copy of the game with the scripts in `tools/kh2`.
- **No graphics, no dependencies.** The library only decides *what happens*: which attack
  starts, when a button press counts, how high a jump goes. Your game draws it.
- **Checked against the real game.** The rules were replayed against frame-by-frame
  recordings of KH2: see [How faithful is it](#how-faithful-is-it).

This is an unofficial fan project. It is not affiliated with or endorsed by Square Enix or
Disney.

## What is in the box

| Folder | What it is |
|---|---|
| `src/kh2-combat-core` | The library (Rust, no dependencies). This is what you integrate. |
| `src/first-combo` | The smallest program that uses the library: it prints one combo in the terminal. |
| `src/arena` | A small test arena made with Bevy: a working example of a game that uses the library. |
| `tools/kh2` | Python scripts that read your copy of KH2 and write the catalog (and, for the arena, Sora's model and animations). |
| `docs/SPEC-FUNZIONALE.md`, `docs/INTERFACCIA.md` | The two documents the library was written from (in Italian): how the combat behaves, and the shape of the library, of the catalog file and of the recordings. |
| `PROVENANCE.md` | How the library's code was produced: a clean-room rewrite from those two documents. |

## What the library covers

- **Ground and air combos**: which attack starts depends on the distance, height and angle
  of the target, on the enemies around, on whether you are in the air, and on the equipped
  abilities. This is KH2's own selection table, not a fixed sequence.
- **When a press counts**: combo windows, the "your attack must have touched something"
  rule, ignored presses.
- **Combo length and finishers**: Combo Plus, Air Combo Plus, Negative Combo, Finishing
  Plus, Combo Master, Berserk Charge.
- **Every attack ability of Sora's base moveset**: Slide Dash, Flash Step, Slapshot, Dodge
  Slash, Vicinity Break, Guard Break, Explosion, Finishing Leap, Upper Slash, Aerial Sweep,
  Aerial Dive, Aerial Spiral, Horizontal Slash, Aerial Finish, Magnet Burst.
- **Movement in the air**: variable-height jump, the small hop of every air attack, falling,
  landing, and the forward lunge of air attacks.
- **Defence**: Guard, successful parry, Counterguard, weapon bouncing off an enemy guard,
  being launched, Retaliating Slash, Aerial Recovery, "cannot be hit" frames.

**Not covered:** Dodge Roll, magic, Drive Forms, Limits, summons, reaction commands (the
library only tells you when one is allowed), enemy behaviour, damage calculation (the
formula is described in the specification, the catalog carries power and knockback of every
hit), levels and menus.

## Requirements

- **Rust** 1.85 or newer (https://rustup.rs).
- **Python** 3.10 or newer (standard library only), to build the catalog.
- Your own copy of **KINGDOM HEARTS -HD 1.5+2.5 ReMIX-** for PC. Tested with the Steam
  version, build 1.0.0.10.
- **OpenKH** (https://github.com/OpenKH/OpenKh), used as a program to unpack the game's
  archives.
- Windows, because the game and the OpenKH tools are Windows programs. The library itself
  builds anywhere Rust does.

## Quick start

1. Tell the scripts where OpenKH is (the folder that contains `OpenKh.Command.IdxImg.exe`).
   If the game is not in Steam's default folder, also set `KH2_GAME_DIR` to the game folder
   (the one that contains `Image`).

   ```powershell
   $env:OPENKH_DIR = "D:\tools\OpenKH"
   ```

2. Build the catalog. The game installation is only read. Everything is written to
   `extracted/`, which Git ignores: **never commit it**.

   ```bash
   python tools/kh2/build_catalog.py
   ```

3. Run the tests.

   ```bash
   cargo test --manifest-path src/Cargo.toml
   ```

4. See it work in the terminal: a fighter, an enemy standing still, and a player who
   presses attack whenever the combo can go on. It prints what the library decides, tick by
   tick (the source is `src/first-combo/src/main.rs`, a good place to start reading).

   ```bash
   cargo run --manifest-path src/Cargo.toml -p first-combo
   ```

5. Optional: try it in the arena. The first command prepares Sora's model and animations
   (it needs `OpenKh.Research.Kh2Anim.exe` from OpenKH); without it the arena still starts.

   ```bash
   python tools/kh2/prepare_arena.py
   ```

   ```bash
   cargo run --manifest-path src/Cargo.toml -p arena
   ```

   Keys: `WASD` move, `Space` attack, `K` jump (hold for a higher jump), `E` special
   attack (square), `Q` guard, `X` the dummy hits you, arrows move the dummy, `PgUp`/`PgDn`
   raise and lower it, `N` second dummy, `F1`/`F2` choose an ability and `F3` toggle it,
   `1`/`2`/`3` game speed. A gamepad works too.

## Integrating it into your game

### Who does what

| The library | Your game |
|---|---|
| Chooses the attack, runs its frames, opens and closes hit and combo windows | Reads the buttons |
| Decides the height of the character (jump, hops, fall) | Moves the character on the ground, and tells the library where the floor is |
| Gives the forward push of air attacks, tick by tick | Adds that push to the position |
| Tells you which KH2 animation is playing and at which frame | Draws the animation |
| Tells you when the weapon can hurt, and with which power | Checks whether the weapon really touched an enemy, and applies the damage |
| Runs parry, bounce, knockback and recovery | Tells the library when an enemy attack arrives or when an enemy blocks |

Time is counted in **ticks of 1/60 of a second**: call `tick` exactly 60 times per second.
Distances are in **KH2 units** (Sora runs 8 units per tick and a full jump is 185 units
high; the arena draws 100 units as 1 metre). Positions are `[x, height, z]` with height
growing upwards.

### A minimal example

```rust
use std::sync::Arc;

use kh2_combat_core::catalog::Catalog;
use kh2_combat_core::combo::Event;
use kh2_combat_core::fighter::{Controls, Fighter};
use kh2_combat_core::rules::{Body, Loadout, Scene};

// Once, at start-up.
let catalog = Arc::new(Catalog::load(&Catalog::default_path()).expect("build the catalog first"));
let mut loadout = Loadout::default();
loadout.equip(catalog.ability_number("Slide_Dash").unwrap());
loadout.combo_plus = 1;
let mut sora = Fighter::new(catalog, loadout);

// 60 times per second.
let enemy = Body {
    position: [enemy_x, enemy_aim_height, enemy_z], // the point to aim at
    radius: 40.0,                                   // distances are measured from its edge
    size: 30.0,                                     // widens the height limits of the moves
};
let nearby = [enemy];
let events = sora.tick(Controls {
    attack_pressed,          // true only on the tick the button goes down
    command: None,           // Some(entry id) for Guard and the other "square" moves
    jump_pressed,
    jump_held,
    hit_landed,              // your collision check: the weapon touched an enemy this tick
    busy: false,
    berserk: false,
    floor: 0.0,              // height of the ground under the character
    scene: Scene {
        fighter: [player_x, 0.0, player_z],   // the library fills in the height
        facing: [facing_x, facing_z],         // unit vector
        main_target: Some(enemy),             // the locked-on (or chosen) enemy
        nearby: &nearby,                      // every enemy around, main target included
        ledge_ahead: false,
    },
});

// Apply what the library decided.
let push = sora.slide();
player_x += push[0];
player_z += push[1];
let player_height = sora.height();
match sora.shown() {
    Some((motion, frame)) => { /* draw KH2 animation number `motion` at `frame` (from 1) */ }
    None => { /* idle or running: your choice */ }
}
for event in events {
    match event {
        Event::MoveStarted { motion, .. } => { /* an attack started */ }
        Event::HitWindowOpened { power, .. } => { /* the weapon can hurt from now */ }
        Event::HitWindowClosed => {}
        _ => {}
    }
}
```

`src/first-combo/src/main.rs` is this loop in a program you can run; the
arena (`src/arena/src/main.rs`, function `simulate`) is the same loop inside a real game.

### Things your game must tell the library

- **A hit landed.** While `sora.combo.hit_power()` is `Some(power)`, check whether the weapon
  touches an enemy. If it does, pass `hit_landed: true` on that tick. Without it the combo
  does not continue, exactly as in KH2. `sora.combo.hit_attack()` gives the number of the
  hit: `catalog.attack(number)` has its power, knockback and the other values.
- **An enemy attack arrives.** If `sora.combo.guarding()` is true call `sora.guarded()`: the
  blow is stopped. If it landed on a real guard the parry starts, and from there the attack
  button gives Counterguard; some moves guard while they attack, and then the move simply goes
  on. If
  `sora.combo.unhittable()` is true the attack misses. Otherwise call
  `sora.knocked(reaction_motion, rise)`: the combo breaks, a damage animation plays, and
  with `rise` greater than zero the character is launched into the air (then Retaliating
  Slash and Aerial Recovery become possible).
- **An enemy blocked your weapon.** Call `sora.deflected()`.

### Buttons that recall a specific move

Some moves do not come from the attack button: you pass their entry number in
`Controls::command`. In Sora's catalog they are: Guard 11, Upper Slash 12, Finishing Leap 37,
Horizontal Slash 38. Counterguard (39) and Retaliating Slash (40) are recalled automatically
by the attack button when the situation is right. A command only works if the ability is
equipped and the moment is right; otherwise you get `Event::PressIgnored`.

### Abilities

`Loadout` holds what is equipped. Abilities that add moves have a number:
`loadout.equip(catalog.ability_number("Flash_Step").unwrap())`. The names are those written
in the catalog (lines starting with `ability`). Abilities that change the rules are fields:
`combo_plus`, `air_combo_plus`, `negative_combo`, `finishing_plus`, `combo_master`,
`berserk_charge`. Aerial Recovery is `rules::AERIAL_RECOVERY`. You can change the loadout at
any time through `sora.combo.loadout`.

### Animations

The library speaks in KH2 animation numbers (for example 151 is the first ground attack, 3
the jump, 4 the fall). It does not contain the animations. To see Sora move you have to
convert them from your copy of the game into the format of your engine: the scripts
`tools/kh2/prepare_arena.py` and `tools/kh2/export_arena_poses.py` show one way (they write
one pose per tick for every bone, which the arena plays back).

### Using it from another language

The library is plain Rust with no dependencies. To call it from C, C++, C# or a scripting
language, wrap `Fighter` in a small `cdylib` crate that exposes `extern "C"` functions
(create, tick, read state). That wrapper is not included.

## How the code was written

The library in `src/kh2-combat-core` is a **clean-room rewrite**. An earlier version was
written while studying the game. Before publishing, the behaviour was written down as a
functional specification (`docs/SPEC-FUNZIONALE.md`) and an interface description
(`docs/INTERFACCIA.md`), and the library was written again from scratch, from those two
documents and the data only, by a separate session that had access to neither the earlier
code nor the study notes. [PROVENANCE.md](PROVENANCE.md) says exactly what that session had
and did not have, and what the method does and does not guarantee. The working log and the
question-and-answer files it mentions are kept privately by the project owner, because they
contain personal names: they are not part of this repository. After the rewrite, the only
change made inside the library's folder was two lines of package metadata in its `Cargo.toml`
(`license` and `publish`): configuration, not code. The two documents are here exactly as that session received them (they mention a
`data\` folder: it is the working folder of the rewrite, with the catalog and the recordings).

The example, the arena and the scripts in `tools/kh2` are not part of the rewrite: they only
use the library's public interface or read the game's data files.

## How faithful is it

The rules come from three sources: the game's own data tables (read by the scripts from your
copy), frame-by-frame recordings of the real game ("telemetry"), and a written description
of the game's behaviour (`docs/SPEC-FUNZIONALE.md`). The tests replay the recordings through
the library and compare.

| What | How it was checked | Result |
|---|---|---|
| Combo rules on the ground and in the air (which presses count, when the next attack starts, when the finisher comes) | 128 recorded combos replayed | 0 differences |
| Jump and fall | 45 recorded jumps replayed, height compared at every tick | 36 within 2 units, 42 within 5 (the others hit a step or an enemy) |
| Forward lunge of air attacks | 30 recorded air attacks with a distant target | 28 travel the same distance within 8% |
| Choice of the attack for every move and ability | every entry of Sora's catalog tested in its own situation | passes; in the real game only checked for the moves without abilities |
| Effect of Combo Plus, Negative Combo, Finishing Plus | taken from community documentation | **not checked in the game** |
| Guard, parry, Counterguard, Retaliating Slash, Aerial Recovery | timings from the game data; rules from the specification | tried in the arena, **not checked against recordings** |
| Lunge speed towards a close target; rise of ground-to-air attacks | fitted on few samples | **approximate** |

The recordings are not included (they come from one player's game), so the two replay tests
skip themselves and say so. The Steam build used for all measurements is 1.0.0.10.

## The catalog file

A plain text file, one thing per line, written by `tools/kh2/build_catalog.py` to
`extracted/kh2/catalog/sora_base.catalog`:

- `entry ...` one line per move of the selection table: when it can be chosen (distance,
  height, angle, ground or air, required ability) and which animation it plays;
- `motion ...` an animation and its length in ticks;
- `span ...` and `point ...` the events of an animation: hit zones, "combo allowed",
  "finisher allowed", "cannot be hit", and so on;
- `attack ...` the numbers of a hit: power, knockback, and so on;
- `ability ...` the number and name of an ability.

`Catalog::load(path)` reads it; `Catalog::default_path()` points to the place where the
script writes it. The catalog contains game data: keep it out of any repository.

## Tests

```bash
cargo test --manifest-path src/Cargo.toml
```

- The tests inside the modules and `tests/interfaccia.rs` use small made-up catalogs: they
  run anywhere.
- `tests/catalogo_vero.rs` uses your real catalog: it skips itself until you build it.
- `tests/registrazioni.rs` replays game recordings from a `logs/` folder: it skips itself
  when there are none.

## A note on language

Code comments, test names and the two documents are in Italian; names in the code, and the
example, are in English.

## Legal

- This repository contains **no game files, no decompiled code and no extracted data**. You
  need your own legitimate copy of the game to generate the catalog.
- Do not publish the `extracted/` folder or anything generated from the game.
- Kingdom Hearts is a trademark of Disney and Square Enix. This project is unofficial.

## Credits

See [CREDITS.md](CREDITS.md).

## License

MIT, see [LICENSE](LICENSE). It covers the code and documents of this repository, not the
game or anything generated from it.
