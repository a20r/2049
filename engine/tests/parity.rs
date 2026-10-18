//! Parity with the JavaScript engine: every fixture here was dumped from the
//! reference `collatz.js`, so these tests fail if the port drifts from it.

mod common;

use common::{
    join, parse_cells, parse_clears, parse_dirs, parse_list, parse_sources, random_board,
    source_str, Fnv, VALUES,
};
use nexus_engine::collatz::{apply_move, generate, is_doomed, solve, Clear, MoveResult, LEVEL_ONE};
use nexus_engine::daily::daily;
use nexus_engine::rng::{hash_string, Mulberry32};

const MOVES: &str = include_str!("fixtures/moves.txt");
const SOLVE: &str = include_str!("fixtures/solve.txt");
const GENERATE: &str = include_str!("fixtures/generate.txt");
const DAILY: &str = include_str!("fixtures/daily.txt");

// JS: mulberry32(7) first eight values.
#[test]
fn mulberry32_matches_js_sequence() {
    let mut rng = Mulberry32::new(7);
    let expected: [f64; 8] = [
        0.011704753153026104,
        0.06195825757458806,
        0.97690763277933,
        0.6990287057124078,
        0.5214452685322613,
        0.4055216880515218,
        0.4662326325196773,
        0.23992518591694534,
    ];
    for (i, &e) in expected.iter().enumerate() {
        let got = rng.next_f64();
        assert_eq!(got.to_bits(), e.to_bits(), "value {i}: {got} != {e}");
    }
}

// JS: hashString of the daily seed and two edge cases.
#[test]
fn hash_string_matches_js() {
    assert_eq!(hash_string("2049-daily-2026-10-18"), 2_606_801_405);
    assert_eq!(hash_string(""), 2_166_136_261);
    assert_eq!(hash_string("a"), 3_826_002_220);
    assert_eq!(
        Mulberry32::from_seed_str("2049-daily-2026-10-18"),
        Mulberry32::new(2_606_801_405)
    );
}

/// The fixture line for one move, in the exact format the JS script wrote.
fn move_line(board: &[u32; 16], dir: u8, m: &MoveResult) -> String {
    let sources: Vec<String> = m.sources.iter().map(source_str).collect();
    let clears: Vec<String> = m
        .clears
        .iter()
        .map(|c| format!("{}:{}:{}", c.at, c.a, c.b))
        .collect();
    format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        join(board),
        dir,
        join(&m.cells),
        u8::from(m.moved),
        m.merges,
        m.cleared,
        m.gained,
        sources.join(","),
        clears.join(",")
    )
}

