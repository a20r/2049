//! Collatz mode: the merge rule, the shared sliding logic, the doomed test,
//! the breadth-first solver, the level table and the board generator.
//!
//! Merge rule: two equal tiles n become n/2 (even) or 3n+1 (odd, n >= 3).
//! Two 1s clear. A cleared pair keeps its cell blocked until the move ends,
//! exactly like a merged tile in 2048, so a trailing tile stops behind it:
//! `[1,1,1,_]` swiped left becomes `[_,1,_,_]`.

use std::collections::{HashMap, HashSet};

use crate::rng::pick;

/// Board side length.
pub const SIZE: usize = 4;
/// Number of cells on a board.
pub const CELLS: usize = SIZE * SIZE;
/// A board: 16 values, index = row * 4 + col, 0 = empty.
pub type Cells = [u32; CELLS];

/// Node cap used by `solve` when the caller passes 0.
const DEFAULT_NODES: u32 = 40_000;

/// Marker for a cleared pair's cell for the rest of a move (never a tile value).
const GHOST: u32 = u32::MAX;

/// Level 1's fixed teaching board: an odd pair growing, then a halving chain.
pub const LEVEL_ONE: Cells = [5, 5, 16, 8, 0, 0, 0, 4, 0, 0, 0, 2, 0, 0, 0, 1];

// ---------- the rule ----------

/// Merge result for two tiles of value `n`; 0 means the pair clears.
pub fn f(n: u32) -> u32 {
    if n <= 1 {
        0
    } else if n.is_multiple_of(2) {
        n / 2
    } else {
        3 * n + 1
    }
}

/// Number of merges a pair at `v` is from the clearing pair at 1 (dist(3) = 7).
pub fn dist(v: u32) -> u32 {
    let mut x = u64::from(v);
    let mut d = 0;
    while x > 1 {
        x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
        d += 1;
    }
    d
}

// ---------- doomed test ----------

/// Whether these values can never all be cleared by pairing equal values.
pub fn is_doomed(cells: &[u32]) -> bool {
    // Fixed point: while some value has two or more tiles, replace a pair by
    // f(v), dropping both when v = 1. Merging is forced, so order is irrelevant.
    let mut counts: HashMap<u32, u32> = HashMap::new();
    let mut queue: Vec<u32> = Vec::new();
    for &v in cells {
        if v > 0 {
            let c = counts.entry(v).or_insert(0);
            *c += 1;
            if *c == 2 {
                queue.push(v);
            }
        }
    }
    while let Some(v) = queue.pop() {
        loop {
            let c = counts.entry(v).or_insert(0);
            if *c < 2 {
                break;
            }
            *c -= 2;
            let w = f(v);
            if w != 0 {
                let cw = counts.entry(w).or_insert(0);
                *cw += 1;
                if *cw == 2 {
                    queue.push(w);
                }
            }
        }
    }
    counts.values().any(|&c| c > 0)
}

// ---------- sliding ----------

/// Where an output tile came from: one input index, or a merged pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A tile that moved or stayed, by its input index.
    One(usize),
    /// A merge product; `ahead` is the input tile nearer the swipe edge.
    Two {
        /// Input index of the tile that was merged into.
        ahead: usize,
        /// Input index of the tile that slid into it.
        mover: usize,
    },
}

/// A cleared pair of 1s: cell `at` ends empty, `a` was ahead, `b` the mover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clear {
    /// Cell where the pair met.
    pub at: usize,
    /// Input index of the tile nearer the swipe edge.
    pub a: usize,
    /// Input index of the tile that slid into it.
    pub b: usize,
}

/// Outcome of one swipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveResult {
    /// The board after the move.
    pub cells: Cells,
    /// Whether anything changed.
    pub moved: bool,
    /// Number of merges, clears included.
    pub merges: u32,
    /// Number of cleared pairs.
    pub cleared: u32,
    /// Sum of the output values of non-clearing merges (the 2048 score rule).
    pub gained: u32,
    /// Per output cell, where its tile came from; empty when not tracked.
    pub sources: Vec<Option<Source>>,
    /// Every cleared pair, in processing order; empty when not tracked.
    pub clears: Vec<Clear>,
}

