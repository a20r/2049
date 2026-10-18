//! Saved-game shapes. The JSON matches the earlier JavaScript build key for key,
//! so saves made before the Rust port keep loading.

use nexus_engine::collatz::{Cells, CELLS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const CLASSIC_KEY: &str = "nexus2049.state";
pub const CLASSIC_BEST_KEY: &str = "nexus2049.best";
pub const MODE_KEY: &str = "nexus2049.mode";
pub const CZ_KEY: &str = "nexus2049.collatz.state";
pub const CZ_BEST_KEY: &str = "nexus2049.collatz.best";
pub const CZ_INTRO_KEY: &str = "nexus2049.collatz.intro";
pub const DAILY_KEY: &str = "nexus2049.daily.state";
pub const DAILY_HIST_KEY: &str = "nexus2049.daily.history";
pub const ENDLESS_KEY: &str = "nexus2049.endless.state";
pub const ENDLESS_BEST_KEY: &str = "nexus2049.endless.best";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassicSave {
    pub cells: Vec<u32>,
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub won: bool,
    #[serde(default)]
    pub keep_playing: bool,
    #[serde(default)]
    pub over: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CzUndo {
    pub cells: Vec<u32>,
    #[serde(default)]
    pub moves: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CzSave {
    pub v: u32,
    pub level: u32,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub moves: u32,
    #[serde(default)]
    pub par: u32,
    pub start: Vec<u32>,
    pub cells: Vec<u32>,
    #[serde(default)]
    pub undo: Vec<CzUndo>,
    #[serde(default)]
    pub undos: u32,
    #[serde(default)]
    pub stuck: bool,
    #[serde(default)]
    pub cleared: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EndlessSave {
    pub cells: Vec<u32>,
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub over: bool,
}

/// Best moves per daily date key.
pub type History = HashMap<String, u32>;

/// A 16-entry board from a save, or None when the save is malformed.
pub fn cells_from(v: &[u32]) -> Option<Cells> {
    if v.len() != CELLS {
        return None;
    }
    let mut cells = [0u32; CELLS];
    cells.copy_from_slice(v);
    Some(cells)
}

pub fn parse<T: for<'a> Deserialize<'a>>(raw: Option<String>) -> Option<T> {
    raw.and_then(|s| serde_json::from_str::<T>(&s).ok())
}

pub fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
