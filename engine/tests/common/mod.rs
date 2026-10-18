//! Fixture parsing shared by the integration tests. The fixture files under
//! `tests/fixtures/` were dumped from the JavaScript reference engine.
#![allow(dead_code)]

use nexus_engine::collatz::{Cells, Clear, Source, CELLS};

/// The value palette the JS test draws random boards from.
pub const VALUES: [u32; 10] = [1, 2, 3, 4, 5, 6, 8, 10, 16, 20];

/// A random board exactly as the JS test builds one: 55% filled from `VALUES`.
pub fn random_board(rng: &mut dyn FnMut() -> f64) -> Cells {
    let mut board = [0; CELLS];
    for cell in board.iter_mut() {
        if rng() < 0.55 {
            *cell = VALUES[(rng() * VALUES.len() as f64).floor() as usize];
        }
    }
    board
}

/// FNV-1a over ASCII text, matching the digest the fixture script computed.
pub struct Fnv(pub u32);

impl Fnv {
    pub fn new() -> Self {
        Fnv(2_166_136_261)
    }
    pub fn feed(&mut self, s: &str) {
        for b in s.bytes() {
            self.0 ^= u32::from(b);
            self.0 = self.0.wrapping_mul(16_777_619);
        }
    }
}

impl Default for Fnv {
    fn default() -> Self {
        Fnv::new()
    }
}

pub fn parse_list(s: &str) -> Vec<u32> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(',')
        .map(|x| {
            x.parse::<u32>()
                .unwrap_or_else(|_| panic!("bad number {x:?}"))
        })
        .collect()
}

pub fn parse_dirs(s: &str) -> Vec<u8> {
    parse_list(s).into_iter().map(|d| d as u8).collect()
}

pub fn parse_cells(s: &str) -> Cells {
    let v = parse_list(s);
    assert_eq!(v.len(), CELLS, "a board has 16 cells: {s:?}");
    let mut cells = [0; CELLS];
    cells.copy_from_slice(&v);
    cells
}

pub fn parse_sources(s: &str) -> Vec<Option<Source>> {
    s.split(',')
        .map(|e| {
            if e == "-" {
                None
            } else if let Some((a, b)) = e.split_once('+') {
                Some(Source::Two {
                    ahead: a.parse().unwrap(),
                    mover: b.parse().unwrap(),
                })
            } else {
                Some(Source::One(e.parse().unwrap()))
            }
        })
        .collect()
}

pub fn parse_clears(s: &str) -> Vec<Clear> {
    if s.is_empty() {
        return Vec::new();
    }
    s.split(',')
        .map(|e| {
            let p: Vec<usize> = e.split(':').map(|x| x.parse().unwrap()).collect();
            Clear {
                at: p[0],
                a: p[1],
                b: p[2],
            }
        })
        .collect()
}

pub fn join<T: ToString>(items: &[T]) -> String {
    items
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn source_str(s: &Option<Source>) -> String {
    match s {
        None => "-".to_string(),
        Some(Source::One(i)) => i.to_string(),
        Some(Source::Two { ahead, mover }) => format!("{ahead}+{mover}"),
    }
}
