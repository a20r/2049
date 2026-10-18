//! The assertions of the JavaScript test file (collatz.test.js), ported.

mod common;

use std::collections::{HashMap, HashSet};

use common::{random_board, VALUES};
use nexus_engine::collatz::{
    any_move, apply_move, count_tiles, dist, f, generate, is_doomed, is_empty, params, solve,
    Clear, Source, LEVEL_ONE,
};
use nexus_engine::rng::Mulberry32;

fn row(cells: &[u32; 16], r: usize) -> [u32; 4] {
    [
        cells[r * 4],
        cells[r * 4 + 1],
        cells[r * 4 + 2],
        cells[r * 4 + 3],
    ]
}

fn board(row0: [u32; 4]) -> [u32; 16] {
    let mut b = [0; 16];
    b[..4].copy_from_slice(&row0);
    b
}

#[test]
fn rule() {
    assert_eq!(f(1), 0);
    assert_eq!(f(2), 1);
    assert_eq!(f(3), 10);
    assert_eq!(f(10), 5);
    assert_eq!(f(16), 8);
    assert_eq!(f(0), 0);
    assert_eq!(dist(3), 7);
    assert_eq!(dist(27), 111);
    assert_eq!(dist(1), 0);
    assert_eq!(dist(2), 1);
    assert_eq!(dist(4), 2);
    assert_eq!(dist(0), 0);
}

#[test]
fn ghost_semantics() {
    let r1 = apply_move(&board([1, 1, 1, 0]), 3, true);
    assert_eq!(
        row(&r1.cells, 0),
        [0, 1, 0, 0],
        "third 1 stops behind the cleared cell"
    );
    assert_eq!(r1.cleared, 1);
    assert_eq!(r1.clears, vec![Clear { at: 0, a: 0, b: 1 }]);
    assert_eq!(r1.sources[1], Some(Source::One(2)));
    assert_eq!(r1.gained, 0);

    let r2 = apply_move(&board([2, 1, 1, 2]), 3, true);
    assert_eq!(
        row(&r2.cells, 0),
        [2, 0, 2, 0],
        "trailing 2 does not slide through a cleared pair"
    );
    assert_eq!(r2.sources[2], Some(Source::One(3)));

    let r3 = apply_move(&board([5, 5, 16, 8]), 3, true);
    assert_eq!(row(&r3.cells, 0), [16, 16, 8, 0]);
    assert_eq!(
        r3.sources[0],
        Some(Source::Two { ahead: 0, mover: 1 }),
        "ahead tile first, mover second"
    );
    assert_eq!(r3.merges, 1);
    assert_eq!(r3.gained, 16);

    let r4 = apply_move(&board([4, 4, 4, 4]), 1, true);
    assert_eq!(
        row(&r4.cells, 0),
        [0, 0, 2, 2],
        "merge products do not merge again this move"
    );
    assert_eq!(r4.sources[2], Some(Source::Two { ahead: 1, mover: 0 }));
    assert_eq!(r4.sources[3], Some(Source::Two { ahead: 3, mover: 2 }));

    let r5 = apply_move(&[0; 16], 0, true);
    assert!(!r5.moved);
    assert_eq!(r5.sources, vec![None; 16]);
    assert!(r5.clears.is_empty());

    let untracked = apply_move(&board([1, 1, 1, 0]), 3, false);
    assert!(untracked.sources.is_empty());
    assert!(untracked.clears.is_empty());
    assert_eq!(untracked.cells, r1.cells);
}

