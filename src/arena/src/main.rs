//! Arena di prova: una capsula (il giocatore), un manichino fermo e il pad.
//!
//! Tutto ciò che riguarda la combo lo decide `kh2-combat-core`. L'arena legge i comandi,
//! muove la capsula, dice al core dov'è il manichino e se il colpo lo ha toccato, e mostra
//! il risultato con colori, scritte e un log.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::mesh::skinning::{SkinnedMesh, SkinnedMeshInverseBindposes};
use bevy::render::mesh::{PrimitiveTopology, VertexAttributeValues};
use bevy::render::view::NoFrustumCulling;
use std::sync::Arc;

use kh2_combat_core::catalog::{motion, Catalog, Entry};
use kh2_combat_core::combo::Event;
use kh2_combat_core::fighter::{Controls, Fighter};
use kh2_combat_core::rules::{Body, Loadout, Scene, RETALIATING_SLASH};
use kh2_combat_core::TICKS_PER_SECOND;

/// 100 unità di KH2 = 1 metro nell'arena. Scelta nostra, solo per disegnare.
const METERS_PER_UNIT: f32 = 0.01;
/// Misurata sul log della quarta prova di KH2.
const RUN_SPEED: f32 = 8.0;
/// Inventato: KH2 non ha un manichino.
const DUMMY_RADIUS: f32 = 40.0;
/// Approssimata dal log: il colpo tocca se il centro del manichino è entro questa distanza.
const HIT_REACH: f32 = 150.0;
const ADVANCE_PER_TICK: f32 = 12.0;
/// Il giocatore smette di avanzare quando è a questa distanza dal bordo del manichino.
const ADVANCE_STOP_DISTANCE: f32 = 50.0;

/// Quanto il giocatore può avanzare verso il bersaglio durante un colpo a terra. Scelta per
/// come si sente nell'arena (prova dell'8 ottobre 2026), non misurata: in KH2
/// l'avanzamento è scritto nell'animazione. L'affondo (animazione 153) arriva più lontano, e i
/// colpi con abilità arrivano fin dove il repertorio li fa scegliere.
fn max_advance(entry: &Entry) -> f32 {
    if entry.ability != 0 && entry.far > 150.0 {
        entry.far
    } else if entry.motion == 153 {
        200.0
    } else {
        150.0
    }
}

// --- Numeri inventati per l'arena: in KH2 il manichino non esiste. (Salto, balzi e caduta
// sono del core, misurati sui log di KH2.)
/// Il punto del manichino a cui si mira sta a questa altezza sopra la sua base.
const DUMMY_AIM_HEIGHT: f32 = 60.0;
/// "Taglia" del manichino: allarga i limiti di dislivello delle voci del repertorio.
const DUMMY_SIZE: f32 = 30.0;
const DUMMY_SPEED: f32 = 6.0;
const DUMMY_LIFT_SPEED: f32 = 4.0;
const DUMMY_MAX_HEIGHT: f32 = 600.0;
/// Dove sta il secondo manichino (di lato al primo), quando è acceso.
const SECOND_DUMMY_OFFSET: Vec2 = Vec2::new(170.0, 0.0);
/// Un colpo tocca se il bersaglio è a questa distanza in altezza, al massimo.
const HIT_VERTICAL_REACH: f32 = 130.0;
/// Il colpo del manichino: l'animazione di danno che fa partire e di quanto sbalza Sora in
/// aria. Scelte per la prova: in KH2 dipendono dal colpo del nemico.
const KNOCK_MOTION: u16 = 64;
const KNOCK_RISE: f32 = 90.0;
/// Numeri delle voci richiamate dai comandi: parata, colpo che lancia, finale in salto, fendente aereo.
const GUARD: u8 = 11;
const UPPER_SLASH: u8 = 12;
const FINISHING_LEAP: u8 = 37;
const HORIZONTAL_SLASH: u8 = 38;
const FINISHING_LEAP_ABILITY: u16 = 95;

/// Una riga del menu delle abilità.
#[derive(Clone, Debug)]
enum Ability {
    /// Abilità che aggiunge colpi al repertorio: ha un numero nel catalogo.
    Numbered(u16),
    ComboPlus,
    AirComboPlus,
    NegativeCombo,
    FinishingPlus,
    ComboMaster,
    BerserkCharge,
}

fn ability_menu(catalog: &Catalog) -> Vec<(String, Ability)> {
    let mut menu = vec![
        ("Combo Plus".to_string(), Ability::ComboPlus),
        ("Air Combo Plus".to_string(), Ability::AirComboPlus),
        ("Negative Combo".to_string(), Ability::NegativeCombo),
        ("Finishing Plus".to_string(), Ability::FinishingPlus),
        ("Combo Master".to_string(), Ability::ComboMaster),
        ("Berserk Charge".to_string(), Ability::BerserkCharge),
    ];
    for (number, name) in &catalog.abilities {
        if catalog.entries.iter().any(|entry| entry.ability == *number) {
            menu.push((name.replace('_', " "), Ability::Numbered(*number)));
        }
    }
    menu
}

/// Lo stato di una riga del menu, come testo.
fn ability_state(loadout: &Loadout, ability: &Ability) -> String {
    let mark = |on: bool| if on { "SI".to_string() } else { "-".to_string() };
    match ability {
        Ability::Numbered(number) => mark(loadout.has(*number)),
        Ability::ComboPlus => loadout.combo_plus.to_string(),
        Ability::AirComboPlus => loadout.air_combo_plus.to_string(),
        Ability::FinishingPlus => loadout.finishing_plus.to_string(),
        Ability::NegativeCombo => mark(loadout.negative_combo),
        Ability::ComboMaster => mark(loadout.combo_master),
        Ability::BerserkCharge => mark(loadout.berserk_charge),
    }
}

