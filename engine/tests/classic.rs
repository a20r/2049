//! Classic 2048 rules on the shared slide.

use nexus_engine::classic::{apply_move, moves_available, spawn};
use nexus_engine::collatz::{Source, CELLS};
use nexus_engine::rng::Mulberry32;

fn board(row0: [u32; 4]) -> [u32; CELLS] {
    let mut b = [0; CELLS];
    b[..4].copy_from_slice(&row0);
    b
}

#[test]
fn merges_double_and_score() {
    let m = apply_move(&board([2, 2, 4, 4]), 3);
    assert_eq!(&m.cells[..4], &[4, 8, 0, 0]);
    assert!(m.moved);
    assert_eq!(m.merges, 2);
    assert_eq!(m.cleared, 0);
    assert_eq!(m.gained, 12);
    assert!(m.clears.is_empty());
    assert_eq!(m.sources.len(), CELLS);
    assert_eq!(m.sources[0], Some(Source::Two { ahead: 0, mover: 1 }));
    assert_eq!(m.sources[1], Some(Source::Two { ahead: 2, mover: 3 }));
    assert_eq!(m.sources[2], None);
}

#[test]
fn merged_tiles_do_not_merge_again() {
    let m = apply_move(&board([2, 2, 2, 2]), 3);
    assert_eq!(&m.cells[..4], &[4, 4, 0, 0]);
    assert_eq!(m.gained, 8);
    let m = apply_move(&board([4, 4, 8, 0]), 3);
    assert_eq!(
        &m.cells[..4],
        &[8, 8, 0, 0],
        "a fresh 8 does not absorb the old 8"
    );
    let m = apply_move(&board([2, 2, 2, 0]), 1);
    assert_eq!(
        &m.cells[..4],
        &[0, 0, 2, 4],
        "the pair nearest the swipe edge merges first"
    );
    assert_eq!(m.sources[3], Some(Source::Two { ahead: 2, mover: 1 }));
    assert_eq!(m.sources[2], Some(Source::One(0)));
}

#[test]
fn sliding_without_merges() {
    let m = apply_move(&board([0, 2, 0, 4]), 3);
    assert_eq!(&m.cells[..4], &[2, 4, 0, 0]);
    assert!(m.moved);
    assert_eq!(m.merges, 0);
    assert_eq!(m.gained, 0);
    assert_eq!(m.sources[0], Some(Source::One(1)));
    assert_eq!(m.sources[1], Some(Source::One(3)));
    let m = apply_move(&board([2, 4, 0, 0]), 3);
    assert!(!m.moved);
    let mut column = [0; CELLS];
    column[0] = 2;
    column[4] = 2;
    column[12] = 8;
    let m = apply_move(&column, 2);
    assert_eq!(m.cells[12], 8);
    assert_eq!(m.cells[8], 4);
    assert_eq!(m.sources[8], Some(Source::Two { ahead: 4, mover: 0 }));
}

#[test]
fn moves_available_checks_empties_and_neighbours() {
    assert!(moves_available(&[0; CELLS]));
    let full = [2, 4, 2, 4, 4, 2, 4, 2, 2, 4, 2, 4, 4, 2, 4, 2];
    assert!(!moves_available(&full));
    let mut horizontal = full;
    horizontal[1] = 2;
    assert!(moves_available(&horizontal));
    let mut vertical = full;
    vertical[4] = 2;
    assert!(moves_available(&vertical));
    let mut hole = full;
    hole[15] = 0;
    assert!(moves_available(&hole));
}

#[test]
fn spawn_picks_an_empty_cell_then_a_value() {
    // Draw order is the cell first, then the value, as JS addRandomTile.
    let mut cells = [2; CELLS];
    cells[5] = 0;
    cells[9] = 0;
    let mut draws = vec![0.6, 0.95];
    let mut next = move || draws.remove(0);
    assert_eq!(spawn(&mut cells, &mut next), Some((9, 4)));
    assert_eq!(cells[9], 4);
    let mut draws = vec![0.0, 0.3];
    let mut next = move || draws.remove(0);
    assert_eq!(spawn(&mut cells, &mut next), Some((5, 2)));
    assert_eq!(cells[5], 2);
    assert_eq!(spawn(&mut cells, &mut || 0.0), None);

    let mut cells = [0; CELLS];
    let mut rng = Mulberry32::new(1);
    let mut twos = 0;
    for _ in 0..CELLS {
        let (idx, value) = spawn(&mut cells, &mut || rng.next_f64()).expect("room left");
        assert_eq!(cells[idx], value);
        assert!(value == 2 || value == 4);
        if value == 2 {
            twos += 1;
        }
    }
    assert!(cells.iter().all(|&v| v != 0));
    assert!(twos >= 10, "roughly nine in ten spawns are 2s");
    assert_eq!(spawn(&mut cells, &mut || rng.next_f64()), None);
}
