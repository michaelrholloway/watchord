//! A timestamp for the All Notes `when` column: `2026-08-28 13:05`, in local
//! time where the platform can say what that is.

use std::time::{SystemTime, UNIX_EPOCH};

/// `2026-08-28 13:05`. Local time on Unix; UTC elsewhere.
pub fn format(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let local = seconds + offset_seconds(seconds);
    let days = local.div_euclid(86_400);
    let rest = local.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60
    )
}

#[cfg(unix)]
fn offset_seconds(unix: i64) -> i64 {
    let mut out: libc::tm = unsafe { std::mem::zeroed() };
    let t: libc::time_t = unix as libc::time_t;
    // SAFETY: `localtime_r` writes into the `tm` we own and reads `t` by pointer;
    // both live for the whole call.
    let written = unsafe { libc::localtime_r(&t, &mut out) };
    if written.is_null() {
        0
    } else {
        out.tm_gmtoff as i64
    }
}

#[cfg(not(unix))]
fn offset_seconds(_unix: i64) -> i64 {
    0
}

/// Howard Hinnant's `civil_from_days`: days since 1970-01-01 to (y, m, d).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_is_1970_01_01() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn a_known_day_round_trips() {
        // 2026-08-28 is 20_693 days after the epoch.
        assert_eq!(civil_from_days(20_693), (2026, 8, 28));
    }
}
