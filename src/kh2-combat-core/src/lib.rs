//! Il combattimento di Sora in KH2 Final Mix (forma base), come libreria.
//!
//! La libreria non legge tasti, non disegna e non conosce i nemici: chi la usa le dice a
//! ogni tick cosa è successo (tasti, colpi a segno, bersagli) e lei risponde con ciò che fa
//! il personaggio. Tutto ciò che è specifico di Sora sta nel catalogo ([`catalog`]), non qui.
//!
//! - [`catalog`]: i dati (repertorio, animazioni, colpi a segno, abilità) e il loro file;
//! - [`rules`]: abilità, limiti della combo, scelta del colpo;
//! - [`combo`]: la combo e le reazioni, un tick alla volta;
//! - [`air`]: altezza (salto, balzi, caduta) e slancio orizzontale;
//! - [`fighter`]: il personaggio intero, cioè combo e altezza legate insieme.
//!
//! Unità: un tick è 1/60 di secondo; le distanze sono in unità di KH2; una posizione è
//! `[x, altezza, z]`; una direzione orizzontale è `[x, z]` di lunghezza 1; i frame delle
//! animazioni si contano da 1.

pub mod air;
pub mod catalog;
pub mod combo;
pub mod fighter;
pub mod rules;

/// Quante volte al secondo chi usa la libreria deve farla avanzare.
pub const TICKS_PER_SECOND: u32 = 60;

/// La versione del pacchetto (quella scritta in Cargo.toml), mai vuota.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