/// Source bookkeeping for a tracked slide.
struct Tracker {
    sources: Vec<Option<Source>>,
    clears: Vec<Clear>,
}

/// Input index behind a single-source entry (merge targets never carry two).
fn first_source(s: Option<Source>, fallback: usize) -> usize {
    match s {
        Some(Source::One(i)) => i,
        Some(Source::Two { ahead, .. }) => ahead,
        None => fallback,
    }
}

/// The slide loop shared by every caller; writes the result into `dst`.
/// Returns (moved, merges, cleared, gained).
fn slide_into(
    src: &Cells,
    dir: u8,
    merge: &dyn Fn(u32) -> u32,
    dst: &mut Cells,
    mut track: Option<&mut Tracker>,
) -> (bool, u32, u32, u32) {
    *dst = *src;
    let (dr, dc): (i32, i32) = match dir {
        0 => (-1, 0),
        1 => (0, 1),
        2 => (1, 0),
        3 => (0, -1),
        _ => return (false, 0, 0, 0),
    };
    let side = SIZE as i32;
    let mut flags = [false; CELLS];
    let mut moved = false;
    let mut merges = 0;
    let mut cleared = 0;
    let mut gained = 0;

    // Lines are processed from the swipe edge outward.
    for ri in 0..SIZE {
        let r = if dr == 1 { SIZE - 1 - ri } else { ri };
        for ci in 0..SIZE {
            let c = if dc == 1 { SIZE - 1 - ci } else { ci };
            let idx = r * SIZE + c;
            let v = dst[idx];
            if v == 0 || v == GHOST {
                continue;
            }

            // Slide until blocked by the edge or any non-empty cell (ghosts block).
            let (mut nr, mut nc) = (r as i32, c as i32);
            loop {
                let (tr, tc) = (nr + dr, nc + dc);
                if tr < 0 || tr >= side || tc < 0 || tc >= side {
                    break;
                }
                if dst[(tr * side + tc) as usize] != 0 {
                    break;
                }
                nr = tr;
                nc = tc;
            }
            let dest = (nr * side + nc) as usize;

            let (ar, ac) = (nr + dr, nc + dc);
            let ahead = if ar >= 0 && ar < side && ac >= 0 && ac < side {
                Some((ar * side + ac) as usize)
            } else {
                None
            };

            match ahead {
                Some(a) if dst[a] == v && !flags[a] => {
                    dst[idx] = 0;
                    let m = merge(v);
                    flags[a] = true;
                    if m != 0 {
                        dst[a] = m;
                        gained += m;
                    } else {
                        dst[a] = GHOST;
                        cleared += 1;
                    }
                    merges += 1;
                    moved = true;
                    if let Some(t) = track.as_deref_mut() {
                        let pair_a = first_source(t.sources[a], a);
                        let pair_b = first_source(t.sources[idx], idx);
                        t.sources[idx] = None;
                        if m != 0 {
                            t.sources[a] = Some(Source::Two {
                                ahead: pair_a,
                                mover: pair_b,
                            });
                        } else {
                            t.sources[a] = None;
                            t.clears.push(Clear {
                                at: a,
                                a: pair_a,
                                b: pair_b,
                            });
                        }
                    }
                }
                _ => {
                    if dest != idx {
                        dst[idx] = 0;
                        dst[dest] = v;
                        moved = true;
                        if let Some(t) = track.as_deref_mut() {
                            t.sources[dest] = t.sources[idx];
                            t.sources[idx] = None;
                        }
                    }
                }
            }
        }
    }
    if cleared > 0 {
        for cell in dst.iter_mut() {
            if *cell == GHOST {
                *cell = 0;
            }
        }
    }
    (moved, merges, cleared, gained)
}

