//! Calendar arithmetic and daily board properties (the JS daily assertions).

use nexus_engine::collatz::{count_tiles, solve};
use nexus_engine::daily::{daily, date_key, day_number, shift_date_key, DAILY_LEVEL};

#[test]
fn day_numbers() {
    assert_eq!(day_number("2026-10-18"), 1);
    assert_eq!(day_number("2026-10-19"), 2);
    assert_eq!(day_number("2027-01-01"), 76);
    assert_eq!(day_number("2026-10-17"), 0);
    // JS dayNumber values via Date.UTC for leap and century edge cases.
    assert_eq!(day_number("2024-02-29"), -961);
    assert_eq!(day_number("2000-01-01"), -9786);
    assert_eq!(day_number("2100-03-01"), 26798);
    assert_eq!(DAILY_LEVEL, 8);
}

#[test]
fn date_keys() {
    assert_eq!(date_key(2026, 1, 5), "2026-01-05");
    assert_eq!(date_key(2026, 10, 18), "2026-10-18");
    assert_eq!(date_key(2026, 12, 31), "2026-12-31");
    let key = date_key(2027, 3, 1);
    assert_eq!(key.len(), 10);
    let bytes = key.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if i == 4 || i == 7 {
            assert_eq!(b, b'-');
        } else {
            assert!(b.is_ascii_digit());
        }
    }
}

#[test]
fn shifting_keys() {
    assert_eq!(shift_date_key("2026-10-18", 0), "2026-10-18");
    assert_eq!(shift_date_key("2026-10-18", 1), "2026-10-19");
    assert_eq!(shift_date_key("2026-10-18", -1), "2026-10-17");
    assert_eq!(shift_date_key("2026-10-31", 1), "2026-11-01");
    assert_eq!(shift_date_key("2026-12-31", 1), "2027-01-01");
    assert_eq!(shift_date_key("2027-01-01", -1), "2026-12-31");
    assert_eq!(shift_date_key("2028-02-28", 1), "2028-02-29");
    assert_eq!(shift_date_key("2028-03-01", -1), "2028-02-29");
    assert_eq!(shift_date_key("2027-03-01", -1), "2027-02-28");
    assert_eq!(shift_date_key("2026-10-18", 75), "2027-01-01");
    for n in -800..800 {
        let key = shift_date_key("2026-10-18", n);
        assert_eq!(day_number(&key), n + 1, "{key}");
        assert_eq!(shift_date_key(&key, -n), "2026-10-18");
    }
}

#[test]
fn daily_boards() {
    let d1 = daily("2026-10-18");
    let d2 = daily("2026-10-18");
    let d3 = daily("2026-10-19");
    assert_eq!(
        d1.board.cells, d2.board.cells,
        "same date gives the same board"
    );
    assert_ne!(
        d1.board.cells, d3.board.cells,
        "different dates give different boards"
    );
    assert_eq!(d1.day, 1);
    assert_eq!(d1.date, "2026-10-18");
    assert_eq!(d3.day, 2);
    assert_eq!(count_tiles(&d1.board.cells), 12);
    assert!(d1.board.par > 0);
    assert_eq!(
        solve(&d1.board.cells, 0).moves.len() as u32,
        d1.board.par,
        "daily par is exact"
    );
}
