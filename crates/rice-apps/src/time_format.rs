//! Dependency-free second-precision timestamp labels shared by native applications.

pub(crate) fn modified_timestamp_label(unix_ms: i64) -> String {
    let days = unix_ms.div_euclid(86_400_000);
    let millis = unix_ms.rem_euclid(86_400_000);
    let hour = millis / 3_600_000;
    let minute = millis % 3_600_000 / 60_000;
    let second = millis % 60_000 / 1_000;
    let (year, month, day) = civil_from_days(days);
    // Keep the existing UTC interpretation; only the display pattern changes.
    i32::try_from(year).ok().and_then(|year| rice_i18n::format_date_time(
        year, month, day, hour as u8, minute as u8, second as u8,
    )).unwrap_or_else(|| format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"))
}

// Howard Hinnant's civil-from-days mapping, adapted to Unix epoch days.
fn civil_from_days(unix_days: i64) -> (i64, u8, u8) {
    let days = unix_days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        year,
        u8::try_from(month).unwrap_or(1),
        u8::try_from(day).unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::modified_timestamp_label;

    #[test]
    fn locale_changes_preserve_utc_and_handle_dates_before_the_epoch() {
        rice_i18n::with_locale(2, || assert_eq!(modified_timestamp_label(-1), "1969年12月31日 23:59:59"));
        rice_i18n::with_locale(1, || assert_eq!(modified_timestamp_label(-1), "31/12/1969, 23:59:59"));
        rice_i18n::with_locale(255, || assert_eq!(modified_timestamp_label(-1), "12/31/1969, 23:59:59"));
        for timestamp in [i64::MIN, i64::MAX] {
            assert!(!modified_timestamp_label(timestamp).is_empty());
        }
    }

    #[test]
    fn formats_a_clean_second_precision_timestamp() {
        assert_eq!(
            modified_timestamp_label(1_704_067_201_234),
            "1/1/2024, 00:00:01"
        );
    }
}
