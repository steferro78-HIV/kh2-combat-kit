//! I dati di un personaggio: repertorio, animazioni, colpi a segno, abilità.
//!
//! Qui non c'è nessuna regola del combattimento: solo la lettura del file catalogo
//! (INTERFACCIA.md, sezione 7) e le domande semplici sui dati (SPEC, sezioni 3 e 4).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Interruttori di una voce del repertorio (si sommano nel campo `flags`).
pub mod flag {
    /// Voce "in aria" (senza: "a terra").
    pub const AERIAL: u32 = 1;
    /// Il limite alto del dislivello cresce con la dote.
    pub const REACH_GROWS: u32 = 2;
    /// La voce è un finale.
    pub const FINISHER: u32 = 4;
    /// La taglia si toglie dal limite basso invece di aggiungerla.
    pub const LOW_INVERTED: u32 = 8;
    /// La taglia si aggiunge al limite alto invece di toglierla.
    pub const HIGH_INVERTED: u32 = 16;
}

/// Tipo di una voce: contro chi si valuta (SPEC 6.2).
pub mod kind {
    pub const BY_COMMAND: u8 = 0;
    pub const MAIN_TARGET: u8 = 1;
    pub const ANY_NEARBY: u8 = 2;
    pub const ALWAYS: u8 = 3;
}

/// Numeri degli eventi "a tratto" usati dalle regole (SPEC 3).
///
/// SPEC 13 (da NON riprodurre): gli altri eventi a tratto del catalogo (scie, "gira verso
/// l'aggancio", "niente gravità"...) si leggono e si conservano, ma non hanno regole.
pub mod trigger {
    /// A terra.
    pub const GROUNDED: u8 = 0;
    /// In aria.
    pub const AIRBORNE: u8 = 1;
    /// Ritorno alla posa di riposo.
    pub const BLEND_TO_IDLE: u8 = 2;
    /// Zona.
    pub const HITBOX: u8 = 10;
    /// Combo permessa.
    pub const COMBO_ALLOWED: u8 = 11;
    /// Comando di reazione permesso.
    pub const REACTION_ALLOWED: u8 = 14;
    /// Non colpibile.
    pub const UNHITTABLE: u8 = 27;
    /// Zona (secondo tipo, vale come il primo).
    pub const HITBOX_COMBO: u8 = 33;
    /// Finale permesso.
    pub const FINISHER_ALLOWED: u8 = 50;
}

/// Numeri di animazione con un significato fisso (SPEC 10).
pub mod motion {
    pub const JUMP: u16 = 3;
    pub const FALL: u16 = 4;
    pub const LAND: u16 = 5;
    pub const AERIAL_RECOVERY: u16 = 40;
    pub const DEFLECT: u16 = 41;
    pub const GUARD_SUCCESS: u16 = 42;
    pub const DEFLECT_AIR: u16 = 43;
}

/// Una voce del repertorio. I campi corrispondono uno a uno a quelli della riga `entry`
/// del file catalogo.
///
/// SPEC 13 (da NON riprodurre): `score`, `distance_adjust`, `blend` e `next_motion` si
/// leggono e si conservano per chi li vuole, ma nessuna regola della libreria li usa.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: u8,
    /// Nel file: `type`.
    pub kind: u8,
    /// Nel file: `sub`.
    pub alternative: i8,
    /// Nel file: `combo_offset`.
    pub step_advance: i8,
    /// Nel file: `flag`.
    pub flags: u32,
    pub motion: u16,
    pub next_motion: u16,
    pub jump: f32,
    pub jump_max: f32,
    pub jump_min: f32,
    pub speed_min: f32,
    pub speed_max: f32,
    pub near: f32,
    pub far: f32,
    pub low: f32,
    pub high: f32,
    pub inner_min: f32,
    pub inner_max: f32,
    pub blend: f32,
    /// Nel file: `dist_adjust`.
    pub distance_adjust: f32,
    pub ability: u16,
    pub score: u16,
}

impl Entry {
    /// Ha l'interruttore `AERIAL`.
    pub fn is_aerial(&self) -> bool {
        self.flags & flag::AERIAL != 0
    }

    /// Ha l'interruttore `FINISHER`.
    pub fn is_finisher(&self) -> bool {
        self.flags & flag::FINISHER != 0
    }

    /// È un colpo "in volo": il campo `jump` è maggiore di zero.
    pub fn lunges(&self) -> bool {
        self.jump > 0.0
    }
}

