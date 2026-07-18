//! Strict RFC 3339 **Zulu** instants (annexe A.1: `instants RFC 3339 Zulu`).
//!
//! No time crate: the protocol compares Zulu strings chronologically
//! (house convention, `mandate.rs`); the only arithmetic the store needs is
//! the ±300 s skew window of A.2 #5, so instants parse to epoch
//! milliseconds here and nowhere else. Fail-closed: anything that is not
//! `YYYY-MM-DDTHH:MM:SS[.fff…]Z` is rejected — no offsets, no lowercase
//! markers, no leniency.

/// Parse a strict RFC 3339 Zulu instant to epoch milliseconds.
pub fn parse_rfc3339z_ms(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    if *b.last()? != b'Z' {
        return None;
    }
    let digits = |r: core::ops::Range<usize>| -> Option<i64> {
        let mut v: i64 = 0;
        for &c in &b[r] {
            if !c.is_ascii_digit() {
                return None;
            }
            v = v * 10 + i64::from(c - b'0');
        }
        Some(v)
    };
    let year = digits(0..4)?;
    let month = digits(5..7)?;
    let day = digits(8..10)?;
    let hour = digits(11..13)?;
    let minute = digits(14..16)?;
    let second = digits(17..19)?;
    // Optional fractional seconds: '.' then 1..=9 digits, then the final 'Z'.
    let mut millis: i64 = 0;
    if b.len() > 20 {
        if b[19] != b'.' || b.len() < 22 || b.len() > 30 {
            return None;
        }
        let frac = &b[20..b.len() - 1];
        if frac.is_empty() || !frac.iter().all(u8::is_ascii_digit) {
            return None;
        }
        for (i, &c) in frac.iter().take(3).enumerate() {
            millis += i64::from(c - b'0') * [100, 10, 1][i];
        }
    }
    if !(1..=12).contains(&month) || !(0..=23).contains(&hour) || !(0..=59).contains(&minute)
        // Leap seconds are rejected: the protocol never emits second 60.
        || !(0..=59).contains(&second)
    {
        return None;
    }
    if day < 1 || day > days_in_month(year, month) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some((((days * 24 + hour) * 60 + minute) * 60 + second) * 1000 + millis)
}

/// Render epoch milliseconds as an RFC 3339 Zulu instant, second precision
/// (the error-body `at` of annexe A.7).
pub fn render_rfc3339z(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

/// Howard Hinnant's `days_from_civil`: days since 1970-01-01.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_vectors_instants() {
        assert_eq!(parse_rfc3339z_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339z_ms("1970-01-01T00:05:01Z"), Some(301_000));
        // 2026-07-16T12:00:00Z, cross-checked with Python
        // datetime(2026,7,16,12, tzinfo=UTC).timestamp() == 1784203200.
        assert_eq!(
            parse_rfc3339z_ms("2026-07-16T12:00:00Z"),
            Some(1_784_203_200_000)
        );
        // The exact 300 s and 301 s deltas of p1 (annexe A.2 #5).
        let at = parse_rfc3339z_ms("2026-07-16T12:00:00Z").unwrap();
        let boundary = parse_rfc3339z_ms("2026-07-16T12:05:00Z").unwrap();
        let beyond = parse_rfc3339z_ms("2026-07-16T12:05:01Z").unwrap();
        assert_eq!(boundary - at, 300_000);
        assert_eq!(beyond - at, 301_000);
        // Fractional seconds are accepted and truncated to milliseconds.
        assert_eq!(parse_rfc3339z_ms("1970-01-01T00:00:00.5Z"), Some(500));
        assert_eq!(
            parse_rfc3339z_ms("1970-01-01T00:00:00.123456789Z"),
            Some(123)
        );
        // Leap day.
        assert!(parse_rfc3339z_ms("2028-02-29T00:00:00Z").is_some());
        assert!(parse_rfc3339z_ms("2026-02-29T00:00:00Z").is_none());
    }

    #[test]
    fn rejects_everything_else_fail_closed() {
        for bad in [
            "",
            "2026-07-16",
            "2026-07-16T12:00:00",       // no Z
            "2026-07-16T12:00:00+00:00", // offset form
            "2026-07-16t12:00:00Z",      // lowercase marker
            "2026-07-16 12:00:00Z",      // space separator
            "2026-13-01T00:00:00Z",      // month 13
            "2026-00-10T00:00:00Z",      // month 0
            "2026-04-31T00:00:00Z",      // April 31
            "2026-07-16T24:00:00Z",      // hour 24
            "2026-07-16T12:60:00Z",      // minute 60
            "2026-07-16T12:00:60Z",      // leap second
            "2026-07-16T12:00:00.Z",     // empty fraction
            "2026-07-16T12:00:00.12a4Z", // non-digit fraction
            "202six-07-16T12:00:00Z",
        ] {
            assert!(parse_rfc3339z_ms(bad).is_none(), "should reject: {bad}");
        }
    }

    #[test]
    fn renders_back_to_zulu_seconds() {
        for s in [
            "1970-01-01T00:00:00Z",
            "2026-07-16T12:05:01Z",
            "2028-02-29T23:59:59Z",
        ] {
            assert_eq!(render_rfc3339z(parse_rfc3339z_ms(s).unwrap()), s);
        }
    }
}