/// Slides a board in `dir` (0 up, 1 right, 2 down, 3 left) under `merge`,
/// where `merge(v) == 0` clears the pair; `track` fills sources and clears.
pub fn slide(cells: &Cells, dir: u8, merge: &dyn Fn(u32) -> u32, track: bool) -> MoveResult {
    let mut tracker = if track {
        Some(Tracker {
            sources: cells
                .iter()
                .enumerate()
                .map(|(i, &v)| if v != 0 { Some(Source::One(i)) } else { None })
                .collect(),
            clears: Vec::new(),
        })
    } else {
        None
    };
    let mut out = [0; CELLS];
    let (moved, merges, cleared, gained) =
        slide_into(cells, dir, merge, &mut out, tracker.as_mut());
    let (sources, clears) = match tracker {
        Some(t) => (t.sources, t.clears),
        None => (Vec::new(), Vec::new()),
    };
    MoveResult {
        cells: out,
        moved,
        merges,
        cleared,
        gained,
        sources,
        clears,
    }
}

/// A Collatz-mode swipe: `slide` with the merge rule `f`.
pub fn apply_move(cells: &Cells, dir: u8, track: bool) -> MoveResult {
    slide(cells, dir, &f, track)
}

/// Whether the board has no tiles.
pub fn is_empty(cells: &Cells) -> bool {
    cells.iter().all(|&v| v == 0)
}

/// Number of non-empty cells.
pub fn count_tiles(cells: &Cells) -> u32 {
    cells.iter().filter(|&&v| v != 0).count() as u32
}

/// Whether some direction changes the board.
pub fn any_move(cells: &Cells) -> bool {
    let mut buf = [0; CELLS];
    (0..4).any(|d| slide_into(cells, d, &f, &mut buf, None).0)
}

// ---------- solver ----------

/// Result of a breadth-first search for the shortest clearing sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solution {
    /// Whether a clearing sequence was found.
    pub solved: bool,
    /// The shortest clearing sequence of directions; empty when unsolved.
    pub moves: Vec<u8>,
    /// Distinct boards generated, root included.
    pub nodes: u32,
    /// Whether the search stopped at the node cap rather than proving a dead end.
    pub exhausted: bool,
}

/// Shortest clearing sequence by breadth-first search; `max_nodes` 0 means 40000.
pub fn solve(cells: &Cells, max_nodes: u32) -> Solution {
    let cap = if max_nodes > 0 {
        max_nodes
    } else {
        DEFAULT_NODES
    };
    let root = *cells;
    if is_empty(&root) {
        return Solution {
            solved: true,
            moves: Vec::new(),
            nodes: 1,
            exhausted: false,
        };
    }
    // The multiset test is only applied to the root: no legal move can turn a
    // clearable multiset into a doomed one, so later dead ends are geometric
    // and the search proves them by exhausting the frontier.
    if is_doomed(&root) {
        return Solution {
            solved: false,
            moves: Vec::new(),
            nodes: 1,
            exhausted: false,
        };
    }

    let mut pool: Vec<Cells> = Vec::with_capacity(4096);
    let mut parents: Vec<u32> = Vec::with_capacity(4096);
    let mut dirs: Vec<u8> = Vec::with_capacity(4096);
    let mut seen: HashSet<Cells> = HashSet::with_capacity(4096);
    pool.push(root);
    parents.push(0);
    dirs.push(0);
    seen.insert(root);
    let mut nodes: u32 = 1;
    let mut head = 0;
    let mut buf = [0; CELLS];

    while head < pool.len() {
        let board = pool[head];
        // Expansion order 0..4 and first-seen deduplication decide which of
        // several shortest solutions is returned, so both match the JS.
        for d in 0..4u8 {
            let (moved, _, cleared, _) = slide_into(&board, d, &f, &mut buf, None);
            if !moved || !seen.insert(buf) {
                continue;
            }
            nodes += 1;
            if cleared > 0 && is_empty(&buf) {
                let mut moves = vec![d];
                let mut p = head;
                while p > 0 {
                    moves.push(dirs[p]);
                    p = parents[p] as usize;
                }
                moves.reverse();
                return Solution {
                    solved: true,
                    moves,
                    nodes,
                    exhausted: false,
                };
            }
            pool.push(buf);
            parents.push(head as u32);
            dirs.push(d);
            if nodes >= cap {
                return Solution {
                    solved: false,
                    moves: Vec::new(),
                    nodes,
                    exhausted: true,
                };
            }
        }
        head += 1;
    }
    Solution {
        solved: false,
        moves: Vec::new(),
        nodes,
        exhausted: false,
    }
}

// ---------- level table ----------

