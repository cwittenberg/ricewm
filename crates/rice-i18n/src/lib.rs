//! Application-owned UI messages shared with the browser catalogs.
//! Locale is scoped to a runtime dispatch, so independent runtimes and tests
//! cannot change each other's language. Never pass user content to `t`.
use std::cell::Cell;
mod catalog;
#[cfg(any(feature = "browser-catalogs", test))]
pub mod browser_catalog;
pub use catalog::{LANGUAGE_VALUES, LOCALE_CODES, language_name};
thread_local! { static LOCALE: Cell<u8> = const { Cell::new(0) }; }

/// Base direction of application-owned labels in the current locale.
/// Editable content and physical canvases require their own direction policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum TextDirection { #[default] Ltr = 0, Rtl = 1 }

#[must_use]
pub fn direction() -> TextDirection { catalog::direction(LOCALE.get()) }

pub fn locale_code(locale: &str) -> u8 {
    catalog::locale_code(locale)
}

/// Set the locale of a single-threaded display instance. App runtimes use scoped dispatch.
pub fn set_locale(locale: u8) { LOCALE.set(locale); }

pub fn with_locale<T>(locale: u8, action: impl FnOnce() -> T) -> T {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) { LOCALE.set(self.0); }
    }
    let _restore = Restore(LOCALE.replace(locale));
    action()
}

#[must_use]
pub fn t(message: &str) -> &str {
    let locale = LOCALE.get();
    catalog::lookup(message, locale)
        .or_else(|| catalog::lookup(message, 0))
        .unwrap_or(message)
}

/// Named placeholders are substituted in one pass; argument text is not parsed.
#[must_use]
pub fn format(message: &str, arguments: &[(&str, &str)]) -> String {
    let mut remaining = t(message);
    let mut result = String::new();
    while let Some(start) = remaining.find('{') {
        result.push_str(&remaining[..start]);
        let Some(end) = remaining[start..].find('}') else {
            result.push_str(&remaining[start..]);
            return result;
        };
        let name = &remaining[start + 1..start + end];
        result.push_str(arguments.iter().find(|(key, _)| *key == name)
            .map_or(&remaining[start..=start + end], |(_, value)| *value));
        remaining = &remaining[start + end + 1..];
    }
    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests;

/// Format host-local calendar components without changing their time zone or instant.
/// The catalog owns date order and separators; time uses an unambiguous 24-hour clock.
#[must_use]
pub fn format_date_time(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<String> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) || hour > 23 || minute > 59 || second > 59 { return None; }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month { 2 => if leap { 29 } else { 28 }, 4 | 6 | 9 | 11 => 30, _ => 31 };
    if day == 0 || day > days { return None; }
    Some(format("common.date-time", &[
        ("year", &year.to_string()), ("month", &month.to_string()), ("day", &day.to_string()),
        ("hour", &format!("{hour:02}")), ("minute", &format!("{minute:02}")), ("second", &format!("{second:02}")),
    ]))
}
