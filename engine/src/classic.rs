//! Classic 2048: the same slide with doubling merges, plus spawning.

use crate::collatz::{slide, Cells, MoveResult, CELLS, SIZE};
use crate::rng::pick;

/// A 2048 swipe: equal tiles double, merge products do not merge again; always tracked.
pub fn apply_move(cells: &Cells, dir: u8) -> MoveResult {
    slide(cells, dir, &|n| n * 2, true)
}

/// Whether any cell is empty or any two orthogonal neighbours are equal.
pub fn moves_available(cells: &Cells) -> bool {
    for r in 0..SIZE {
        for c in 0..SIZE {
            let v = cells[r * SIZE + c];
            if v == 0 {
                return true;
            }
            if c + 1 < SIZE && cells[r * SIZE + c + 1] == v {
                return true;
            }
            if r + 1 < SIZE && cells[(r + 1) * SIZE + c] == v {
                return true;
            }
        }
    }
    false
}

/// Places a 2 (90%) or 4 (10%) on a random empty cell; None when the board is full.
pub fn spawn(cells: &mut Cells, rng: &mut dyn FnMut() -> f64) -> Option<(usize, u32)> {
    let empty: Vec<usize> = (0..CELLS).filter(|&i| cells[i] == 0).collect();
    if empty.is_empty() {
        return None;
    }
    // Same draw order as JS addRandomTile: the cell first, then the value.
    let idx = empty[pick(rng, empty.len())];
    let value = if rng() < 0.9 { 2 } else { 4 };
    cells[idx] = value;
    Some((idx, value))
}