/// Un evento "a tratto".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: i16,
    /// Negativo: fino alla fine dell'animazione.
    pub end: i16,
    pub trigger: u8,
    pub params: Vec<i16>,
}

impl Span {
    /// Vero se l'evento vale al frame dato: start <= frame, e (end negativo oppure frame <= end).
    pub fn covers(&self, frame: u16) -> bool {
        let frame = i32::from(frame);
        i32::from(self.start) <= frame && (self.end < 0 || frame <= i32::from(self.end))
    }
}

/// Un evento "a istante".
///
/// SPEC 13 (da NON riprodurre): gli eventi a istante si conservano ma non hanno regole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Point {
    pub frame: i16,
    pub trigger: u8,
    pub params: Vec<i16>,
}

/// Un'animazione con i suoi eventi, nell'ordine in cui sono scritti nel file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Motion {
    pub id: u16,
    pub name: String,
    pub frames: u16,
    /// Eventi a tratto del personaggio.
    pub body: Vec<Span>,
    /// Eventi a tratto dell'arma.
    pub weapon: Vec<Span>,
    pub body_points: Vec<Point>,
    pub weapon_points: Vec<Point>,
}

impl Motion {
    /// Vero se un evento a tratto **del personaggio** con quel numero vale al frame dato.
    pub fn active(&self, trigger: u8, frame: u16) -> bool {
        self.body
            .iter()
            .any(|span| span.trigger == trigger && span.covers(frame))
    }

    /// Il più piccolo `start` (portato a zero se negativo) tra gli eventi a tratto del
    /// personaggio con quel numero; `None` se non ce ne sono.
    pub fn first_frame_of(&self, trigger: u8) -> Option<u16> {
        self.body
            .iter()
            .filter(|span| span.trigger == trigger)
            .map(|span| u16::try_from(span.start).unwrap_or(0))
            .min()
    }

    /// "Neutro" al frame dato: vale GROUNDED, AIRBORNE o BLEND_TO_IDLE (SPEC 5).
    pub fn neutral(&self, frame: u16) -> bool {
        self.active(trigger::GROUNDED, frame)
            || self.active(trigger::AIRBORNE, frame)
            || self.active(trigger::BLEND_TO_IDLE, frame)
    }

    /// Il numero del colpo a segno della **prima** zona che copre il frame dato: il primo
    /// parametro del primo evento HITBOX o HITBOX_COMBO che copre il frame, cercando prima
    /// tra quelli dell'arma e poi tra quelli del personaggio. `None` se non c'è una zona, se
    /// l'evento trovato non ha parametri, o se il suo primo parametro è negativo. È una
    /// domanda sui soli dati dell'animazione; quale zona conta per la combo quando più zone
    /// si sovrappongono lo decide la combo (INTERFACCIA, "Le zone attive").
    pub fn hitbox(&self, frame: u16) -> Option<u16> {
        let zone = self.zone_spans(frame).next()?;
        let first = *zone.params.first()?;
        // Un primo parametro negativo non può essere il numero di un colpo a segno (R8 c).
        u16::try_from(first).ok()
    }

    /// I numeri dei colpi a segno di **tutte** le zone che coprono il frame, nell'ordine di
    /// sempre: prima gli eventi dell'arma, poi quelli del personaggio, ciascuno nell'ordine
    /// del file. Gli eventi senza un primo numero, o con un primo numero negativo, non
    /// contano.
    pub(crate) fn zones(&self, frame: u16) -> impl Iterator<Item = u16> + '_ {
        self.zone_spans(frame)
            .filter_map(|zone| zone.params.first())
            .filter_map(|first| u16::try_from(*first).ok())
    }

    /// Gli eventi "zona" (dei due tipi) che coprono il frame, arma prima e personaggio poi.
    fn zone_spans(&self, frame: u16) -> impl Iterator<Item = &Span> + '_ {
        self.weapon.iter().chain(self.body.iter()).filter(move |span| {
            (span.trigger == trigger::HITBOX || span.trigger == trigger::HITBOX_COMBO)
                && span.covers(frame)
        })
    }
}