/// Accende, spegne o fa crescere (le abilità che si possono avere in più copie vanno da 0 a 2).
fn change_ability(loadout: &mut Loadout, ability: &Ability) {
    match ability {
        Ability::Numbered(number) if loadout.has(*number) => loadout.unequip(*number),
        Ability::Numbered(number) => loadout.equip(*number),
        Ability::ComboPlus => loadout.combo_plus = (loadout.combo_plus + 1) % 3,
        Ability::AirComboPlus => loadout.air_combo_plus = (loadout.air_combo_plus + 1) % 3,
        Ability::FinishingPlus => loadout.finishing_plus = (loadout.finishing_plus + 1) % 3,
        Ability::NegativeCombo => loadout.negative_combo = !loadout.negative_combo,
        Ability::ComboMaster => loadout.combo_master = !loadout.combo_master,
        Ability::BerserkCharge => loadout.berserk_charge = !loadout.berserk_charge,
    }
}

/// Dalla specifica: nei primi frame di un colpo il personaggio si gira verso il bersaglio.
const TURN_FRAMES: u16 = 12;
const PLAYER_RADIUS: f32 = 30.0;
const ARENA_HALF_SIZE: f32 = 1500.0;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Dummy;

#[derive(Component)]
struct SecondDummy;

#[derive(Component)]
struct Hud;

/// Lo stato dell'arena tra un tick e l'altro.
#[derive(Resource)]
struct Arena {
    fighter: Fighter,
    tick: u64,
    /// Attacco premuto dopo l'ultimo tick: resta "in attesa" finché un tick lo consuma.
    attack_latched: bool,
    jump_latched: bool,
    jump_held: bool,
    /// Il manichino colpisce Sora (per provare parata, contrattacco, sbalzo).
    strike_latched: bool,
    /// "Quadrato": il colpo speciale (Upper Slash, Finishing Leap, Horizontal Slash).
    special_latched: bool,
    guard_latched: bool,
    move_input: Vec2,
    /// Altezza dei piedi di Sora dal pavimento: la decide il core, qui è una copia per disegnare.
    player_height: f32,
    /// Il manichino: posizione (x, z), altezza della base, e i comandi per spostarlo.
    dummy_position: Vec2,
    dummy_height: f32,
    dummy_input: Vec3,
    second_dummy: bool,
    menu: Vec<(String, Ability)>,
    menu_cursor: usize,
    /// Posizione e direzione del giocatore in unità di KH2 (x, z).
    player_position: Vec2,
    player_facing: Vec2,
    advanced: f32,
    hit_done_in_window: bool,
    touched: bool,
    chain_open: bool,
    hits: u32,
    power_total: u32,
    dummy_flash: f32,
    last_event: String,
    last_ignored: String,
    /// Velocità del tempo di gioco, in percentuale: 100 = 60 tick al secondo, come KH2.
    speed_percent: u32,
    started: Instant,
    log: BufWriter<File>,
}

impl Arena {
    fn write_log(&mut self, what: &str, distance: f32) {
        let (name, frame) = match self.fighter.combo.current_move() {
            Some((motion, frame)) => (motion.to_string(), frame),
            None => ("-".to_string(), 0),
        };
        let _ = writeln!(
            self.log,
            "{},{:.3},{},{},{},{},{:.0}",
            self.tick,
            self.started.elapsed().as_secs_f32(),
            what,
            name,
            frame,
            self.fighter.combo.step(),
            distance
        );
        let _ = self.log.flush();
    }
}

/// Legge numeri da un file binario, uno dopo l'altro.
struct Reader {
    data: Vec<u8>,
    position: usize,
}

impl Reader {
    fn open(name: &str, magic: &[u8; 4]) -> Option<Self> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted/kh2/arena").join(name);
        let data = fs::read(path).ok()?;
        data.starts_with(magic).then_some(Self { data, position: 4 })
    }

    fn bytes(&mut self, count: usize) -> Option<&[u8]> {
        let slice = self.data.get(self.position..self.position + count)?;
        self.position += count;
        Some(slice)
    }

    fn u32(&mut self) -> Option<usize> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?) as usize)
    }

    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.bytes(2)?.try_into().ok()?))
    }

    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    fn floats<const N: usize>(&mut self) -> Option<[f32; N]> {
        let mut values = [0.0; N];
        for value in &mut values {
            *value = self.f32()?;
        }
        Some(values)
    }

    fn text(&mut self) -> Option<String> {
        let length = self.u32()?;
        String::from_utf8(self.bytes(length)?.to_vec()).ok()
    }

    /// Una matrice scritta riga per riga con la traslazione nell'ultima riga è, per Bevy, la
    /// stessa matrice letta colonna per colonna.
    fn matrix(&mut self) -> Option<Mat4> {
        Some(Mat4::from_cols_array(&self.floats::<16>()?))
    }
}

/// Lo scheletro di Sora e le pose dei suoi colpi, preparati da `tools/kh2/export_arena_poses.py`
/// a partire dalla copia del gioco di chi usa l'arena. Il file non fa parte del progetto: se
/// manca, l'arena disegna solo la capsula.
#[derive(Resource)]
struct Skeleton {
    parents: Vec<i32>,
    /// Nome dell'animazione -> pose (una per tick) -> matrice assoluta di ogni osso, in unità di KH2.
    motions: HashMap<String, Vec<Vec<Mat4>>>,
}

/// Le pose della Catena Regale: stesse animazioni di Sora, ma relative all'osso della mano.
#[derive(Resource)]
struct KeybladePoses(Skeleton);

fn load_skeleton(name: &str) -> Option<Skeleton> {
    let mut reader = Reader::open(&format!("{name}_ground.bin"), b"KPOS")?;
    let bone_count = reader.u32()?;
    let parents = (0..bone_count).map(|_| reader.i32()).collect::<Option<Vec<_>>>()?;
    let mut motions = HashMap::new();
    for _ in 0..reader.u32()? {
        let name = reader.text()?;
        let mut poses = Vec::new();
        for _ in 0..reader.u32()? {
            poses.push((0..bone_count).map(|_| reader.matrix()).collect::<Option<Vec<_>>>()?);
        }
        motions.insert(name, poses);
    }
    println!("scheletro {name} caricato: {} ossa, {} animazioni", bone_count, motions.len());
    Some(Skeleton { parents, motions })
}

