//! Deterministic randomness: the FNV-1a string hash and the mulberry32
//! generator used by the JavaScript engine, reproduced bit for bit.

/// FNV-1a hash of a string over its UTF-16 code units, as JS `hashString`.
pub fn hash_string(s: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for unit in s.encode_utf16() {
        h ^= u32::from(unit);
        h = h.wrapping_mul(16_777_619);
    }
    h
}

/// The mulberry32 generator: 32 bits of state, one `f64` in [0, 1) per step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// A generator seeded with a raw 32-bit value.
    pub fn new(seed: u32) -> Self {
        Mulberry32 { state: seed }
    }

    /// A generator seeded with `hash_string(s)`, as JS `seededRng(string)`.
    pub fn from_seed_str(s: &str) -> Self {
        Mulberry32::new(hash_string(s))
    }

    /// The next value in [0, 1), identical to the JS sequence for the same seed.
    pub fn next_f64(&mut self) -> f64 {
        // JS works in int32 with Math.imul and `>>> 0`; u32 wrapping arithmetic
        // gives the same bit patterns, and `t ^= t + imul(..)` is a wrapping
        // add followed by xor.
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

/// `floor(rng() * n)` as JS computes it, clamped into `0..n` for safety.
pub(crate) fn pick(rng: &mut dyn FnMut() -> f64, n: usize) -> usize {
    let i = (rng() * n as f64).floor();
    if i < 0.0 {
        0
    } else {
        (i as usize).min(n.saturating_sub(1))
    }
}