/// I numeri di un colpo a segno (riga `attack` del file catalogo).
///
/// SPEC 13 (da NON riprodurre): solo `power` e `attack_type` hanno una regola; gli altri
/// campi si conservano per chi usa la libreria.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Attack {
    pub id: u16,
    /// Nel file: `sub_id`.
    pub sub: u16,
    pub power: u16,
    /// Nel file: `type`.
    pub attack_type: u8,
    pub element: u8,
    pub reaction: u8,
    pub knock1: i16,
    pub knock2: i16,
    pub interval: u16,
    pub revenge: u8,
    pub combo_group: u8,
    pub kind: u8,
}

impl Attack {
    /// La zona è una parata: il tipo è 2 (parata) o 3 (parata totale).
    pub fn guards(&self) -> bool {
        self.attack_type == 2 || self.attack_type == 3
    }
}

/// Tutto il catalogo di un personaggio.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    /// Nell'ordine del file: è l'ordine di priorità.
    pub entries: Vec<Entry>,
    /// Per numero di animazione.
    pub motions: BTreeMap<u16, Motion>,
    /// Per numero; le righe con lo stesso numero, nell'ordine del file.
    pub attacks: BTreeMap<u16, Vec<Attack>>,
    /// Numero -> nome.
    pub abilities: BTreeMap<u16, String>,
}

impl Catalog {
    /// `<cartella del pacchetto>/../../extracted/kh2/catalog/sora_base.catalog`, dove la
    /// cartella del pacchetto è quella nota al momento della compilazione.
    pub fn default_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("extracted")
            .join("kh2")
            .join("catalog")
            .join("sora_base.catalog")
    }

    /// Legge il file e lo passa a `parse`. Se il file non si legge, l'errore contiene il percorso.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| {
            format!("non riesco a leggere il catalogo {}: {error}", path.display())
        })?;
        Self::parse(&text)
    }

    /// Legge il testo di un catalogo (INTERFACCIA.md, sezione 7).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut catalog = Catalog::default();
        for (index, raw) in text.lines().enumerate() {
            let Some(line) = Line::split(index + 1, raw) else {
                continue;
            };
            match line.keyword {
                "entry" => catalog.entries.push(read_entry(&line)?),
                "motion" => {
                    let id: u16 = line.number("id")?;
                    // Una seconda riga con lo stesso numero aggiorna nome e durata e
                    // conserva gli eventi già letti.
                    let motion = catalog.motions.entry(id).or_default();
                    motion.id = id;
                    // Il nome è solo informativo: può mancare, e allora resta vuoto (R1).
                    motion.name = line.text("name").unwrap_or("").to_string();
                    motion.frames = line.number("frames")?;
                }
                "span" => {
                    let span = Span {
                        start: line.number("start")?,
                        end: line.number("end")?,
                        trigger: line.number("trigger")?,
                        params: line.params(),
                    };
                    let weapon = line.is_weapon();
                    let motion = line.motion_of(&mut catalog)?;
                    if weapon {
                        motion.weapon.push(span);
                    } else {
                        motion.body.push(span);
                    }
                }
                "point" => {
                    let point = Point {
                        frame: line.number("frame")?,
                        trigger: line.number("trigger")?,
                        params: line.params(),
                    };
                    let weapon = line.is_weapon();
                    let motion = line.motion_of(&mut catalog)?;
                    if weapon {
                        motion.weapon_points.push(point);
                    } else {
                        motion.body_points.push(point);
                    }
                }
                "attack" => {
                    let attack = read_attack(&line)?;
                    catalog.attacks.entry(attack.id).or_default().push(attack);
                }
                "ability" => {
                    let id: u16 = line.number("id")?;
                    let name = line.required("name")?.to_string();
                    catalog.abilities.insert(id, name);
                }
                // SPEC 13 (da NON riprodurre): le righe con una parola chiave sconosciuta
                // (per esempio `kh2-catalog` e `source`) si ignorano senza errore.
                _ => {}
            }
        }
        if catalog.entries.is_empty() {
            return Err("il catalogo non contiene nemmeno una riga `entry`".to_string());
        }
        Ok(catalog)
    }

    /// Posizione in `entries` della **prima** voce con quel numero.
    pub fn index_of_id(&self, id: u8) -> Option<usize> {
        self.entries.iter().position(|entry| entry.id == id)
    }

    pub fn motion(&self, id: u16) -> Option<&Motion> {
        self.motions.get(&id)
    }

    /// La riga del primo contatto di quel colpo a segno: quella con `sub` zero; se non c'è,
    /// la prima con quel numero.
    pub fn attack(&self, id: u16) -> Option<&Attack> {
        let rows = self.attacks.get(&id)?;
        rows.iter().find(|row| row.sub == 0).or_else(|| rows.first())
    }

    /// Il numero dell'abilità con quel nome (per esempio "Slide_Dash").
    pub fn ability_number(&self, name: &str) -> Option<u16> {
        self.abilities
            .iter()
            .find(|(_, known)| known.as_str() == name)
            .map(|(number, _)| *number)
    }
}