/// Una parte del modello: i triangoli che usano la stessa texture.
struct ModelPart {
    texture: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    joints: Vec<[u16; 4]>,
    weights: Vec<[f32; 4]>,
}

/// Un modello preparato da `tools/kh2/export_arena_model.py`. Come lo scheletro, viene dalla
/// copia del gioco di chi usa l'arena e non fa parte del progetto.
struct ModelData {
    /// Nome del modello nei file di KH2: è anche il nome della cartella con le sue texture.
    name: &'static str,
    /// Matrice di riposo di ogni osso: la posa in cui il modello è stato costruito.
    bind: Vec<Mat4>,
    parts: Vec<ModelPart>,
}

#[derive(Resource)]
struct SoraModel(ModelData);

/// La Catena Regale. Ha ossa e animazioni proprie, che si muovono rispetto alla mano destra di Sora.
#[derive(Resource)]
struct KeybladeModel(ModelData);

const SORA: &str = "P_EX100";
const KEYBLADE: &str = "W_EX010";
/// L'osso del palmo destro di Sora: è senza "pelle" e da lui partono le cinque dita.
const RIGHT_HAND_BONE: usize = 178;

fn load_model(name: &'static str) -> Option<ModelData> {
    let mut reader = Reader::open(&format!("{name}_model.bin"), b"KMDL")?;
    let bone_count = reader.u32()?;
    let bind = (0..bone_count).map(|_| reader.matrix()).collect::<Option<Vec<_>>>()?;
    let mut parts = Vec::new();
    for _ in 0..reader.u32()? {
        let texture = reader.text()?;
        let count = reader.u32()?;
        let mut part = ModelPart {
            texture,
            positions: Vec::with_capacity(count),
            normals: Vec::with_capacity(count),
            uvs: Vec::with_capacity(count),
            joints: Vec::with_capacity(count),
            weights: Vec::with_capacity(count),
        };
        for _ in 0..count {
            part.positions.push(reader.floats()?);
            part.normals.push(reader.floats()?);
            part.uvs.push(reader.floats()?);
            part.joints.push([reader.u16()?, reader.u16()?, reader.u16()?, reader.u16()?]);
            part.weights.push(reader.floats()?);
        }
        parts.push(part);
    }
    println!("modello {name} caricato: {} parti", parts.len());
    Some(ModelData { name, bind, parts })
}

/// Il materiale di una parte: i colori della sua texture, senza luci (i modelli di KH2 non ne
/// hanno di proprie).
fn part_material(
    model: &ModelData,
    part: &ModelPart,
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted/kh2/arena").join(model.name).join(&part.texture);
    let texture = fs::read(path).ok().and_then(|bytes| {
        Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::default(),
            RenderAssetUsages::default(),
        )
        .ok()
    });
    if texture.is_none() {
        println!("texture non trovata o non leggibile: {}", part.texture);
    }
    materials.add(StandardMaterial {
        base_color_texture: texture.map(|image| images.add(image)),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        alpha_mode: AlphaMode::Mask(0.5),
        ..default()
    })
}

fn part_mesh(part: &ModelPart) -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, part.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, part.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, part.uvs.clone());
    mesh
}

/// Un osso di Sora: l'arena lo sposta a ogni fotogramma nella posa da mostrare.
#[derive(Component)]
struct Joint(usize);

/// Un osso della Catena Regale.
#[derive(Component)]
struct KeybladeJoint(usize);

/// Crea le ossa e la "pelle" di un modello. Le ossa non sono una dentro l'altra: ognuna riceve
/// direttamente la sua posizione assoluta, perché è così che arrivano le pose.
fn spawn_skinned<J: Component>(
    model: &ModelData,
    joint: impl Fn(usize) -> J,
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    bindposes: &mut Assets<SkinnedMeshInverseBindposes>,
) {
    let joints: Vec<Entity> =
        (0..model.bind.len()).map(|index| commands.spawn((joint(index), Transform::default())).id()).collect();
    let inverse = bindposes.add(SkinnedMeshInverseBindposes::from(
        model.bind.iter().map(|matrix| matrix.inverse()).collect::<Vec<_>>(),
    ));
    for part in &model.parts {
        let mut mesh = part_mesh(part);
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(part.joints.clone()));
        mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, part.weights.clone());
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(part_material(model, part, images, materials)),
            SkinnedMesh { inverse_bindposes: inverse.clone(), joints: joints.clone() },
            // Le forme sono in unità di KH2 attorno all'origine: il controllo "è inquadrato?"
            // sbaglierebbe, perché la pelle viene spostata dalle ossa.
            NoFrustumCulling,
        ));
    }
}

fn spawn_model(
    model: Option<Res<SoraModel>>,
    keyblade: Option<Res<KeybladeModel>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
) {
    if let Some(model) = model {
        spawn_skinned(&model.0, Joint, &mut commands, &mut meshes, &mut materials, &mut images, &mut bindposes);
    }
    if let Some(keyblade) = keyblade {
        spawn_skinned(
            &keyblade.0,
            KeybladeJoint,
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut images,
            &mut bindposes,
        );
    }
}

/// Quanti tick dura il passaggio morbido da un'animazione alla successiva. Scelto a occhio:
/// KH2 sfuma tra le animazioni, ma di quanto non è nella specifica.
const BLEND_TICKS: u64 = 5;

/// Le pose mostrate in questo momento.
#[derive(Resource, Default)]
struct ShownPose {
    motion: String,
    index: usize,
    since_tick: u64,
    last_tick: u64,
    /// Pose da cui si sta sfumando verso l'animazione in corso.
    from: Vec<Mat4>,
    keyblade_from: Vec<Mat4>,
    /// Sora, senza lo spostamento in avanti contenuto nell'animazione.
    pose: Vec<Mat4>,
    /// La Catena Regale, rispetto alla mano destra di Sora.
    keyblade: Vec<Mat4>,
}