/// Generator settings for one level.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    /// The level these settings describe (at least 1).
    pub level: u32,
    /// Board size at the start of the level.
    pub tiles: u32,
    /// The reverse construction starts from 1 or 2 pairs of 1s.
    pub max_pairs: u32,
    /// When a value has an odd parent, probability of splitting that way.
    pub p_odd: f64,
    /// Probability that a split picks the tile farthest from 1 instead of a random one.
    pub p_deep: f64,
    /// Boards must contain at least this many odd values.
    pub min_odd: u32,
    /// Boards generated per level; the one with the longest par wins.
    pub candidates: u32,
    /// Ceiling on any value the construction may create.
    pub max_value: u32,
    /// Node cap handed to the solver.
    pub max_nodes: u32,
    /// Random placements tried per value multiset.
    pub placements: u32,
    /// Regenerations before the constraints are relaxed.
    pub max_regens: u32,
    /// Whether level 1 deals the fixed `LEVEL_ONE` board.
    pub fixed: bool,
}

/// The level table; levels below 1 are treated as 1.
pub fn params(level: u32) -> Params {
    let l = level.max(1);
    let tiles = (5 + l).min(12);
    Params {
        level: l,
        tiles,
        max_pairs: if tiles <= 8 { 1 } else { 2 },
        p_odd: 0.7,
        p_deep: if l <= 4 {
            0.2
        } else if l <= 7 {
            0.3
        } else {
            (0.3 + 0.03 * f64::from(l - 7)).min(0.45)
        },
        min_odd: if l >= 2 { 1 } else { 0 },
        candidates: if l <= 4 { 1 } else { 2 },
        max_value: if l <= 6 { 100 } else { 400 },
        max_nodes: DEFAULT_NODES,
        placements: 10,
        max_regens: 300,
        fixed: true,
    }
}

// ---------- generator ----------

/// A generated board with its exact par and generation statistics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Board {
    /// The dealt board.
    pub cells: Cells,
    /// Length of the shortest clearing sequence.
    pub par: u32,
    /// A shortest clearing sequence.
    pub moves: Vec<u8>,
    /// The value multiset in construction order.
    pub values: Vec<u32>,
    /// Placements solved while generating.
    pub attempts: u32,
    /// Solver nodes over all placements.
    pub nodes: u32,
    /// Largest solver node count of a single placement.
    pub peak_nodes: u32,
    /// Value multisets rejected before a board was found.
    pub regens: u32,
}

/// The odd u >= 3 with 3u + 1 = v, or 0 when there is none.
fn odd_parent(v: u32) -> u32 {
    if v < 10 || !(v - 1).is_multiple_of(3) {
        return 0;
    }
    let u = (v - 1) / 3;
    if u % 2 == 1 {
        u
    } else {
        0
    }
}

/// Number of odd values above 1.
fn count_odd(values: &[u32]) -> u32 {
    values.iter().filter(|&&v| v % 2 == 1 && v > 1).count() as u32
}

/// Reverse construction from pairs of 1s; None when every tile is at the ceiling.
fn build_values(p: &Params, rng: &mut dyn FnMut() -> f64) -> Option<Vec<u32>> {
    // Short-circuit order matters: rng is only consumed when max_pairs > 1.
    let pairs = if p.max_pairs > 1 && rng() < 0.5 { 2 } else { 1 };
    let mut vals: Vec<u32> = vec![1; pairs * 2];
    let mut cand: Vec<usize> = Vec::new();
    let mut ties: Vec<usize> = Vec::new();
    while (vals.len() as u32) < p.tiles {
        cand.clear();
        for (i, &v) in vals.iter().enumerate() {
            let even = 2 * v <= p.max_value;
            let odd = odd_parent(v);
            let odd = if odd > p.max_value { 0 } else { odd };
            if even || odd != 0 {
                cand.push(i);
            }
        }
        if cand.is_empty() {
            return None;
        }
        let idx = if p.p_deep > 0.0 && rng() < p.p_deep {
            // Farthest from 1 among the splittable tiles, ties broken at random.
            let mut best: Option<u32> = None;
            ties.clear();
            for &ci in &cand {
                let dd = dist(vals[ci]);
                if best.is_none_or(|b| dd > b) {
                    best = Some(dd);
                    ties.clear();
                    ties.push(ci);
                } else if best == Some(dd) {
                    ties.push(ci);
                }
            }
            ties[pick(rng, ties.len())]
        } else {
            cand[pick(rng, cand.len())]
        };
        let v = vals[idx];
        let even = 2 * v <= p.max_value;
        let odd = odd_parent(v);
        let odd = if odd > p.max_value { 0 } else { odd };
        // rng is consumed for the parity choice only when both parents exist.
        let u = if odd != 0 && (!even || rng() < p.p_odd) {
            odd
        } else {
            2 * v
        };
        vals[idx] = u;
        vals.push(u);
    }
    Some(vals)
}

