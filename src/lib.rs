//! 2049 in the browser: the game loop, rendering and input live in `app`,
//! the rules in the `nexus-engine` crate. A few engine functions are exported
//! for tests and tinkering (`window.Nexus` in index.html).

mod app;
mod dom;
mod storage;

use nexus_engine::collatz::{self, Cells, CELLS};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    app::boot();
}

fn cells_arg(cells: &[u32]) -> Cells {
    let mut out = [0u32; CELLS];
    for (i, v) in cells.iter().take(CELLS).enumerate() {
        out[i] = *v;
    }
    out
}

/// The daily board for a YYYY-MM-DD key.
#[wasm_bindgen]
pub fn daily_cells(key: &str) -> Vec<u32> {
    nexus_engine::daily::daily(key).board.cells.to_vec()
}

/// Shortest clearing line for the daily board, as direction codes (0 up, 1 right, 2 down, 3 left).
#[wasm_bindgen]
pub fn daily_moves(key: &str) -> Vec<u8> {
    nexus_engine::daily::daily(key).board.moves
}

#[wasm_bindgen]
pub fn daily_day(key: &str) -> i64 {
    nexus_engine::daily::day_number(key)
}

#[wasm_bindgen]
pub fn date_key() -> String {
    dom::today_key()
}

/// The board after one Collatz move.
#[wasm_bindgen]
pub fn apply_move(cells: &[u32], dir: u8) -> Vec<u32> {
    collatz::apply_move(&cells_arg(cells), dir, false)
        .cells
        .to_vec()
}

/// Shortest clearing line from a board, empty when there is none.
#[wasm_bindgen]
pub fn solve(cells: &[u32]) -> Vec<u8> {
    collatz::solve(&cells_arg(cells), 0).moves
}

/// A generated board for a level, using the page's random source.
#[wasm_bindgen]
pub fn generate(level: u32) -> Vec<u32> {
    collatz::generate(level, &mut dom::random).cells.to_vec()
}
