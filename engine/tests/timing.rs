//! Prints solver and generator timings (run with `--nocapture`); the bounds
//! asserted here are deliberately loose so a busy machine does not fail them.

use std::time::Instant;

use nexus_engine::collatz::{generate, solve};
use nexus_engine::daily::daily;
use nexus_engine::rng::Mulberry32;

#[test]
fn solve_and_generate_timings() {
    let boards: Vec<_> = ["2026-10-18", "2026-10-19", "2026-12-25", "2027-03-01"]
        .iter()
        .map(|k| daily(k).board.cells)
        .collect();
    let mut worst = 0.0f64;
    for cells in &boards {
        let t0 = Instant::now();
        let s = solve(cells, 0);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        worst = worst.max(ms);
        println!(
            "solve 12 tiles: {:.3} ms, {} nodes, par {}",
            ms,
            s.nodes,
            s.moves.len()
        );
    }
    let mut rng = Mulberry32::new(1012);
    let mut gen_worst = 0.0f64;
    for _ in 0..10 {
        let t0 = Instant::now();
        let b = generate(12, &mut || rng.next_f64());
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        gen_worst = gen_worst.max(ms);
        println!(
            "generate level 12: {:.3} ms, {} attempts, {} nodes",
            ms, b.attempts, b.nodes
        );
    }
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    println!("{profile}: worst solve {worst:.3} ms, worst generate(12) {gen_worst:.3} ms");
    assert!(worst < 500.0, "solve took {worst} ms");
    assert!(gen_worst < 5000.0, "generate took {gen_worst} ms");
}