// A tracked and an untracked move must agree on every random board, and the
// sources must account for every input tile exactly once.
#[test]
fn tracked_and_untracked_agree_with_full_source_accounting() {
    let mut rng = Mulberry32::new(11);
    for _ in 0..2000 {
        let board = random_board(&mut || rng.next_f64());
        for d in 0..4u8 {
            let a = apply_move(&board, d, false);
            let b = apply_move(&board, d, true);
            assert_eq!(a.cells, b.cells);
            assert_eq!(a.moved, b.moved);
            assert_eq!(a.merges, b.merges);
            assert_eq!(a.cleared, b.cleared);
            assert_eq!(a.gained, b.gained);
            assert_eq!(b.sources.len(), 16);
            let mut seen: HashSet<usize> = HashSet::new();
            let mut gained = 0;
            for i in 0..16 {
                if b.cells[i] != 0 {
                    match b.sources[i].expect("tile without sources") {
                        Source::One(s) => {
                            assert!(board[s] > 0);
                            assert!(seen.insert(s));
                            assert_eq!(b.cells[i], board[s]);
                        }
                        Source::Two { ahead, mover } => {
                            assert!(board[ahead] > 0 && board[mover] > 0);
                            assert!(seen.insert(ahead));
                            assert!(seen.insert(mover));
                            assert_eq!(board[ahead], board[mover]);
                            assert_eq!(b.cells[i], f(board[ahead]));
                            gained += b.cells[i];
                        }
                    }
                } else {
                    assert_eq!(b.sources[i], None);
                }
            }
            for c in &b.clears {
                assert_eq!(board[c.a], 1);
                assert_eq!(board[c.b], 1);
                assert!(seen.insert(c.a));
                assert!(seen.insert(c.b));
                assert_eq!(b.cells[c.at], 0);
            }
            let inputs = board.iter().filter(|&&v| v != 0).count();
            assert_eq!(seen.len(), inputs, "every input tile accounted for");
            assert_eq!(b.gained, gained);
            assert_eq!(
                b.merges as usize,
                b.clears.len() + (inputs - count_tiles(&b.cells) as usize - 2 * b.clears.len())
            );
        }
    }
}