/// La posa a metà strada tra due pose, osso per osso.
fn blend_pose(from: &[Mat4], to: impl Iterator<Item = Mat4>, amount: f32) -> Vec<Mat4> {
    to.enumerate()
        .map(|(bone, matrix)| match from.get(bone) {
            Some(start) if amount < 1.0 => blend(*start, matrix, amount),
            _ => matrix,
        })
        .collect()
}

fn blend(from: Mat4, to: Mat4, amount: f32) -> Mat4 {
    let (from_scale, from_rotation, from_translation) = from.to_scale_rotation_translation();
    let (to_scale, to_rotation, to_translation) = to.to_scale_rotation_translation();
    Mat4::from_scale_rotation_translation(
        from_scale.lerp(to_scale, amount),
        from_rotation.slerp(to_rotation, amount),
        from_translation.lerp(to_translation, amount),
    )
}

/// Sceglie le pose da mostrare: quelle del frame deciso dal core, sfumate dalle precedenti
/// quando l'animazione cambia.
fn update_pose(
    arena: Res<Arena>,
    skeleton: Option<Res<Skeleton>>,
    keyblade_poses: Option<Res<KeybladePoses>>,
    mut shown: ResMut<ShownPose>,
) {
    let Some(skeleton) = skeleton else { return };
    if shown.last_tick == arena.tick && !shown.pose.is_empty() {
        return;
    }
    // Il frame 1 del core è la prima posa, cioè il primo tick dell'animazione.
    // Nel file delle pose ogni animazione ha per nome il suo numero in KH2: 0 fermo, 2 corsa, 4 caduta.
    let moving = arena.move_input != Vec2::ZERO;
    let (name, frame) = match arena.fighter.shown() {
        // La caduta gira in tondo; l'atterraggio si taglia appena il giocatore si muove (come in KH2).
        Some((motion::FALL, _)) => ("4".to_string(), None),
        Some((motion::LAND, _)) if moving => ("2".to_string(), None),
        Some((motion, frame)) => (motion.to_string(), Some(usize::from(frame.saturating_sub(1)))),
        None if moving => ("2".to_string(), None),
        None => ("0".to_string(), None),
    };
    // Se manca l'animazione (file delle pose vecchio) resta la posa di prima.
    let Some(poses) = skeleton.motions.get(&name).filter(|poses| !poses.is_empty()) else { return };

    let shown = &mut *shown;
    shown.last_tick = arena.tick;
    if shown.motion != name || frame.is_some_and(|frame| frame < shown.index) {
        shown.from = std::mem::take(&mut shown.pose);
        shown.keyblade_from = std::mem::take(&mut shown.keyblade);
        shown.since_tick = arena.tick;
        shown.motion = name;
    }
    let elapsed = arena.tick - shown.since_tick;
    // Da fermo e in corsa le animazioni girano in tondo: l'ultima posa si attacca alla prima.
    shown.index = match frame {
        Some(frame) => frame.min(poses.len() - 1),
        None => elapsed as usize % poses.len(),
    };
    let target = &poses[shown.index];
    let amount = ((elapsed + 1) as f32 / BLEND_TICKS as f32).min(1.0);

    // L'animazione contiene anche lo spostamento in avanti: qui lo si toglie, perché la
    // posizione del giocatore la decide l'arena.
    let drift = target[0].w_axis - poses[0][0].w_axis;
    let undo_drift = Mat4::from_translation(Vec3::new(-drift.x, 0.0, -drift.z));
    shown.pose = blend_pose(&shown.from, target.iter().map(|matrix| undo_drift * *matrix), amount);

    let keyblade_target = keyblade_poses
        .as_ref()
        .and_then(|poses| poses.0.motions.get(&shown.motion))
        .and_then(|poses| poses.get(shown.index.min(poses.len().saturating_sub(1))));
    if let Some(target) = keyblade_target {
        shown.keyblade = blend_pose(&shown.keyblade_from, target.iter().copied(), amount);
    }
}

/// Porta una posa (in unità di KH2, attorno all'origine) dove sta il giocatore nell'arena.
fn placement(arena: &Arena) -> Mat4 {
    let forward = arena.player_facing;
    let side = Vec2::new(forward.y, -forward.x);
    let scale = METERS_PER_UNIT;
    Mat4::from_cols(
        Vec4::new(side.x, 0.0, side.y, 0.0) * scale,
        Vec4::new(0.0, scale, 0.0, 0.0),
        Vec4::new(forward.x, 0.0, forward.y, 0.0) * scale,
        Vec4::new(arena.player_position.x * scale, arena.player_height * scale, arena.player_position.y * scale, 1.0),
    )
}

/// Mette ogni osso di Sora e della Catena Regale nella posa da mostrare.
fn pose_model(
    arena: Res<Arena>,
    shown: Res<ShownPose>,
    mut joints: Query<(&Joint, &mut Transform), Without<KeybladeJoint>>,
    mut keyblade_joints: Query<(&KeybladeJoint, &mut Transform), Without<Joint>>,
) {
    let place = placement(&arena);
    for (joint, mut transform) in &mut joints {
        if let Some(matrix) = shown.pose.get(joint.0) {
            *transform = Transform::from_matrix(place * *matrix);
        }
    }
    // Le pose dell'arma sono relative alla mano che la tiene.
    let Some(hand) = shown.pose.get(RIGHT_HAND_BONE) else { return };
    for (joint, mut transform) in &mut keyblade_joints {
        if let Some(matrix) = shown.keyblade.get(joint.0) {
            *transform = Transform::from_matrix(place * *hand * *matrix);
        }
    }
}

fn open_log() -> BufWriter<File> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../logs");
    fs::create_dir_all(&directory).expect("non riesco a creare la cartella dei log");
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|elapsed| elapsed.as_secs()).unwrap_or(0);
    let path = directory.join(format!("arena_{seconds}.csv"));
    let mut log = BufWriter::new(File::create(&path).expect("non riesco a creare il log dell'arena"));
    writeln!(log, "# arena avviata, core {}", kh2_combat_core::version()).unwrap();
    writeln!(log, "tick,secondi,evento,animazione,frame,passo,distanza_dal_bordo").unwrap();
    log.flush().unwrap();
    println!("log dell'arena: {}", path.display());
    log
}