/// Fisher-Yates placement of `values` on an empty board.
fn place(values: &[u32], rng: &mut dyn FnMut() -> f64) -> Cells {
    let mut order: [usize; CELLS] = [0; CELLS];
    for (i, slot) in order.iter_mut().enumerate() {
        *slot = i;
    }
    for i in (1..CELLS).rev() {
        let j = pick(rng, i + 1);
        order.swap(i, j);
    }
    let mut cells = [0; CELLS];
    for (i, &v) in values.iter().enumerate().take(CELLS) {
        cells[order[i]] = v;
    }
    cells
}

/// Generates a board for `level` with the level table's settings.
pub fn generate(level: u32, rng: &mut dyn FnMut() -> f64) -> Board {
    generate_with(params(level), rng)
}

/// Generates a board under explicit settings; level 1 with `fixed` deals `LEVEL_ONE`.
pub fn generate_with(p: Params, rng: &mut dyn FnMut() -> f64) -> Board {
    let mut p = p;
    if p.level == 1 && p.fixed {
        let sol = solve(&LEVEL_ONE, p.max_nodes);
        return Board {
            cells: LEVEL_ONE,
            par: sol.moves.len() as u32,
            moves: sol.moves,
            values: LEVEL_ONE.iter().copied().filter(|&v| v > 0).collect(),
            attempts: 1,
            nodes: sol.nodes,
            peak_nodes: sol.nodes,
            regens: 0,
        };
    }
    let mut attempts = 0;
    let mut nodes = 0;
    let mut peak = 0;
    let mut regens = 0;
    let mut best: Option<Board> = None;
    // JS would crash on zero candidates; one is the sensible minimum.
    for _ in 0..p.candidates.max(1) {
        let board = loop {
            // Relax the constraints rather than loop forever when a parameter
            // set is hard to satisfy; a missing multiset means the ceiling is
            // too low for the tile count, so raise it.
            if regens >= p.max_regens {
                p.min_odd = 0;
                p.p_deep = 0.0;
            }
            let values = build_values(&p, rng);
            if values.is_none() && regens >= p.max_regens {
                p.max_value *= 2;
            }
            let mut found: Option<Board> = None;
            if let Some(values) = values {
                if count_odd(&values) >= p.min_odd {
                    for _ in 0..p.placements {
                        if found.is_some() {
                            break;
                        }
                        let cells = place(&values, rng);
                        let res = solve(&cells, p.max_nodes);
                        attempts += 1;
                        nodes += res.nodes;
                        peak = peak.max(res.nodes);
                        if res.solved {
                            found = Some(Board {
                                cells,
                                par: res.moves.len() as u32,
                                moves: res.moves,
                                values: values.clone(),
                                attempts: 0,
                                nodes: 0,
                                peak_nodes: 0,
                                regens: 0,
                            });
                        }
                    }
                }
            }
            match found {
                Some(b) => break b,
                None => regens += 1,
            }
        };
        if best.as_ref().is_none_or(|b| board.par > b.par) {
            best = Some(board);
        }
    }
    let mut best = best.unwrap_or_else(|| Board {
        cells: [0; CELLS],
        par: 0,
        moves: Vec::new(),
        values: Vec::new(),
        attempts: 0,
        nodes: 0,
        peak_nodes: 0,
        regens: 0,
    });
    best.attempts = attempts;
    best.nodes = nodes;
    best.peak_nodes = peak;
    best.regens = regens;
    best
}