/// Port of the JS test's bruteClearable: exhaustive pairing search with memo.
fn brute_clearable(values: &[u32]) -> bool {
    fn rec(ms: Vec<u32>, memo: &mut HashMap<Vec<u32>, bool>) -> bool {
        if ms.is_empty() {
            return true;
        }
        if let Some(&r) = memo.get(&ms) {
            return r;
        }
        let mut ok = false;
        let mut tried: HashSet<u32> = HashSet::new();
        for i in 0..ms.len() {
            if ok {
                break;
            }
            let v = ms[i];
            if !tried.insert(v) {
                continue;
            }
            let j = match ms[i + 1..].iter().position(|&x| x == v) {
                Some(p) => i + 1 + p,
                None => continue,
            };
            let mut next = ms.clone();
            next.remove(j);
            next.remove(i);
            let w = f(v);
            if w != 0 {
                next.push(w);
            }
            next.sort_unstable();
            ok = rec(next, memo);
        }
        memo.insert(ms, ok);
        ok
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    rec(sorted, &mut HashMap::new())
}

#[test]
fn doomed() {
    assert!(is_doomed(&[4, 4, 2, 2]));
    assert!(is_doomed(&[3, 3, 10, 5, 5, 16]));
    assert!(
        !is_doomed(&[3, 3, 10, 5, 16, 8, 4, 2, 1]),
        "magnitude order would wrongly say doomed"
    );
    assert!(is_doomed(&[16, 16]));
    assert!(!is_doomed(&[]));
    assert!(!is_doomed(&[0, 0, 1, 1, 0]));
    let mut rng = Mulberry32::new(5);
    for _ in 0..3000 {
        let n = 2 + (rng.next_f64() * 9.0).floor() as usize;
        let ms: Vec<u32> = (0..n)
            .map(|_| VALUES[(rng.next_f64() * VALUES.len() as f64).floor() as usize])
            .collect();
        assert_eq!(
            is_doomed(&ms),
            !brute_clearable(&ms),
            "doomed mismatch on {ms:?}"
        );
    }
}

#[test]
fn solver() {
    let one = solve(&LEVEL_ONE, 0);
    assert!(one.solved, "level one is solvable");
    assert_eq!(one.moves, vec![1, 1, 1, 0, 0, 0]);
    assert_eq!(one.nodes, 33);
    let mut replay = LEVEL_ONE;
    for &dir in &one.moves {
        replay = apply_move(&replay, dir, false).cells;
    }
    assert!(is_empty(&replay), "solution replays to an empty board");

    let empty = solve(&[0; 16], 0);
    assert!(empty.solved);
    assert!(empty.moves.is_empty());
    assert_eq!(empty.nodes, 1);

    let dead = solve(&board([2, 1, 2, 0]), 0);
    assert!(!dead.solved);
    assert!(!dead.exhausted, "dead end is proven, not capped");
    assert!(dead.moves.is_empty());

    let doomed = solve(&board([4, 4, 2, 2]), 0);
    assert!(!doomed.solved);
    assert_eq!(doomed.nodes, 1);

    let capped = solve(&LEVEL_ONE, 10);
    assert!(!capped.solved);
    assert!(capped.exhausted);
    assert_eq!(capped.nodes, 10);
}

#[test]
fn helpers() {
    assert!(is_empty(&[0; 16]));
    assert!(!is_empty(&LEVEL_ONE));
    assert_eq!(count_tiles(&LEVEL_ONE), 7);
    assert_eq!(count_tiles(&[0; 16]), 0);
    assert!(any_move(&LEVEL_ONE));
    assert!(!any_move(&[0; 16]));
    assert!(!any_move(&[
        1, 2, 3, 4, 5, 6, 8, 10, 16, 20, 1, 2, 3, 4, 5, 6
    ]));
    assert!(any_move(&[
        1, 2, 3, 4, 5, 6, 8, 10, 16, 20, 1, 2, 3, 4, 5, 5
    ]));
}

#[test]
fn level_table() {
    let p1 = params(1);
    assert_eq!(p1.level, 1);
    assert_eq!(p1.tiles, 6);
    assert_eq!(p1.max_pairs, 1);
    assert_eq!(p1.p_odd, 0.7);
    assert_eq!(p1.p_deep, 0.2);
    assert_eq!(p1.min_odd, 0);
    assert_eq!(p1.candidates, 1);
    assert_eq!(p1.max_value, 100);
    assert_eq!(p1.max_nodes, 40000);
    assert_eq!(p1.placements, 10);
    assert_eq!(p1.max_regens, 300);
    assert!(p1.fixed);
    assert_eq!(params(0), p1, "levels below 1 are level 1");
    assert_eq!(params(2).min_odd, 1);
    assert_eq!(params(3).tiles, 8);
    assert_eq!(params(3).max_pairs, 1);
    assert_eq!(params(4).tiles, 9);
    assert_eq!(params(4).max_pairs, 2);
    assert_eq!(params(5).p_deep, 0.3);
    assert_eq!(params(5).candidates, 2);
    assert_eq!(params(7).p_deep, 0.3);
    assert_eq!(params(7).max_value, 400);
    assert_eq!(params(8).p_deep, 0.3 + 0.03);
    assert_eq!(params(12).p_deep, 0.3 + 0.03 * 5.0);
    assert_eq!(params(12).tiles, 12);
    assert_eq!(params(20).p_deep, 0.45);
    assert_eq!(params(20).tiles, 12);
}

#[test]
fn generator_level_one_is_fixed() {
    let one = solve(&LEVEL_ONE, 0);
    let mut rng = Mulberry32::new(99);
    let mut draws = 0;
    let g1 = generate(1, &mut || {
        draws += 1;
        rng.next_f64()
    });
    assert_eq!(g1.cells, LEVEL_ONE);
    assert_eq!(g1.par, one.moves.len() as u32);
    assert_eq!(g1.par, 6);
    assert_eq!(g1.moves, one.moves);
    assert_eq!(g1.values, vec![5, 5, 16, 8, 4, 2, 1]);
    assert_eq!(g1.attempts, 1);
    assert_eq!(g1.nodes, one.nodes);
    assert_eq!(g1.peak_nodes, one.nodes);
    assert_eq!(g1.regens, 0);
    assert_eq!(draws, 0, "the fixed deal does not consume the rng");
}

// Every generated board has exactly params(L).tiles tiles, is not doomed, and
// its moves replay to an empty board (the par is exact for the slide rule).
#[test]
fn generated_boards_are_consistent() {
    for level in [2, 3, 4, 5, 6, 7, 8, 10, 12, 15, 20] {
        let mut rng = Mulberry32::new(1000 + level);
        for _ in 0..3 {
            let g = generate(level, &mut || rng.next_f64());
            let mut rep = g.cells;
            for &dir in &g.moves {
                rep = apply_move(&rep, dir, false).cells;
            }
            assert!(is_empty(&rep));
            assert_eq!(g.par as usize, g.moves.len());
            assert_eq!(count_tiles(&g.cells), params(level).tiles);
            assert!(!is_doomed(&g.cells));
            assert_eq!(g.values.len() as u32, params(level).tiles);
            if level >= 2 {
                assert!(
                    g.values.iter().any(|&v| v % 2 == 1 && v > 1),
                    "min_odd holds"
                );
            }
            assert_eq!(g.par, solve(&g.cells, 0).moves.len() as u32, "par is exact");
        }
    }
}

#[test]
fn random_level_one_deal() {
    let mut p = params(1);
    p.fixed = false;
    let mut rng = Mulberry32::new(99);
    let g = nexus_engine::collatz::generate_with(p, &mut || rng.next_f64());
    assert_ne!(g.cells, LEVEL_ONE);
    assert_eq!(count_tiles(&g.cells), 6);
    assert!(g.par > 0);
}