fn main() {
    let catalog = match Catalog::load(&Catalog::default_path()) {
        Ok(catalog) => Arc::new(catalog),
        Err(error) => {
            println!("Manca il catalogo dei colpi: {error}");
            println!("Crealo con: python tools/kh2/prepare_arena.py");
            return;
        }
    };
    let menu = ability_menu(&catalog);
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "Arena di prova - kh2-combat-core".into(), ..default() }),
            ..default()
        }))
        .insert_resource(Time::<Fixed>::from_hz(f64::from(TICKS_PER_SECOND)))
        .insert_resource(Arena {
            fighter: Fighter::new(catalog, Loadout::default()),
            tick: 0,
            attack_latched: false,
            jump_latched: false,
            jump_held: false,
            strike_latched: false,
            special_latched: false,
            guard_latched: false,
            move_input: Vec2::ZERO,
            player_height: 0.0,
            dummy_position: Vec2::ZERO,
            dummy_height: 0.0,
            dummy_input: Vec3::ZERO,
            second_dummy: false,
            menu,
            menu_cursor: 0,
            player_position: Vec2::new(0.0, 400.0),
            player_facing: Vec2::new(0.0, -1.0),
            advanced: 0.0,
            hit_done_in_window: false,
            touched: false,
            chain_open: false,
            hits: 0,
            power_total: 0,
            dummy_flash: 0.0,
            last_event: "-".into(),
            last_ignored: "-".into(),
            speed_percent: 100,
            started: Instant::now(),
            log: open_log(),
        })
        .add_systems(Startup, (setup, spawn_model))
        .init_resource::<ShownPose>()
        .add_systems(Update, (read_input, show_state, (update_pose, pose_model, draw_skeleton).chain()))
        .add_systems(FixedUpdate, simulate);
    match load_skeleton(SORA) {
        Some(skeleton) => {
            app.insert_resource(skeleton);
            // Senza le pose un modello non serve: resterebbe fermo nella posa di riposo.
            match load_model(SORA) {
                Some(model) => {
                    app.insert_resource(SoraModel(model));
                    match (load_model(KEYBLADE), load_skeleton(KEYBLADE)) {
                        (Some(keyblade), Some(poses)) => {
                            app.insert_resource(KeybladeModel(keyblade));
                            app.insert_resource(KeybladePoses(poses));
                        }
                        _ => println!("Keyblade non trovato: Sora combatte a mani vuote"),
                    }
                }
                None => println!("modello non trovato: l'arena disegna Sora come omino di linee"),
            }
        }
        None => println!("scheletro non trovato: l'arena disegna solo la capsula"),
    }
    app.run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor_size = ARENA_HALF_SIZE * 2.0 * METERS_PER_UNIT;
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(floor_size, floor_size))),
        MeshMaterial3d(materials.add(Color::srgb(0.25, 0.3, 0.25))),
    ));
    commands.spawn((
        Player,
        Mesh3d(meshes.add(Capsule3d::new(PLAYER_RADIUS * METERS_PER_UNIT, 0.9))),
        MeshMaterial3d(materials.add(Color::srgb(0.6, 0.6, 0.6))),
        Transform::from_xyz(0.0, 0.75, 4.0),
    ));
    commands.spawn((
        Dummy,
        Mesh3d(meshes.add(Cylinder::new(DUMMY_RADIUS * METERS_PER_UNIT, 1.6))),
        MeshMaterial3d(materials.add(Color::srgb(0.7, 0.45, 0.2))),
        Transform::from_xyz(0.0, 0.8, 0.0),
    ));
    commands.spawn((
        SecondDummy,
        Mesh3d(meshes.add(Cylinder::new(DUMMY_RADIUS * METERS_PER_UNIT, 1.6))),
        MeshMaterial3d(materials.add(Color::srgb(0.45, 0.45, 0.7))),
        Transform::from_xyz(0.0, 0.8, 0.0),
        Visibility::Hidden,
    ));
    commands.spawn((
        DirectionalLight { illuminance: 8000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(4.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((Camera3d::default(), Transform::from_xyz(0.0, 9.0, 13.0).looking_at(Vec3::ZERO, Vec3::Y)));
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont { font_size: 16.0, ..default() },
        Node { position_type: PositionType::Absolute, top: Val::Px(10.0), left: Val::Px(10.0), ..default() },
    ));
}

/// Legge pad e tastiera a ogni fotogramma disegnato; il tick successivo consuma i comandi.
fn read_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut arena: ResMut<Arena>,
    mut exit: EventWriter<AppExit>,
    mut fixed_time: ResMut<Time<Fixed>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        exit.write(AppExit::Success);
    }
    // Rallentatore, per confrontare i movimenti con KH2: cambia solo quanti tick passano al secondo.
    for (key, percent) in [(KeyCode::Digit1, 100), (KeyCode::Digit2, 75), (KeyCode::Digit3, 50)] {
        if keyboard.just_pressed(key) {
            arena.speed_percent = percent;
            fixed_time.set_timestep_hz(f64::from(TICKS_PER_SECOND * percent) / 100.0);
            arena.write_log(&format!("VELOCITA {percent}"), 0.0);
        }
    }

    let mut direction = Vec2::ZERO;
    for (key, step) in [
        (KeyCode::KeyW, Vec2::new(0.0, -1.0)),
        (KeyCode::KeyS, Vec2::new(0.0, 1.0)),
        (KeyCode::KeyA, Vec2::new(-1.0, 0.0)),
        (KeyCode::KeyD, Vec2::new(1.0, 0.0)),
    ] {
        if keyboard.pressed(key) {
            direction += step;
        }
    }
    let mut attack = keyboard.just_pressed(KeyCode::Space) || keyboard.just_pressed(KeyCode::KeyJ);
    let mut jump = keyboard.just_pressed(KeyCode::KeyK);
    let mut jump_held = keyboard.pressed(KeyCode::KeyK);
    let mut strike = keyboard.just_pressed(KeyCode::KeyX);
    let mut special = keyboard.just_pressed(KeyCode::KeyE);
    let mut guard = keyboard.just_pressed(KeyCode::KeyQ);
    // Il manichino: frecce per spostarlo, Pag su / Pag giù per alzarlo e abbassarlo.
    let mut dummy = Vec3::ZERO;
    for (key, step) in [
        (KeyCode::ArrowUp, Vec3::new(0.0, 0.0, -1.0)),
        (KeyCode::ArrowDown, Vec3::new(0.0, 0.0, 1.0)),
        (KeyCode::ArrowLeft, Vec3::new(-1.0, 0.0, 0.0)),
        (KeyCode::ArrowRight, Vec3::new(1.0, 0.0, 0.0)),
        (KeyCode::PageUp, Vec3::new(0.0, 1.0, 0.0)),
        (KeyCode::PageDown, Vec3::new(0.0, -1.0, 0.0)),
    ] {
        if keyboard.pressed(key) {
            dummy += step;
        }
    }
    let mut menu_step = i32::from(keyboard.just_pressed(KeyCode::F2)) - i32::from(keyboard.just_pressed(KeyCode::F1));
    let mut menu_change = keyboard.just_pressed(KeyCode::F3);
    let mut second = keyboard.just_pressed(KeyCode::KeyN);
    let mut reset = keyboard.just_pressed(KeyCode::Home);

    for gamepad in &gamepads {
        let stick = gamepad.left_stick();
        if stick.length() > 0.2 {
            // In avanti sulla levetta = lontano dalla telecamera = z negativa.
            direction += Vec2::new(stick.x, -stick.y);
        }
        attack |= gamepad.just_pressed(GamepadButton::South);
        jump |= gamepad.just_pressed(GamepadButton::East);
        jump_held |= gamepad.pressed(GamepadButton::East);
        strike |= gamepad.just_pressed(GamepadButton::RightTrigger2) || gamepad.just_pressed(GamepadButton::RightThumb);
        special |= gamepad.just_pressed(GamepadButton::West);
        guard |= gamepad.just_pressed(GamepadButton::North);
        let right = gamepad.right_stick();
        if right.length() > 0.2 {
            dummy += Vec3::new(right.x, 0.0, -right.y);
        }
        dummy.y += f32::from(u8::from(gamepad.pressed(GamepadButton::RightTrigger)))
            - f32::from(u8::from(gamepad.pressed(GamepadButton::LeftTrigger)));
        menu_step += i32::from(gamepad.just_pressed(GamepadButton::DPadDown)) - i32::from(gamepad.just_pressed(GamepadButton::DPadUp));
        menu_change |= gamepad.just_pressed(GamepadButton::DPadRight);
        second |= gamepad.just_pressed(GamepadButton::DPadLeft);
        reset |= gamepad.just_pressed(GamepadButton::Select);
    }

    arena.move_input = direction.clamp_length_max(1.0);
    arena.attack_latched |= attack;
    arena.jump_latched |= jump;
    arena.jump_held = jump_held;
    arena.strike_latched |= strike;
    arena.special_latched |= special;
    arena.guard_latched |= guard;
    arena.dummy_input = dummy.clamp_length_max(1.0);
    if menu_step != 0 {
        let rows = arena.menu.len() as i32;
        arena.menu_cursor = (arena.menu_cursor as i32 + menu_step).rem_euclid(rows) as usize;
    }
    if menu_change {
        let ability = arena.menu[arena.menu_cursor].clone();
        change_ability(&mut arena.fighter.combo.loadout, &ability.1);
        let state = ability_state(&arena.fighter.combo.loadout, &ability.1);
        arena.write_log(&format!("ABILITA {} = {state}", ability.0), 0.0);
    }
    if second {
        arena.second_dummy = !arena.second_dummy;
    }
    if reset {
        arena.dummy_position = Vec2::ZERO;
        arena.dummy_height = 0.0;
    }
}