/// Una riga del file già divisa: il suo numero (da 1), la parola chiave e le coppie
/// `nome=valore`. I pezzi senza `=` sono già stati scartati.
struct Line<'a> {
    number: usize,
    keyword: &'a str,
    pairs: Vec<(&'a str, &'a str)>,
}

impl<'a> Line<'a> {
    /// `None` per una riga vuota.
    fn split(number: usize, raw: &'a str) -> Option<Self> {
        let mut pieces = raw.split_whitespace();
        let keyword = pieces.next()?;
        let pairs = pieces.filter_map(|piece| piece.split_once('=')).collect();
        Some(Line {
            number,
            keyword,
            pairs,
        })
    }

    /// Il valore di un campo, se c'è.
    fn text(&self, name: &str) -> Option<&'a str> {
        self.pairs
            .iter()
            .find(|(field, _)| *field == name)
            .map(|(_, value)| *value)
    }

    /// Il valore di un campo richiesto.
    fn required(&self, name: &str) -> Result<&'a str, String> {
        self.text(name).ok_or_else(|| {
            format!(
                "riga {}: nella riga `{}` manca il campo `{name}`",
                self.number, self.keyword
            )
        })
    }

    /// Un campo richiesto, letto come numero del tipo voluto.
    fn number<T: FromStr>(&self, name: &str) -> Result<T, String> {
        let value = self.required(name)?;
        value.parse().map_err(|_| self.wrong(name, value))
    }

    /// Un campo richiesto, letto come numero decimale (finito).
    fn decimal(&self, name: &str) -> Result<f32, String> {
        let value = self.required(name)?;
        match value.parse::<f32>() {
            Ok(number) if number.is_finite() => Ok(number),
            _ => Err(self.wrong(name, value)),
        }
    }

    fn wrong(&self, name: &str, value: &str) -> String {
        format!(
            "riga {}: nella riga `{}` il campo `{name}` ha un valore non valido: `{value}`",
            self.number, self.keyword
        )
    }

    /// I numeri del campo `params`, separati da `;`. I pezzi che non sono numeri si saltano.
    /// Il campo può mancare: allora vale come "nessun numero" (R1).
    fn params(&self) -> Vec<i16> {
        self.text("params")
            .unwrap_or("")
            .split(';')
            .filter_map(|piece| piece.parse().ok())
            .collect()
    }

    /// `who=weapon`: evento dell'arma. Qualsiasi altro valore, o assente: del personaggio.
    fn is_weapon(&self) -> bool {
        self.text("who") == Some("weapon")
    }

    /// L'animazione a cui appartiene una riga `span` o `point`: deve essere già stata letta.
    fn motion_of<'c>(&self, catalog: &'c mut Catalog) -> Result<&'c mut Motion, String> {
        let id: u16 = self.number("motion")?;
        catalog.motions.get_mut(&id).ok_or_else(|| {
            format!(
                "riga {}: la riga `{}` nomina l'animazione {id}, \
                 ma la sua riga `motion` non è ancora arrivata",
                self.number, self.keyword
            )
        })
    }
}

fn read_entry(line: &Line) -> Result<Entry, String> {
    Ok(Entry {
        id: line.number("id")?,
        // Un tipo oltre 3 si legge lo stesso: quella voce non è mai candidata nella scelta,
        // ma un comando la può richiamare come ogni altra (R2).
        kind: line.number("type")?,
        alternative: line.number("sub")?,
        step_advance: line.number("combo_offset")?,
        flags: line.number("flag")?,
        motion: line.number("motion")?,
        next_motion: line.number("next_motion")?,
        jump: line.decimal("jump")?,
        jump_max: line.decimal("jump_max")?,
        jump_min: line.decimal("jump_min")?,
        speed_min: line.decimal("speed_min")?,
        speed_max: line.decimal("speed_max")?,
        near: line.decimal("near")?,
        far: line.decimal("far")?,
        low: line.decimal("low")?,
        high: line.decimal("high")?,
        inner_min: line.decimal("inner_min")?,
        inner_max: line.decimal("inner_max")?,
        blend: line.decimal("blend")?,
        distance_adjust: line.decimal("dist_adjust")?,
        ability: line.number("ability")?,
        score: line.number("score")?,
    })
}