// JS applyMove(board, dir, true) on the 2000 boards the JS test draws from
// mulberry32(11): the first 500 boards are compared field by field against
// the embedded fixture, all 2000 x 4 against an FNV digest of the same lines.
#[test]
fn apply_move_matches_js_on_random_boards() {
    let mut rng = Mulberry32::new(11);
    let mut fixture = MOVES.lines();
    let mut digest = Fnv::new();
    let mut compared = 0;
    for t in 0..2000 {
        let board = random_board(&mut || rng.next_f64());
        for dir in 0..4u8 {
            let m = apply_move(&board, dir, true);
            let line = move_line(&board, dir, &m);
            digest.feed(&line);
            digest.feed("\n");
            if let Some(expected) = fixture.next() {
                let p: Vec<&str> = expected.split('|').collect();
                assert_eq!(p.len(), 9, "fixture line {t}/{dir}");
                assert_eq!(board, parse_cells(p[0]), "board {t} differs from JS draw");
                assert_eq!(dir, p[1].parse::<u8>().unwrap());
                assert_eq!(m.cells, parse_cells(p[2]), "cells, board {t} dir {dir}");
                assert_eq!(m.moved, p[3] == "1", "moved, board {t} dir {dir}");
                assert_eq!(m.merges, p[4].parse::<u32>().unwrap(), "merges {t}/{dir}");
                assert_eq!(m.cleared, p[5].parse::<u32>().unwrap(), "cleared {t}/{dir}");
                assert_eq!(m.gained, p[6].parse::<u32>().unwrap(), "gained {t}/{dir}");
                assert_eq!(m.sources, parse_sources(p[7]), "sources {t}/{dir}");
                assert_eq!(m.clears, parse_clears(p[8]), "clears {t}/{dir}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 2000, "500 boards x 4 directions in the fixture");
    assert_eq!(
        digest.0, 2_772_424_138,
        "digest over 2000 boards x 4 directions"
    );
}

// JS solve() on LEVEL_ONE, 30 generated boards and four dead ends.
#[test]
fn solve_matches_js() {
    let mut count = 0;
    for line in SOLVE.lines() {
        let p: Vec<&str> = line.split('|').collect();
        let cells = parse_cells(p[0]);
        let s = solve(&cells, 0);
        assert_eq!(s.solved, p[1] == "1", "solved on {line}");
        assert_eq!(s.moves, parse_dirs(p[2]), "moves on {line}");
        assert_eq!(s.nodes, p[3].parse::<u32>().unwrap(), "nodes on {line}");
        assert_eq!(s.exhausted, p[4] == "1", "exhausted on {line}");
        count += 1;
    }
    assert_eq!(count, 35);
    assert_eq!(
        parse_cells(SOLVE.lines().next().unwrap().split('|').next().unwrap()),
        LEVEL_ONE
    );
}

// JS generate(L, mulberry32(1000 + L)) for L in 2..=12, as the JS test seeds levels.
#[test]
fn generate_matches_js_per_level() {
    let mut levels = Vec::new();
    for line in GENERATE.lines() {
        let p: Vec<&str> = line.split('|').collect();
        let level: u32 = p[0].parse().unwrap();
        let mut rng = Mulberry32::new(1000 + level);
        let b = generate(level, &mut || rng.next_f64());
        assert_eq!(b.cells, parse_cells(p[1]), "cells, level {level}");
        assert_eq!(b.par, p[2].parse::<u32>().unwrap(), "par, level {level}");
        assert_eq!(b.moves, parse_dirs(p[3]), "moves, level {level}");
        assert_eq!(b.values, parse_list(p[4]), "values, level {level}");
        assert_eq!(
            b.attempts,
            p[5].parse::<u32>().unwrap(),
            "attempts, level {level}"
        );
        assert_eq!(
            b.nodes,
            p[6].parse::<u32>().unwrap(),
            "nodes, level {level}"
        );
        assert_eq!(
            b.peak_nodes,
            p[7].parse::<u32>().unwrap(),
            "peak, level {level}"
        );
        assert_eq!(
            b.regens,
            p[8].parse::<u32>().unwrap(),
            "regens, level {level}"
        );
        levels.push(level);
    }
    assert_eq!(levels, (2..=12).collect::<Vec<u32>>());
}

// JS daily(key) for four dates: players' boards must survive the engine switch.
#[test]
fn daily_matches_js() {
    let mut keys = Vec::new();
    for line in DAILY.lines() {
        let p: Vec<&str> = line.split('|').collect();
        let d = daily(p[0]);
        assert_eq!(d.date, p[0]);
        assert_eq!(d.day, p[1].parse::<i64>().unwrap(), "day of {}", p[0]);
        assert_eq!(d.board.cells, parse_cells(p[2]), "cells of {}", p[0]);
        assert_eq!(d.board.par, p[3].parse::<u32>().unwrap(), "par of {}", p[0]);
        assert_eq!(d.board.moves, parse_dirs(p[4]), "moves of {}", p[0]);
        assert_eq!(
            d.board.attempts,
            p[5].parse::<u32>().unwrap(),
            "attempts of {}",
            p[0]
        );
        assert_eq!(
            d.board.nodes,
            p[6].parse::<u32>().unwrap(),
            "nodes of {}",
            p[0]
        );
        assert_eq!(
            d.board.peak_nodes,
            p[7].parse::<u32>().unwrap(),
            "peak of {}",
            p[0]
        );
        assert_eq!(
            d.board.regens,
            p[8].parse::<u32>().unwrap(),
            "regens of {}",
            p[0]
        );
        keys.push(p[0].to_string());
    }
    assert_eq!(
        keys,
        ["2026-10-18", "2026-10-19", "2026-12-25", "2027-03-01"]
    );
}

// JS isDoomed over the 3000 multisets the JS test draws from mulberry32(5).
#[test]
fn is_doomed_matches_js_digest() {
    let mut rng = Mulberry32::new(5);
    let mut digest = Fnv::new();
    for _ in 0..3000 {
        let n = 2 + (rng.next_f64() * 9.0).floor() as usize;
        let ms: Vec<u32> = (0..n)
            .map(|_| VALUES[(rng.next_f64() * VALUES.len() as f64).floor() as usize])
            .collect();
        digest.feed(&format!("{}={}\n", join(&ms), u8::from(is_doomed(&ms))));
    }
    assert_eq!(digest.0, 3_544_273_267);
}

#[test]
fn clear_struct_round_trips_through_fixture_format() {
    assert_eq!(parse_clears("0:0:1"), vec![Clear { at: 0, a: 0, b: 1 }]);
}