/// Un tick di gioco: 60 al secondo, sempre uguali.
fn simulate(mut arena: ResMut<Arena>) {
    let arena = &mut *arena;
    arena.tick += 1;

    // Il manichino si sposta con i suoi comandi.
    arena.dummy_position += Vec2::new(arena.dummy_input.x, arena.dummy_input.z) * DUMMY_SPEED;
    arena.dummy_position = arena.dummy_position.clamp(Vec2::splat(-ARENA_HALF_SIZE), Vec2::splat(ARENA_HALF_SIZE));
    arena.dummy_height = (arena.dummy_height + arena.dummy_input.y * DUMMY_LIFT_SPEED).clamp(0.0, DUMMY_MAX_HEIGHT);

    let to_dummy = arena.dummy_position - arena.player_position;
    let center_distance = to_dummy.length();
    let edge_distance = (center_distance - DUMMY_RADIUS).max(0.0);
    let toward_dummy = to_dummy.normalize_or_zero();
    let airborne = arena.fighter.airborne();

    // Il manichino colpisce: se Sora sta parando il colpo è fermato, se non può essere colpito
    // va a vuoto, altrimenti lo sbalza in aria.
    if std::mem::take(&mut arena.strike_latched) {
        let (what, events) = if arena.fighter.combo.guarding() {
            ("PARATO", arena.fighter.guarded())
        } else if arena.fighter.combo.unhittable() {
            ("A_VUOTO non colpibile", Vec::new())
        } else {
            ("SUBITO", arena.fighter.knocked(KNOCK_MOTION, KNOCK_RISE))
        };
        arena.last_event = format!("colpo del manichino: {what}");
        arena.write_log(&format!("COLPO_DEL_MANICHINO {what} {events:?}").replace(',', ";"), edge_distance);
    }

    // Il colpo tocca se in questo momento può fare danno e il manichino è a portata, davanti
    // e più o meno alla stessa altezza.
    let height_gap = (arena.dummy_height - arena.player_height).abs();
    let hit_landed = arena.fighter.combo.hit_power().is_some()
        && !arena.hit_done_in_window
        && center_distance <= HIT_REACH
        && height_gap <= HIT_VERTICAL_REACH
        && arena.player_facing.dot(toward_dummy) > 0.5;
    let hit_power = arena.fighter.combo.hit_power().unwrap_or(0);

    // Come il core vede i nemici: posizione del punto a cui mirare, raggio e taglia.
    let body = |position: Vec2, height: f32| Body {
        position: [position.x, height + DUMMY_AIM_HEIGHT, position.y],
        radius: DUMMY_RADIUS,
        size: DUMMY_SIZE,
    };
    let main_target = body(arena.dummy_position, arena.dummy_height);
    let mut nearby = vec![main_target];
    if arena.second_dummy {
        nearby.push(body(arena.dummy_position + SECOND_DUMMY_OFFSET, 0.0));
    }
    let scene = Scene {
        fighter: [arena.player_position.x, arena.player_height, arena.player_position.y],
        facing: [arena.player_facing.x, arena.player_facing.y],
        main_target: Some(main_target),
        nearby: &nearby,
        ledge_ahead: false,
    };
    // I comandi oltre ad Attacco: quale voce del repertorio richiamano.
    let loadout = &arena.fighter.combo.loadout;
    let finisher_next = arena.fighter.combo.step() + 2 >= loadout.limits().ground_steps;
    let command = if std::mem::take(&mut arena.guard_latched) {
        Some(GUARD)
    } else if std::mem::take(&mut arena.special_latched) {
        Some(if arena.fighter.combo.launched() {
            RETALIATING_SLASH
        } else if airborne {
            HORIZONTAL_SLASH
        } else if finisher_next && loadout.has(FINISHING_LEAP_ABILITY) {
            FINISHING_LEAP
        } else {
            UPPER_SLASH
        })
    } else {
        None
    };
    let berserk = loadout.berserk_charge;
    let events = arena.fighter.tick(Controls {
        attack_pressed: std::mem::take(&mut arena.attack_latched),
        command,
        jump_pressed: std::mem::take(&mut arena.jump_latched),
        jump_held: arena.jump_held,
        hit_landed,
        busy: false,
        berserk,
        floor: 0.0,
        scene,
    });
    arena.player_height = arena.fighter.height();

    if hit_landed {
        arena.hit_done_in_window = true;
        arena.touched = true;
        arena.hits += 1;
        arena.power_total += u32::from(hit_power);
        arena.dummy_flash = 1.0;
        arena.write_log(&format!("A_SEGNO potenza {hit_power}"), edge_distance);
    }
    for event in events {
        match event {
            Event::MoveStarted { .. } => {
                arena.advanced = 0.0;
                arena.touched = false;
                arena.chain_open = false;
                arena.hit_done_in_window = false;
            }
            Event::Lunge { .. } => {
                // Il colpo porta Sora in volo verso il bersaglio: di quanto sale e di quanto
                // avanza lo decide il core (vedi più sotto, `slide`).
                arena.player_facing = if toward_dummy != Vec2::ZERO { toward_dummy } else { arena.player_facing };
            }
            Event::ReactionStarted { .. } => {
                arena.chain_open = false;
                arena.hit_done_in_window = false;
            }
            Event::Jumped | Event::Landed => {}
            Event::HitWindowOpened { .. } => arena.hit_done_in_window = false,
            Event::ChainOpened => arena.chain_open = true,
            Event::ComboEnded => arena.chain_open = false,
            Event::PressIgnored(reason) => arena.last_ignored = format!("{reason:?}"),
            Event::HitWindowClosed => {}
        }
        arena.last_event = format!("{event:?}");
        arena.write_log(&format!("{event:?}").replace(',', ";"), edge_distance);
    }

    let flying_move = arena.fighter.combo.current_entry().is_some_and(|entry| entry.is_aerial() || entry.lunges());
    match arena.fighter.combo.current_move() {
        None => {
            let step = arena.move_input * RUN_SPEED;
            if step != Vec2::ZERO {
                arena.player_position += step;
                arena.player_facing = step.normalize();
            }
        }
        Some((_, frame)) => {
            if frame <= TURN_FRAMES && toward_dummy != Vec2::ZERO {
                arena.player_facing = toward_dummy;
            }
            let budget = arena.fighter.combo.current_entry().map_or(150.0, max_advance);
            // Solo i colpi a terra avanzano verso il bersaglio: una reazione (parata riuscita,
            // sbalzo...) no.
            let can_advance = arena.fighter.combo.current_entry().is_some()
                && !flying_move
                && arena.fighter.combo.hit_power().is_none()
                && !arena.touched
                && arena.advanced < budget
                && edge_distance > ADVANCE_STOP_DISTANCE
                && frame <= TURN_FRAMES * 2;
            if can_advance {
                let speed = ADVANCE_PER_TICK.max(budget / f32::from(TURN_FRAMES * 2));
                let step = speed.min(edge_distance - ADVANCE_STOP_DISTANCE);
                arena.player_position += toward_dummy * step;
                arena.advanced += step;
            }
        }
    }

    // Lo slancio orizzontale di un colpo "in volo": lo calcola il core, con i numeri di KH2.
    arena.player_position += Vec2::from(arena.fighter.slide());

    // Il giocatore non attraversa il manichino (se è alla sua altezza) e non esce dal pavimento.
    let minimum = DUMMY_RADIUS + PLAYER_RADIUS;
    let apart = arena.player_position - arena.dummy_position;
    if apart.length() < minimum && height_gap < 160.0 {
        arena.player_position = arena.dummy_position + apart.normalize_or(Vec2::Y) * minimum;
    }
    arena.player_position = arena.player_position.clamp(Vec2::splat(-ARENA_HALF_SIZE), Vec2::splat(ARENA_HALF_SIZE));
    arena.dummy_flash = (arena.dummy_flash - 0.08).max(0.0);
}

