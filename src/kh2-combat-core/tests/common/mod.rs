//! Cose in comune tra i test che usano i dati veri: dove cercarli e come leggere una
//! registrazione (INTERFACCIA.md, sezioni 1 e 8).
//!
//! I dati veri non stanno in Git. Se mancano, i test che li usano si saltano da soli
//! stampando una riga che lo dice.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use kh2_combat_core::catalog::Catalog;

/// La cartella del pacchetto, nota al momento della compilazione.
fn package() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Il catalogo vero: prima in `Catalog::default_path()`, poi in
/// `<pacchetto>\..\..\data\catalog\sora_base.catalog`. `None` (con una riga stampata) se
/// non c'è in nessuno dei due posti.
pub fn real_catalog() -> Option<Catalog> {
    let places = [
        Catalog::default_path(),
        package()
            .join("..")
            .join("..")
            .join("data")
            .join("catalog")
            .join("sora_base.catalog"),
    ];
    for place in &places {
        if place.is_file() {
            return Some(Catalog::load(place).expect("il catalogo vero si deve leggere"));
        }
    }
    println!("SALTATO: il catalogo vero non c'è (cercato in {places:?})");
    None
}

/// Le registrazioni: i file `kh2_telemetry_*.csv` in `<pacchetto>\..\..\logs\`, oppure in
/// `<pacchetto>\..\..\data\logs\`, in ordine di nome. Vuoto (con una riga stampata) se non
/// ce ne sono.
pub fn recordings() -> Vec<Log> {
    let root = package().join("..").join("..");
    let places = [root.join("logs"), root.join("data").join("logs")];
    for place in &places {
        let mut files: Vec<PathBuf> = std::fs::read_dir(place)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
                name.starts_with("kh2_telemetry_") && name.ends_with(".csv")
            })
            .collect();
        if !files.is_empty() {
            files.sort();
            return files.iter().map(|file| Log::load(file)).collect();
        }
    }
    println!("SALTATO: le registrazioni non ci sono (cercate in {places:?})");
    Vec::new()
}

/// Una registrazione, un fotogramma per tick.
pub struct Log {
    /// Il nome del file.
    pub name: String,
    columns: Vec<String>,
    /// Le righe del file, già lette come numeri.
    rows: Vec<Vec<f64>>,
    /// Per ogni fotogramma, anche quelli che il file salta: la riga del file che vale.
    ticks: Vec<usize>,
}

/// Una colonna di una registrazione.
#[derive(Clone, Copy)]
pub struct Column(usize);

impl Log {
    pub fn load(path: &Path) -> Log {
        let text = std::fs::read_to_string(path).expect("la registrazione si deve leggere");
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();
        Log::parse(&name, &text)
    }

    pub fn parse(name: &str, text: &str) -> Log {
        // Le righe che cominciano con `#` sono commenti; la prima che non lo è contiene i
        // nomi delle colonne.
        let mut lines = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty());
        let columns: Vec<String> = lines
            .next()
            .unwrap_or("")
            .split(',')
            .map(|column| column.trim().to_string())
            .collect();
        let frame = columns
            .iter()
            .position(|column| column == "frame")
            .expect("ogni registrazione ha la colonna frame");

        let mut rows: Vec<Vec<f64>> = Vec::new();
        let mut ticks = Vec::new();
        for line in lines {
            let row: Vec<f64> = line
                .split(',')
                .enumerate()
                .map(|(index, field)| {
                    let column = columns.get(index).map_or("", String::as_str);
                    read_field(column, field.trim())
                })
                .collect();
            // Se il numero del fotogramma salta, per ogni numero mancante vale una riga
            // uguale alla precedente.
            if let Some(previous) = rows.last() {
                let missing = row[frame] - previous[frame] - 1.0;
                for _ in 0..missing.max(0.0) as usize {
                    ticks.push(rows.len() - 1);
                }
            }
            ticks.push(rows.len());
            rows.push(row);
        }
        Log {
            name: name.to_string(),
            columns,
            rows,
            ticks,
        }
    }

    /// Quanti fotogrammi copre la registrazione.
    pub fn len(&self) -> usize {
        self.ticks.len()
    }

    /// La colonna con quel nome, se il file ce l'ha.
    pub fn column(&self, name: &str) -> Option<Column> {
        self.columns.iter().position(|column| column == name).map(Column)
    }

    /// Vero se il file ha tutte le colonne nominate.
    pub fn has(&self, names: &[&str]) -> bool {
        names.iter().all(|name| self.column(name).is_some())
    }

    /// Il valore di una colonna in un fotogramma.
    pub fn at(&self, tick: usize, column: Column) -> f64 {
        self.rows[self.ticks[tick]]
            .get(column.0)
            .copied()
            .unwrap_or(f64::NAN)
    }
}

/// Un campo di una riga, come numero. `input` è esadecimale; `tgt` è un identificativo
/// (esadecimale) oppure `-1`: qui conta solo se è maggiore di zero.
fn read_field(column: &str, field: &str) -> f64 {
    match column {
        "input" => u32::from_str_radix(field, 16).map_or(f64::NAN, f64::from),
        "tgt" => {
            if field.starts_with('-') {
                -1.0
            } else {
                u64::from_str_radix(field, 16).map_or(-1.0, |id| id as f64)
            }
        }
        _ => field.parse().unwrap_or(f64::NAN),
    }
}

/// Attacco è giù: (valore AND 08000004) è uguale a 08000004.
pub fn attack_down(input: f64) -> bool {
    (input as u32) & 0x0800_0004 == 0x0800_0004
}

/// Il tasto del salto è giù: il bit 100 è acceso.
pub fn jump_down(input: f64) -> bool {
    (input as u32) & 0x100 != 0
}
