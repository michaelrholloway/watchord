//! ISO-8601 instants, the way Foundation's `.iso8601` strategy writes and
//! reads them: `2026-08-28T18:00:00Z`, whole seconds, UTC.
//!
//! Hand-rolled rather than a date crate because the contract is exactly one
//! shape. Reading is a little wider than writing — a numeric offset
//! (`+01:00`) and fractional seconds are accepted — so a file touched by
//! another tool still loads.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// `2026-08-28T18:00:00Z`. Sub-second precision is dropped, as Foundation does.
pub fn format(instant: SystemTime) -> String {
    let (date, time) = civil(instant);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        date.0, date.1, date.2, time.0, time.1, time.2
    )
}

/// `2026-08-28T180000`, for a filename: no colons.
pub fn quarantine_stamp(instant: SystemTime) -> String {
    let (date, time) = civil(instant);
    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}",
        date.0, date.1, date.2, time.0, time.1, time.2
    )
}

/// Parses `YYYY-MM-DDTHH:MM:SS` followed by `Z` or `±HH:MM`, with optional
/// fractional seconds. `None` for anything else.
pub fn parse(text: &str) -> Option<SystemTime> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let digits = |from: usize, len: usize| -> Option<i64> {
        let slice = text.get(from..from + len)?;
        if !slice.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        slice.parse().ok()
    };
    let expect = |at: usize, ch: u8| -> Option<()> { (bytes.get(at) == Some(&ch)).then_some(()) };

    let year = digits(0, 4)?;
    expect(4, b'-')?;
    let month = digits(5, 2)?;
    expect(7, b'-')?;
    let day = digits(8, 2)?;
    expect(10, b'T')?;
    let hour = digits(11, 2)?;
    expect(13, b':')?;
    let minute = digits(14, 2)?;
    expect(16, b':')?;
    let second = digits(17, 2)?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let mut rest = 19;
    if bytes.get(rest) == Some(&b'.') {
        rest += 1;
        let start = rest;
        while bytes.get(rest).is_some_and(u8::is_ascii_digit) {
            rest += 1;
        }
        if rest == start {
            return None;
        }
    }
    let offset_seconds: i64 = match bytes.get(rest) {
        Some(b'Z') if rest + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-')) if rest + 6 == bytes.len() => {
            let oh = digits(rest + 1, 2)?;
            expect(rest + 3, b':')?;
            let om = digits(rest + 4, 2)?;
            let magnitude = oh * 3600 + om * 60;
            if *sign == b'+' { magnitude } else { -magnitude }
        }
        _ => return None,
    };

    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds;
    if seconds < 0 {
        return None;
    }
    Some(UNIX_EPOCH + Duration::from_secs(seconds as u64))
}

fn civil(instant: SystemTime) -> ((i64, i64, i64), (i64, i64, i64)) {
    let seconds = instant
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    (
        civil_from_days(days),
        (of_day / 3600, (of_day % 3600) / 60, of_day % 60),
    )
}

/// Howard Hinnant's algorithm: days since 1970-01-01 to (year, month, day).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The inverse: (year, month, day) to days since 1970-01-01.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_writes_foundations_shape() {
        let instant = UNIX_EPOCH + Duration::from_secs(1_770_000_000);
        assert_eq!(format(instant), "2026-02-02T02:40:00Z");
    }

    #[test]
    fn parse_reads_what_format_wrote() {
        let instant = UNIX_EPOCH + Duration::from_secs(1_760_000_000);
        assert_eq!(parse(&format(instant)), Some(instant));
    }

    #[test]
    fn parse_reads_note_views_real_stamps() {
        assert_eq!(
            parse("2026-08-17T00:33:32Z"),
            Some(UNIX_EPOCH + Duration::from_secs(1_786_840_412))
        );
    }

    #[test]
    fn parse_accepts_an_offset_and_fractions() {
        let utc = parse("2026-08-10T14:00:00Z").unwrap();
        assert_eq!(parse("2026-08-10T15:00:00+01:00"), Some(utc));
        assert_eq!(parse("2026-08-10T14:00:00.250Z"), Some(utc));
    }

    #[test]
    fn parse_refuses_the_malformed() {
        for bad in [
            "",
            "2026-08-10",
            "2026-08-10T14:00:00",
            "2026-13-10T14:00:00Z",
            "2026-08-10 14:00:00Z",
            "not a date",
            "2026-08-10T14:00:00Zx",
        ] {
            assert_eq!(parse(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn quarantine_stamp_has_no_colons() {
        let instant = UNIX_EPOCH + Duration::from_secs(1_770_000_000);
        assert_eq!(quarantine_stamp(instant), "2026-02-02T024000");
    }

    #[test]
    fn civil_round_trips_across_leap_years() {
        for days in [-1, 0, 59, 60, 365, 11_016, 20_000, 30_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
}