/// Nome e colore dello stato in cui si trova il giocatore.
fn state_of(arena: &Arena) -> (&'static str, Color) {
    match (arena.fighter.combo.current_move(), arena.fighter.combo.hit_power()) {
        (None, _) => ("libero", Color::srgb(0.6, 0.6, 0.6)),
        (Some(_), Some(_)) => ("PUO' FARE DANNO", Color::srgb(0.9, 0.15, 0.1)),
        (Some(_), None) if arena.chain_open && arena.touched => ("puoi proseguire", Color::srgb(0.1, 0.8, 0.2)),
        (Some(_), None) if arena.chain_open => ("finestra aperta, ma colpo a vuoto", Color::srgb(0.15, 0.3, 0.9)),
        (Some(_), None) => ("colpo in corso", Color::srgb(0.9, 0.8, 0.1)),
    }
}

/// Porta sullo schermo lo stato dell'arena: posizione, colori, scritte.
fn show_state(
    arena: Res<Arena>,
    mut player: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<Player>>,
    mut dummy: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), (With<Dummy>, Without<Player>)>,
    mut second: Query<(&mut Transform, &mut Visibility), (With<SecondDummy>, Without<Player>, Without<Dummy>)>,
    mut hud: Query<&mut Text, With<Hud>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (state_name, color) = state_of(&arena);

    if let Ok((mut transform, material)) = player.single_mut() {
        transform.translation.x = arena.player_position.x * METERS_PER_UNIT;
        transform.translation.z = arena.player_position.y * METERS_PER_UNIT;
        if let Some(material) = materials.get_mut(&material.0) {
            material.base_color = color;
        }
    }
    if let Ok((mut transform, mut visibility)) = second.single_mut() {
        let position = arena.dummy_position + SECOND_DUMMY_OFFSET;
        transform.translation = Vec3::new(position.x * METERS_PER_UNIT, 0.8, position.y * METERS_PER_UNIT);
        *visibility = if arena.second_dummy { Visibility::Visible } else { Visibility::Hidden };
    }
    if let Ok((mut transform, material)) = dummy.single_mut() {
        transform.translation = Vec3::new(
            arena.dummy_position.x * METERS_PER_UNIT,
            0.8 + arena.dummy_height * METERS_PER_UNIT,
            arena.dummy_position.y * METERS_PER_UNIT,
        );
        if let Some(material) = materials.get_mut(&material.0) {
            let flash = arena.dummy_flash;
            material.base_color = Color::srgb(0.7 + 0.3 * flash, 0.45 + 0.55 * flash, 0.2 + 0.8 * flash);
        }
    }
    if let Ok(mut text) = hud.single_mut() {
        let (move_name, frame) = match (arena.fighter.combo.current_move(), arena.fighter.combo.current_entry()) {
            (Some((motion, frame)), Some(entry)) => {
                let ability = arena.fighter.combo.catalog().abilities.get(&entry.ability).map_or(String::new(), |name| format!(" {name}"));
                (format!("animazione {motion} (voce {}{ability})", entry.id), frame)
            }
            _ => ("-".to_string(), 0),
        };
        let distance = ((arena.player_position - arena.dummy_position).length() - DUMMY_RADIUS).max(0.0);
        let limits = arena.fighter.combo.loadout.limits();
        let mut menu = String::new();
        for (row, (name, ability)) in arena.menu.iter().enumerate() {
            let cursor = if row == arena.menu_cursor { ">" } else { " " };
            menu += &format!("{cursor} {name}: {}\n", ability_state(&arena.fighter.combo.loadout, ability));
        }
        text.0 = format!(
            "Colpo: {move_name}   frame: {frame}   passo: {}   finali: {}\nStato: {state_name}\nUltimo evento: {}\n\
             Ultima pressione ignorata: {}\nColpi a segno: {}   potenza totale: {}\n\
             Distanza dal manichino: {distance:.0}   altezza di Sora: {:.0}   altezza del manichino: {:.0}\n\
             Limiti: {} colpi a terra, {} in aria, {} finali   Velocita': {}%  (tasti 1, 2, 3)\n\n\
             WASD / levetta: muovi   Spazio / Croce: attacca   K / Cerchio: salta\n\
             E / Quadrato: colpo speciale   Q / Triangolo: parata   X / R2: il manichino colpisce\n\
             Frecce / levetta destra: sposta il manichino   Pag su-giu' / grilletti: alzalo\n\
             N / croce sinistra: secondo manichino   Home: manichino al centro   Esc: esci\n\n\
             ABILITA' (F1 / F2 o croce su-giu': scegli   F3 o croce destra: cambia)\n{menu}",
            arena.fighter.combo.step(),
            arena.fighter.combo.finishers(),
            arena.last_event,
            arena.last_ignored,
            arena.hits,
            arena.power_total,
            arena.player_height,
            arena.dummy_height,
            limits.ground_steps,
            limits.air_steps,
            limits.finishers,
            arena.speed_percent,
        );
    }
}

/// Con lo scheletro la capsula diventa un disco sotto i piedi, che mostra il colore dello stato.
/// Se manca il modello, Sora viene disegnato come un omino fatto con le sue ossa.
fn draw_skeleton(
    arena: Res<Arena>,
    skeleton: Option<Res<Skeleton>>,
    model: Option<Res<SoraModel>>,
    shown: Res<ShownPose>,
    mut capsule: Query<&mut Transform, With<Player>>,
    mut gizmos: Gizmos,
) {
    let Some(skeleton) = skeleton else { return };
    if let Ok(mut transform) = capsule.single_mut() {
        transform.translation.y = 0.02;
        transform.scale = Vec3::new(1.6, 0.02, 1.6);
    }
    if model.is_some() {
        return;
    }
    if shown.pose.len() != skeleton.parents.len() {
        return;
    }
    let place = placement(&arena);
    let (_, color) = state_of(&arena);
    let position = |bone: usize| place.transform_point3(shown.pose[bone].w_axis.truncate());
    for (bone, parent) in skeleton.parents.iter().enumerate() {
        if let Ok(parent) = usize::try_from(*parent) {
            gizmos.line(position(parent), position(bone), color);
        }
    }
}
