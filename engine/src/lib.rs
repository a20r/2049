//! Rules, solver and board generator for 2049's game modes, ported from the
//! JavaScript engine with bit-for-bit behavioural parity (same random
//! sequences, same boards, same solutions).
//!
//! Boards are flat arrays of 16 values, index = row * 4 + col, 0 = empty.
//! Directions are 0 up, 1 right, 2 down, 3 left.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod classic;
pub mod collatz;
pub mod daily;
pub mod rng;
