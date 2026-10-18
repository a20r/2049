//! The daily board: a date key seeds the generator so everyone gets the same
//! board, plus the calendar arithmetic around it.

use crate::collatz::{generate_with, params, Board};
use crate::rng::Mulberry32;

/// The level the daily board is generated at.
pub const DAILY_LEVEL: u32 = 8;

/// Days since 1970-01-01 of the first daily board, 2026-10-18.
const EPOCH_DAYS: i64 = days_from_civil(2026, 10, 18);

/// Days since 1970-01-01 of a proleptic Gregorian date.
const fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian (year, month, day) of a day count since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Parses 'YYYY-MM-DD' into numbers; missing or malformed parts read as 0.
fn parse_key(key: &str) -> (i64, i64, i64) {
    let mut parts = key.split('-').map(|p| p.trim().parse::<i64>().unwrap_or(0));
    let y = parts.next().unwrap_or(0);
    let m = parts.next().unwrap_or(0);
    let d = parts.next().unwrap_or(0);
    (y, m, d)
}

/// The daily number of a 'YYYY-MM-DD' key: 2026-10-18 is day 1.
pub fn day_number(key: &str) -> i64 {
    let (y, m, d) = parse_key(key);
    days_from_civil(y, m, d) - EPOCH_DAYS + 1
}

/// A zero-padded 'YYYY-MM-DD' key.
pub fn date_key(year: i32, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

/// The key `days` calendar days after `key` (negative moves back).
pub fn shift_date_key(key: &str, days: i64) -> String {
    let (y, m, d) = parse_key(key);
    let (ny, nm, nd) = civil_from_days(days_from_civil(y, m, d) + days);
    date_key(ny as i32, nm as u32, nd as u32)
}

/// A daily board with its day number and date key.
#[derive(Clone, Debug)]
pub struct Daily {
    /// The generated board and its par.
    pub board: Board,
    /// Day number, 2026-10-18 being 1.
    pub day: i64,
    /// The 'YYYY-MM-DD' key the board was generated for.
    pub date: String,
}

/// The board for a date key: same key, same board everywhere.
pub fn daily(key: &str) -> Daily {
    let mut p = params(DAILY_LEVEL);
    p.candidates = 3;
    let mut rng = Mulberry32::from_seed_str(&format!("2049-daily-{key}"));
    let board = generate_with(p, &mut || rng.next_f64());
    Daily {
        board,
        day: day_number(key),
        date: key.to_string(),
    }
}