fn read_attack(line: &Line) -> Result<Attack, String> {
    Ok(Attack {
        id: line.number("id")?,
        sub: line.number("sub_id")?,
        power: line.number("power")?,
        attack_type: line.number("type")?,
        element: line.number("element")?,
        reaction: line.number("reaction")?,
        knock1: line.number("knock1")?,
        knock2: line.number("knock2")?,
        interval: line.number("interval")?,
        revenge: line.number("revenge")?,
        combo_group: line.number("combo_group")?,
        kind: line.number("kind")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = "\
kh2-catalog 1
source character=PROVA weapon=NESSUNA

entry id=5 type=3 sub=6 combo_offset=0 flag=0 motion=10 next_motion=0 jump=0.0 jump_max=0.0 jump_min=0.0 speed_min=0.0 speed_max=0.0 near=0.0 far=0.0 low=0.0 high=0.0 inner_min=1.0 inner_max=1.0 blend=0.0 dist_adjust=0.0 ability=0 score=1
entry id=9 type=3 sub=-1 combo_offset=1 flag=5 motion=20 next_motion=4 jump=21.0 jump_max=71.0 jump_min=21.0 speed_min=-8.0 speed_max=8.0 near=0.0 far=0.0 low=0.0 high=0.0 inner_min=1.0 inner_max=1.0 blend=0.0 dist_adjust=85.0 ability=77 score=1
entry id=5 type=0 sub=-1 combo_offset=0 flag=0 motion=30 next_motion=0 jump=0.0 jump_max=0.0 jump_min=0.0 speed_min=0.0 speed_max=0.0 near=0.0 far=0.0 low=0.0 high=0.0 inner_min=1.0 inner_max=1.0 blend=0.0 dist_adjust=0.0 ability=0 score=1
motion id=10 name=PRIMO frames=48
span motion=10 who=body start=0 end=12 trigger=28 params=15
span motion=10 who=body start=23 end=-1 trigger=11 params=
span motion=10 start=40 end=-1 trigger=2
point motion=10 who=body frame=20 trigger=19 params=12
span motion=10 who=weapon start=15 end=23 trigger=10 params=126;1
span motion=10 who=body start=10 end=30 trigger=10 params=7
point motion=10 who=weapon frame=1 trigger=8 params=500;x;35;;0
attack sub_id=1 id=126 type=0 crit=0 power=25 team=0 element=0 reaction=3 knock1=75 knock2=0 flags=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=0 id=126 type=0 crit=0 power=100 team=0 element=0 reaction=3 knock1=125 knock2=-1 flags=1 interval=0 revenge=8 combo_group=0 kind=0
attack sub_id=2 id=7 type=2 power=0 element=0 reaction=0 knock1=32767 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
attack sub_id=3 id=7 type=3 power=0 element=0 reaction=0 knock1=32767 knock2=0 interval=0 revenge=0 combo_group=0 kind=0
ability id=77 name=Aerial_Prova
ability id=78 name=Altra
";

    fn small() -> Catalog {
        Catalog::parse(SMALL).expect("il catalogo di prova si deve leggere")
    }

    #[test]
    fn legge_voci_nell_ordine_del_file() {
        let catalog = small();
        assert_eq!(catalog.entries.len(), 3);
        let first = &catalog.entries[0];
        assert_eq!((first.id, first.kind, first.alternative), (5, 3, 6));
        assert_eq!((first.motion, first.next_motion), (10, 0));
        let second = &catalog.entries[1];
        assert_eq!((second.id, second.step_advance, second.flags), (9, 1, 5));
        assert_eq!((second.jump, second.jump_min, second.jump_max), (21.0, 21.0, 71.0));
        assert_eq!((second.speed_min, second.speed_max), (-8.0, 8.0));
        assert_eq!((second.distance_adjust, second.ability, second.score), (85.0, 77, 1));
        assert!(second.is_aerial() && second.is_finisher() && second.lunges());
        assert!(!first.is_aerial() && !first.is_finisher() && !first.lunges());
    }

    #[test]
    fn index_of_id_trova_la_prima_voce_con_quel_numero() {
        let catalog = small();
        assert_eq!(catalog.index_of_id(5), Some(0));
        assert_eq!(catalog.index_of_id(9), Some(1));
        assert_eq!(catalog.index_of_id(6), None);
    }

    #[test]
    fn eventi_divisi_tra_personaggio_e_arma() {
        let catalog = small();
        let motion = catalog.motion(10).unwrap();
        assert_eq!((motion.id, motion.name.as_str(), motion.frames), (10, "PRIMO", 48));
        assert_eq!(motion.body.len(), 4);
        assert_eq!(motion.weapon.len(), 1);
        assert_eq!(motion.body_points.len(), 1);
        assert_eq!(motion.weapon_points.len(), 1);
        // `who` assente e `params` assente: evento del personaggio, senza numeri.
        assert_eq!(motion.body[2], Span { start: 40, end: -1, trigger: 2, params: vec![] });
        // `params=` vuoto.
        assert!(motion.body[1].params.is_empty());
        // I pezzi che non sono numeri si saltano.
        assert_eq!(motion.weapon_points[0].params, vec![500, 35, 0]);
        assert_eq!(motion.weapon[0].params, vec![126, 1]);
    }

    #[test]
    fn un_tratto_copre_i_frame_dal_primo_all_ultimo_compresi() {
        let span = Span { start: 15, end: 23, trigger: 10, params: vec![] };
        assert!(!span.covers(14));
        assert!(span.covers(15));
        assert!(span.covers(23));
        assert!(!span.covers(24));
        let open = Span { start: 23, end: -1, trigger: 11, params: vec![] };
        assert!(!open.covers(22));
        assert!(open.covers(23));
        assert!(open.covers(u16::MAX));
        let from_zero = Span { start: 0, end: -1, trigger: 0, params: vec![] };
        assert!(from_zero.covers(1));
    }

    #[test]
    fn domande_sull_animazione() {
        let catalog = small();
        let motion = catalog.motion(10).unwrap();
        assert!(!motion.active(trigger::COMBO_ALLOWED, 22));
        assert!(motion.active(trigger::COMBO_ALLOWED, 23));
        // `active` guarda solo gli eventi del personaggio: la zona dell'arma non conta.
        assert!(!motion.active(trigger::HITBOX, 5));
        assert!(motion.active(trigger::HITBOX, 10));
        assert_eq!(motion.first_frame_of(trigger::COMBO_ALLOWED), Some(23));
        assert_eq!(motion.first_frame_of(trigger::FINISHER_ALLOWED), None);
        assert!(!motion.neutral(39));
        assert!(motion.neutral(40));
    }

    #[test]
    fn first_frame_of_prende_il_piu_piccolo_e_non_va_sotto_zero() {
        let motion = Motion {
            body: vec![
                Span { start: 30, end: 40, trigger: 11, params: vec![] },
                Span { start: 12, end: 20, trigger: 11, params: vec![] },
                Span { start: -3, end: 5, trigger: 50, params: vec![] },
            ],
            ..Motion::default()
        };
        assert_eq!(motion.first_frame_of(11), Some(12));
        assert_eq!(motion.first_frame_of(50), Some(0));
    }

    #[test]
    fn la_zona_si_cerca_prima_sull_arma_e_poi_sul_personaggio() {
        let catalog = small();
        let motion = catalog.motion(10).unwrap();
        assert_eq!(motion.hitbox(9), None);
        assert_eq!(motion.hitbox(10), Some(7)); // solo il personaggio
        assert_eq!(motion.hitbox(15), Some(126)); // tutti e due: vince l'arma
        assert_eq!(motion.hitbox(23), Some(126));
        assert_eq!(motion.hitbox(24), Some(7));
        assert_eq!(motion.hitbox(31), None);
    }

    #[test]
    fn una_zona_senza_parametri_non_da_un_numero() {
        let motion = Motion {
            weapon: vec![Span { start: 1, end: 9, trigger: trigger::HITBOX_COMBO, params: vec![] }],
            body: vec![Span { start: 1, end: 9, trigger: trigger::HITBOX, params: vec![44] }],
            ..Motion::default()
        };
        // L'evento trovato è quello dell'arma, senza parametri: non si prosegue oltre.
        assert_eq!(motion.hitbox(5), None);
        let second_kind = Motion {
            body: vec![Span { start: 1, end: 9, trigger: trigger::HITBOX_COMBO, params: vec![44, 2] }],
            ..Motion::default()
        };
        assert_eq!(second_kind.hitbox(5), Some(44));
    }

    #[test]
    fn colpi_a_segno() {
        let catalog = small();
        assert_eq!(catalog.attacks[&126].len(), 2);
        // Le righe restano nell'ordine del file; `attack` dà quella con sub zero.
        assert_eq!(catalog.attacks[&126][0].sub, 1);
        let first = catalog.attack(126).unwrap();
        assert_eq!((first.sub, first.power, first.knock1, first.knock2, first.revenge), (0, 100, 125, -1, 8));
        assert!(!first.guards());
        // Senza una riga con sub zero: la prima con quel numero.
        let guard = catalog.attack(7).unwrap();
        assert_eq!(guard.sub, 2);
        assert!(guard.guards());
        assert!(catalog.attacks[&7][1].guards());
        assert!(catalog.attack(999).is_none());
    }

    #[test]
    fn tipi_di_parata() {
        let of_type = |attack_type| Attack { attack_type, ..Attack::default() };
        assert!(!of_type(0).guards());
        assert!(!of_type(1).guards());
        assert!(of_type(2).guards());
        assert!(of_type(3).guards());
        assert!(!of_type(4).guards());
    }

    #[test]
    fn abilita() {
        let catalog = small();
        assert_eq!(catalog.ability_number("Aerial_Prova"), Some(77));
        assert_eq!(catalog.ability_number("Altra"), Some(78));
        assert_eq!(catalog.ability_number("Sconosciuta"), None);
        assert_eq!(catalog.abilities[&77], "Aerial_Prova");
    }

    #[test]
    fn una_seconda_riga_motion_aggiorna_nome_e_durata_e_conserva_gli_eventi() {
        let text = format!("{SMALL}motion id=10 name=SECONDO frames=50\n");
        let catalog = Catalog::parse(&text).unwrap();
        let motion = catalog.motion(10).unwrap();
        assert_eq!((motion.name.as_str(), motion.frames), ("SECONDO", 50));
        assert_eq!(motion.body.len(), 4);
        assert_eq!(motion.weapon.len(), 1);
    }

    #[test]
    fn pezzi_senza_uguale_e_campi_in_piu_si_ignorano() {
        let text = format!("{SMALL}motion id=11 questo-non-conta name=X frames=9 inventato=3\n");
        let catalog = Catalog::parse(&text).unwrap();
        assert_eq!(catalog.motion(11).unwrap().frames, 9);
    }

    #[test]
    fn un_campo_mancante_e_un_errore_con_il_numero_della_riga() {
        let text = format!("{SMALL}motion id=12 name=X\n");
        let error = Catalog::parse(&text).unwrap_err();
        let line = SMALL.lines().count() + 1;
        assert!(error.contains(&line.to_string()), "{error}");
        assert!(error.contains("frames"), "{error}");
    }

    #[test]
    fn un_valore_del_tipo_sbagliato_e_un_errore_con_il_numero_della_riga() {
        for bad in [
            "motion id=12 name=X frames=tanti",
            "motion id=-1 name=X frames=10",
            "motion id=70000 name=X frames=10",
            "span motion=10 start=1.5 end=3 trigger=11 params=",
            "span motion=10 start=1 end=3 trigger=256 params=",
            "ability id=x name=Y",
            "ability id=3",
        ] {
            let text = format!("{SMALL}{bad}\n");
            let error = Catalog::parse(&text).unwrap_err();
            let line = SMALL.lines().count() + 1;
            assert!(error.contains(&line.to_string()), "{bad}: {error}");
        }
    }

    #[test]
    fn una_voce_con_un_numero_fuori_posto_e_un_errore() {
        let good = SMALL.lines().nth(3).unwrap();
        assert!(good.starts_with("entry id=5 type=3"));
        for (from, to) in [
            ("id=5", "id=256"),
            ("sub=6", "sub=sei"),
            ("jump=0.0", "jump=alto"),
            ("far=0.0", "far=NaN"),
            ("speed_max=0.0", "speed_max=inf"),
            ("type=3", "type=256"),
            ("type=3", "type=-1"),
            (" score=1", ""),
        ] {
            let bad = good.replacen(from, to, 1);
            let error = Catalog::parse(&bad).unwrap_err();
            assert!(error.contains("riga 1"), "{from} -> {to}: {error}");
        }
    }

    #[test]
    fn un_tipo_di_voce_oltre_tre_si_legge_lo_stesso() {
        let good = SMALL.lines().nth(3).unwrap();
        let odd = good.replacen("type=3", "type=4", 1);
        let catalog = Catalog::parse(&odd).unwrap();
        assert_eq!(catalog.entries[0].kind, 4);
        let odd = good.replacen("type=3", "type=255", 1);
        assert_eq!(Catalog::parse(&odd).unwrap().entries[0].kind, 255);
    }

    #[test]
    fn solo_who_params_e_il_nome_dell_animazione_possono_mancare() {
        let entry = SMALL.lines().nth(3).unwrap();
        let text = format!(
            "{entry}\nmotion id=10 frames=48\nspan motion=10 start=1 end=5 trigger=11\npoint motion=10 frame=3 trigger=19\n"
        );
        let catalog = Catalog::parse(&text).unwrap();
        let motion = catalog.motion(10).unwrap();
        assert_eq!(motion.name, "");
        assert_eq!(motion.body, [Span { start: 1, end: 5, trigger: 11, params: vec![] }]);
        assert_eq!(motion.body_points, [Point { frame: 3, trigger: 19, params: vec![] }]);
        // Tutti gli altri campi elencati sono richiesti, anche quelli di `attack` che
        // nessuna regola usa.
        for missing in [" element=0", " reaction=3", " knock2=-1", " kind=0", " power=100"] {
            let attack = SMALL.lines().find(|line| line.starts_with("attack sub_id=0")).unwrap();
            assert!(attack.contains(missing), "{missing}");
            let text = format!("{entry}\n{}\n", attack.replacen(missing, "", 1));
            let error = Catalog::parse(&text).unwrap_err();
            assert!(error.contains("riga 2"), "{missing}: {error}");
        }
    }

    #[test]
    fn tutte_le_zone_che_coprono_un_frame_nell_ordine_di_sempre() {
        let catalog = small();
        let motion = catalog.motion(10).unwrap();
        // Frame 15-23: la zona dell'arma (126) e quella del personaggio (7), in quest'ordine.
        assert_eq!(motion.zones(15).collect::<Vec<u16>>(), [126, 7]);
        assert_eq!(motion.zones(12).collect::<Vec<u16>>(), [7]);
        assert!(motion.zones(31).next().is_none());
        // Senza un primo numero, o con un primo numero negativo, la zona non conta.
        let odd = Motion {
            weapon: vec![
                Span { start: 1, end: 9, trigger: trigger::HITBOX, params: vec![] },
                Span { start: 1, end: 9, trigger: trigger::HITBOX, params: vec![-5] },
                Span { start: 1, end: 9, trigger: trigger::HITBOX_COMBO, params: vec![44, 2] },
            ],
            ..Motion::default()
        };
        assert_eq!(odd.zones(5).collect::<Vec<u16>>(), [44]);
        assert_eq!(odd.hitbox(5), None);
        let negative_first = Motion { weapon: odd.weapon[1..].to_vec(), ..Motion::default() };
        assert_eq!(negative_first.hitbox(5), None);
    }

    #[test]
    fn eventi_prima_della_loro_animazione_sono_un_errore() {
        let entry = SMALL.lines().nth(3).unwrap();
        let text = format!("{entry}\nspan motion=10 who=body start=1 end=2 trigger=11 params=\nmotion id=10 name=X frames=5\n");
        let error = Catalog::parse(&text).unwrap_err();
        assert!(error.contains("riga 2"), "{error}");
        let text = format!("{entry}\npoint motion=10 who=body frame=1 trigger=11 params=\n");
        assert!(Catalog::parse(&text).unwrap_err().contains("riga 2"));
    }

    #[test]
    fn senza_nemmeno_una_voce_la_lettura_fallisce() {
        assert!(Catalog::parse("").is_err());
        assert!(Catalog::parse("kh2-catalog 1\nmotion id=1 name=A frames=10\n").is_err());
    }

    #[test]
    fn il_percorso_predefinito_punta_al_catalogo_estratto() {
        let path = Catalog::default_path();
        assert!(path.ends_with(Path::new("extracted").join("kh2").join("catalog").join("sora_base.catalog")));
        assert!(path.starts_with(env!("CARGO_MANIFEST_DIR")));
    }

    #[test]
    fn se_il_file_non_si_legge_l_errore_contiene_il_percorso() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("questo-file-non-esiste.catalog");
        let error = Catalog::load(&path).unwrap_err();
        assert!(error.contains("questo-file-non-esiste.catalog"), "{error}");
    }
}
