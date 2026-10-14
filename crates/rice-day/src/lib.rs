//! Domain model for the `RiceWM` Day calendar application.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split('-');
        let date = Self {
            year: parts.next()?.parse().ok()?,
            month: parts.next()?.parse().ok()?,
            day: parts.next()?.parse().ok()?,
        };
        (parts.next().is_none()
            && date.month > 0
            && date.month <= 12
            && date.day > 0
            && date.day <= days_in_month(date.year, date.month))
        .then_some(date)
    }

    #[must_use]
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    #[must_use]
    pub fn add_days(self, days: i32) -> Self {
        civil_from_days(days_from_civil(self).saturating_add(i64::from(days)))
    }

    #[must_use]
    pub fn weekday(self) -> usize {
        usize::try_from((days_from_civil(self) + 4).rem_euclid(7)).unwrap_or(0)
    }

    #[must_use]
    pub fn monday(self) -> Self {
        let from_monday = (self.weekday() + 6) % 7;
        self.add_days(-i32::try_from(from_monday).unwrap_or(0))
    }

    #[must_use]
    pub fn shift_month(self, delta: i32) -> Self {
        let total = self
            .year
            .saturating_mul(12)
            .saturating_add(i32::from(self.month) - 1)
            .saturating_add(delta);
        let year = total.div_euclid(12);
        let month = u8::try_from(total.rem_euclid(12) + 1).unwrap_or(1);
        Self {
            year,
            month,
            day: self.day.min(days_in_month(year, month)),
        }
    }

    /// ISO-8601 week number for this date (weeks start on Monday).
    #[must_use]
    pub fn iso_week(self) -> u8 {
        let week_monday = self.monday();
        let week_year = week_monday.add_days(3).year;
        let first_monday = Self {
            year: week_year,
            month: 1,
            day: 4,
        }
        .monday();
        u8::try_from((days_from_civil(week_monday) - days_from_civil(first_monday)) / 7 + 1)
            .unwrap_or(1)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Availability {
    Available,
    #[default]
    Busy,
    OutOfOffice,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ParticipationStatus {
    #[default]
    NeedsAction,
    Accepted,
    Tentative,
    Declined,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attendee {
    pub email: String,
    pub display_name: String,
    pub status: ParticipationStatus,
    pub organizer: bool,
}

impl Attendee {
    #[must_use]
    pub fn new(email: &str, display_name: &str) -> Option<Self> {
        let email = email.trim().to_ascii_lowercase();
        (email.len() <= 320 && email.contains('@') && !email.contains(['\0', '\r', '\n', ' ']))
            .then(|| Self {
                email,
                display_name: display_name
                    .chars()
                    .filter(|character| !character.is_control())
                    .take(160)
                    .collect(),
                status: ParticipationStatus::NeedsAction,
                organizer: false,
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecurrenceRule {
    pub frequency: String,
    pub interval: u16,
    pub count: Option<u16>,
    pub until: Option<Date>,
}

impl RecurrenceRule {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let mut frequency = None;
        let mut interval = 1;
        let mut count = None;
        let mut until = None;
        for field in value.trim().split(';') {
            let (name, value) = field.split_once('=')?;
            match name.to_ascii_uppercase().as_str() {
                "FREQ" if matches!(value, "DAILY" | "WEEKLY" | "MONTHLY" | "YEARLY") => {
                    frequency = Some(value.to_owned());
                }
                "INTERVAL" => {
                    interval = value
                        .parse()
                        .ok()
                        .filter(|value| (1..=365).contains(value))?
                }
                "COUNT" => {
                    count = Some(
                        value
                            .parse()
                            .ok()
                            .filter(|value| (1..=999).contains(value))?,
                    )
                }
                "UNTIL" => until = Date::parse(value),
                _ => return None,
            }
        }
        Some(Self {
            frequency: frequency?,
            interval,
            count,
            until,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EventIntegration {
    pub provider_id: String,
    pub calendar_id: String,
    pub calendar_name: String,
    pub timezone: String,
    pub revision: String,
    pub recurrence: Option<RecurrenceRule>,
    pub attendees: Vec<Attendee>,
    pub conference_url: String,
    pub ical_uid: String,
    pub html_link: String,
}

const MAX_ICALENDAR_BYTES: usize = 4 * 1024 * 1024;
const MAX_ICALENDAR_EVENTS: usize = 2_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedEvent {
    pub event: Event,
    pub integration: EventIntegration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IcalendarError {
    Oversized,
    InvalidUtf8,
    NoEvents,
}

impl EventIntegration {
    #[must_use]
    pub fn checked(
        calendar_id: &str,
        timezone: &str,
        revision: &str,
        conference_url: &str,
    ) -> Option<Self> {
        let timezone_ok = timezone == "UTC"
            || (timezone.len() <= 64
                && timezone.split('/').all(|part| {
                    !part.is_empty()
                        && part.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || matches!(character, '_' | '-' | '+')
                        })
                }));
        let conference_ok = conference_url.is_empty()
            || (conference_url.starts_with("https://") && conference_url.len() <= 2_048);
        (timezone_ok && conference_ok).then(|| Self {
            provider_id: String::new(),
            calendar_id: bounded(calendar_id, 256),
            calendar_name: String::new(),
            timezone: timezone.to_owned(),
            revision: bounded(revision, 256),
            recurrence: None,
            attendees: Vec::new(),
            conference_url: conference_url.to_owned(),
            ical_uid: String::new(),
            html_link: String::new(),
        })
    }
}

#[must_use]
pub fn conflicting_event_ids(events: &[Event]) -> Vec<(String, String)> {
    let mut conflicts = Vec::new();
    for (index, left) in events.iter().enumerate() {
        if left.availability == Availability::Available {
            continue;
        }
        for right in events.iter().skip(index + 1) {
            if right.availability == Availability::Available {
                continue;
            }
            let overlaps = left.start_date < right.end_date && right.start_date < left.end_date
                || left.start_date == right.start_date
                    && minutes(&left.start_time) < minutes(&right.end_time)
                    && minutes(&right.start_time) < minutes(&left.end_time);
            if overlaps {
                conflicts.push((left.id.clone(), right.id.clone()));
            }
        }
    }
    conflicts
}

fn bounded(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum)
        .collect()
}

impl Availability {
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "available" | "free" => Self::Available,
            "out-of-office" | "ooo" => Self::OutOfOffice,
            _ => Self::Busy,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Busy => "busy",
            Self::OutOfOffice => "out-of-office",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub id: String,
    pub start_date: Date,
    pub start_time: String,
    pub end_date: Date,
    pub end_time: String,
    pub reminder: u16,
    pub all_day: bool,
    pub availability: Availability,
    pub title: String,
    pub location: String,
    pub notes: String,
    pub source: String,
}

impl Event {
    #[must_use]
    pub fn starts_on(&self, date: Date) -> bool {
        self.start_date == date
    }

    #[must_use]
    pub fn intersects(&self, start: Date, end: Date) -> bool {
        self.start_date < end && self.end_date >= start
    }

    #[must_use]
    pub fn duration_minutes(&self) -> i64 {
        let days = days_from_civil(self.end_date) - days_from_civil(self.start_date);
        days.saturating_mul(1_440)
            .saturating_add(i64::from(
                minutes(&self.end_time) - minutes(&self.start_time),
            ))
            .max(1)
    }
}

/// Parse an inert, bounded RFC 5545 calendar stream for the manual Proton
/// Calendar path. Unknown properties and components are ignored; URLs remain
/// plain validated metadata and no attachment or external reference is loaded.
pub fn import_icalendar(bytes: &[u8]) -> Result<Vec<ImportedEvent>, IcalendarError> {
    if bytes.len() > MAX_ICALENDAR_BYTES {
        return Err(IcalendarError::Oversized);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| IcalendarError::InvalidUtf8)?;
    let lines = unfold_icalendar(text);
    let mut events = Vec::new();
    let mut fields: Option<Vec<(String, String, String)>> = None;
    for line in lines {
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            fields = Some(Vec::new());
            continue;
        }
        if line.eq_ignore_ascii_case("END:VEVENT") {
            if let Some(value) = fields.take()
                && let Some(event) = event_from_icalendar(&value, events.len())
            {
                events.push(event);
                if events.len() >= MAX_ICALENDAR_EVENTS {
                    break;
                }
            }
            continue;
        }
        let Some(event) = fields.as_mut() else {
            continue;
        };
        let Some((left, value)) = line.split_once(':') else {
            continue;
        };
        let name = left
            .split(';')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let parameters = left.get(name.len()..).unwrap_or_default().to_owned();
        event.push((name, parameters, icalendar_unescape(value)));
    }
    (!events.is_empty())
        .then_some(events)
        .ok_or(IcalendarError::NoEvents)
}

/// Export local events as a portable calendar accepted by Proton Calendar and
/// other RFC 5545 clients.
#[must_use]
pub fn export_icalendar(events: &[ImportedEvent]) -> Vec<u8> {
    let mut lines = vec![
        String::from("BEGIN:VCALENDAR"),
        String::from("VERSION:2.0"),
        String::from("PRODID:-//RiceWM//Rice Day//EN"),
        String::from("CALSCALE:GREGORIAN"),
    ];
    for imported in events.iter().take(MAX_ICALENDAR_EVENTS) {
        let event = &imported.event;
        lines.push(String::from("BEGIN:VEVENT"));
        lines.push(format!("UID:{}", icalendar_escape(&event.id)));
        if event.all_day {
            lines.push(format!(
                "DTSTART;VALUE=DATE:{}",
                compact_date(event.start_date)
            ));
            let exclusive = if event.end_date > event.start_date && event.end_time == "00:00" {
                event.end_date
            } else {
                event.end_date.add_days(1)
            };
            lines.push(format!("DTEND;VALUE=DATE:{}", compact_date(exclusive)));
        } else {
            let timezone = &imported.integration.timezone;
            let parameter = if timezone.is_empty() || timezone == "UTC" {
                String::new()
            } else {
                format!(";TZID={}", icalendar_escape(timezone))
            };
            lines.push(format!(
                "DTSTART{parameter}:{}T{}00",
                compact_date(event.start_date),
                compact_time(&event.start_time)
            ));
            lines.push(format!(
                "DTEND{parameter}:{}T{}00",
                compact_date(event.end_date),
                compact_time(&event.end_time)
            ));
        }
        lines.push(format!("SUMMARY:{}", icalendar_escape(&event.title)));
        if !event.location.is_empty() {
            lines.push(format!("LOCATION:{}", icalendar_escape(&event.location)));
        }
        if !event.notes.is_empty() {
            lines.push(format!("DESCRIPTION:{}", icalendar_escape(&event.notes)));
        }
        if event.availability == Availability::Available {
            lines.push(String::from("TRANSP:TRANSPARENT"));
        }
        if let Some(rule) = &imported.integration.recurrence {
            let mut value = format!("FREQ={};INTERVAL={}", rule.frequency, rule.interval);
            if let Some(count) = rule.count {
                value.push_str(&format!(";COUNT={count}"));
            }
            if let Some(until) = rule.until {
                value.push_str(&format!(";UNTIL={}", compact_date(until)));
            }
            lines.push(format!("RRULE:{value}"));
        }
        for attendee in imported.integration.attendees.iter().take(500) {
            let status = match attendee.status {
                ParticipationStatus::Accepted => "ACCEPTED",
                ParticipationStatus::Tentative => "TENTATIVE",
                ParticipationStatus::Declined => "DECLINED",
                ParticipationStatus::NeedsAction => "NEEDS-ACTION",
            };
            let property = if attendee.organizer {
                "ORGANIZER"
            } else {
                "ATTENDEE"
            };
            lines.push(format!(
                "{property};CN={};PARTSTAT={status}:mailto:{}",
                icalendar_escape(&attendee.display_name),
                attendee.email
            ));
        }
        if !imported.integration.conference_url.is_empty() {
            lines.push(format!(
                "URL:{}",
                icalendar_escape(&imported.integration.conference_url)
            ));
        }
        if !imported.integration.revision.is_empty() {
            lines.push(format!(
                "SEQUENCE:{}",
                icalendar_escape(&imported.integration.revision)
            ));
        }
        lines.push(String::from("END:VEVENT"));
    }
    lines.push(String::from("END:VCALENDAR"));
    let mut output = String::new();
    for line in lines {
        output.push_str(&fold_icalendar_line(&line));
        output.push_str("\r\n");
    }
    output.into_bytes()
}

fn event_from_icalendar(
    fields: &[(String, String, String)],
    index: usize,
) -> Option<ImportedEvent> {
    let first = |name: &str| fields.iter().find(|field| field.0 == name);
    let (start_date, start_time, all_day, timezone) = {
        let field = first("DTSTART")?;
        parse_icalendar_datetime(&field.1, &field.2)?
    };
    let (end_date, end_time) = first("DTEND")
        .and_then(|field| parse_icalendar_datetime(&field.1, &field.2))
        .map(|value| (value.0, value.1))
        .unwrap_or_else(|| {
            if all_day {
                (start_date.add_days(1), String::from("00:00"))
            } else {
                add_hour(start_date, &start_time)
            }
        });
    if (end_date, end_time.as_str()) <= (start_date, start_time.as_str()) {
        return None;
    }
    let id = first("UID")
        .map(|field| bounded(&field.2, 96))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("ics:{}", index + 1));
    let title = first("SUMMARY")
        .map(|field| bounded(&field.2, 160))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| String::from("Imported event"));
    let availability =
        if first("TRANSP").is_some_and(|field| field.2.eq_ignore_ascii_case("TRANSPARENT")) {
            Availability::Available
        } else if first("X-MICROSOFT-CDO-BUSYSTATUS")
            .is_some_and(|field| field.2.eq_ignore_ascii_case("OOF"))
        {
            Availability::OutOfOffice
        } else {
            Availability::Busy
        };
    let mut integration = EventIntegration::checked(
        "proton-manual",
        if timezone.is_empty() {
            "UTC"
        } else {
            &timezone
        },
        first("SEQUENCE").map_or("", |field| field.2.as_str()),
        first("URL").map_or("", |field| field.2.as_str()),
    )?;
    integration.recurrence = first("RRULE").and_then(|field| RecurrenceRule::parse(&field.2));
    integration.attendees = fields
        .iter()
        .filter(|field| matches!(field.0.as_str(), "ATTENDEE" | "ORGANIZER"))
        .filter_map(|field| attendee_from_icalendar(field))
        .take(500)
        .collect();
    integration.provider_id = id.clone();
    integration.calendar_name = String::from("Proton Calendar (manual)");
    integration.ical_uid = id.clone();
    Some(ImportedEvent {
        event: Event {
            id,
            start_date,
            start_time,
            end_date,
            end_time,
            reminder: 0,
            all_day,
            availability,
            title,
            location: first("LOCATION").map_or_else(String::new, |field| bounded(&field.2, 512)),
            notes: first("DESCRIPTION")
                .map_or_else(String::new, |field| bounded_multiline(&field.2, 64 * 1024)),
            source: String::from("proton-manual"),
        },
        integration,
    })
}

fn parse_icalendar_datetime(parameters: &str, value: &str) -> Option<(Date, String, bool, String)> {
    if parameters.to_ascii_uppercase().contains("VALUE=DATE") || value.len() == 8 {
        return Some((
            parse_compact_date(value)?,
            String::from("00:00"),
            true,
            String::from("UTC"),
        ));
    }
    let raw = value.trim_end_matches('Z');
    if raw.len() < 13 || raw.as_bytes().get(8) != Some(&b'T') {
        return None;
    }
    let time = format!("{}:{}", &raw[9..11], &raw[11..13]);
    if !valid_time(&time) {
        return None;
    }
    let timezone = if value.ends_with('Z') {
        String::from("UTC")
    } else {
        parameter_value(parameters, "TZID").unwrap_or_else(|| String::from("UTC"))
    };
    Some((
        parse_compact_date(&raw[..8])?,
        time,
        false,
        bounded(&timezone, 64),
    ))
}

fn attendee_from_icalendar(field: &(String, String, String)) -> Option<Attendee> {
    let email = field
        .2
        .strip_prefix("mailto:")
        .or_else(|| field.2.strip_prefix("MAILTO:"))
        .unwrap_or(&field.2);
    let mut attendee = Attendee::new(email, &parameter_value(&field.1, "CN").unwrap_or_default())?;
    attendee.organizer = field.0 == "ORGANIZER";
    attendee.status = match parameter_value(&field.1, "PARTSTAT")
        .unwrap_or_default()
        .to_ascii_uppercase()
        .as_str()
    {
        "ACCEPTED" => ParticipationStatus::Accepted,
        "TENTATIVE" => ParticipationStatus::Tentative,
        "DECLINED" => ParticipationStatus::Declined,
        _ => ParticipationStatus::NeedsAction,
    };
    Some(attendee)
}

fn parameter_value(parameters: &str, name: &str) -> Option<String> {
    parameters
        .trim_start_matches(';')
        .split(';')
        .find_map(|parameter| {
            let (key, value) = parameter.split_once('=')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim_matches('"').to_owned())
        })
}

fn parse_compact_date(value: &str) -> Option<Date> {
    (value.len() >= 8)
        .then(|| format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..8]))
        .and_then(|value| Date::parse(&value))
}

fn compact_date(date: Date) -> String {
    format!("{:04}{:02}{:02}", date.year, date.month, date.day)
}
fn compact_time(value: &str) -> String {
    value.replace(':', "")
}

fn add_hour(date: Date, time: &str) -> (Date, String) {
    let hour = time
        .get(..2)
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    let minute = time.get(3..).unwrap_or("00");
    if hour >= 23 {
        (date.add_days(1), format!("00:{minute}"))
    } else {
        (date, format!("{:02}:{minute}", hour + 1))
    }
}

fn unfold_icalendar(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if raw.starts_with([' ', '\t']) {
            if let Some(previous) = lines.last_mut() {
                previous.push_str(raw.trim_start_matches([' ', '\t']));
            }
        } else {
            lines.push(raw.to_owned());
        }
    }
    lines
}

fn icalendar_unescape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => output.push('\n'),
            Some('\\') => output.push('\\'),
            Some(',') => output.push(','),
            Some(';') => output.push(';'),
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }
    output
}

fn icalendar_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\r', "")
}

fn bounded_multiline(value: &str, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect()
}

fn fold_icalendar_line(line: &str) -> String {
    let mut output = String::new();
    let mut bytes = 0;
    for character in line.chars() {
        let width = character.len_utf8();
        if bytes > 0 && bytes + width > 75 {
            output.push_str("\r\n ");
            bytes = 1;
        }
        output.push(character);
        bytes += width;
    }
    output
}

#[must_use]
pub fn valid_time(value: &str) -> bool {
    value.len() == 5
        && value.as_bytes()[2] == b':'
        && value[..2].parse::<u8>().is_ok_and(|hour| hour < 24)
        && value[3..].parse::<u8>().is_ok_and(|minute| minute < 60)
}

fn minutes(value: &str) -> i32 {
    i32::from(value[..2].parse::<u8>().unwrap_or(0)) * 60
        + i32::from(value[3..].parse::<u8>().unwrap_or(0))
}

fn leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Proleptic-Gregorian civil date conversion, anchored at the Unix epoch.
#[must_use]
pub fn days_from_civil(date: Date) -> i64 {
    let mut year = i64::from(date.year);
    let month = i64::from(date.month);
    let day = i64::from(date.day);
    year -= i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let shifted = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted + 2) / 5 + day - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

#[must_use]
pub fn civil_from_days(days: i64) -> Date {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    Date {
        year: i32::try_from(year).unwrap_or(1970),
        month: u8::try_from(month).unwrap_or(1),
        day: u8::try_from(day).unwrap_or(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip_and_week_starts_monday() {
        let date = Date::parse("2026-08-09").unwrap();
        assert_eq!(civil_from_days(days_from_civil(date)), date);
        assert_eq!(date.monday().iso(), "2026-08-03");
        assert_eq!(date.add_days(1).iso(), "2026-08-10");
        assert_eq!(
            Date::parse("2024-02-29").unwrap().add_days(1).iso(),
            "2024-03-01"
        );
    }

    #[test]
    fn multi_day_duration_is_preserved() {
        let event = Event {
            id: "1".into(),
            start_date: Date::parse("2026-08-09").unwrap(),
            start_time: "23:30".into(),
            end_date: Date::parse("2026-08-10").unwrap(),
            end_time: "01:00".into(),
            reminder: 15,
            all_day: false,
            availability: Availability::Busy,
            title: "Deploy".into(),
            location: "Operations room".into(),
            notes: String::new(),
            source: "day".into(),
        };
        assert_eq!(event.duration_minutes(), 90);
    }

    #[test]
    fn iso_weeks_cross_calendar_years_correctly() {
        assert_eq!(Date::parse("2026-08-09").unwrap().iso_week(), 32);
        assert_eq!(Date::parse("2021-01-01").unwrap().iso_week(), 53);
        assert_eq!(Date::parse("2021-01-04").unwrap().iso_week(), 1);
    }

    #[test]
    fn provider_event_extensions_validate_recurrence_attendees_and_conferencing() {
        let recurrence = RecurrenceRule::parse("FREQ=WEEKLY;INTERVAL=2;COUNT=12").unwrap();
        assert_eq!(recurrence.frequency, "WEEKLY");
        assert_eq!(recurrence.interval, 2);
        assert!(RecurrenceRule::parse("FREQ=SECONDLY").is_none());
        assert!(
            Attendee::new("USER@Example.test", "Ada Lovelace")
                .is_some_and(|attendee| { attendee.email == "user@example.test" })
        );
        assert!(
            EventIntegration::checked(
                "calendar@example.test",
                "Europe/Amsterdam",
                "etag-7",
                "https://meet.example.test/room",
            )
            .is_some()
        );
        assert!(EventIntegration::checked("calendar", "../UTC", "7", "javascript:bad").is_none());
    }

    #[test]
    fn busy_overlaps_are_reported_without_flagging_free_events() {
        let event = |id: &str, start: &str, end: &str, availability| Event {
            id: id.into(),
            start_date: Date::parse("2026-08-13").unwrap(),
            start_time: start.into(),
            end_date: Date::parse("2026-08-13").unwrap(),
            end_time: end.into(),
            reminder: 0,
            all_day: false,
            availability,
            title: id.into(),
            location: String::new(),
            notes: String::new(),
            source: "day".into(),
        };
        let conflicts = conflicting_event_ids(&[
            event("one", "09:00", "10:00", Availability::Busy),
            event("two", "09:30", "10:30", Availability::Busy),
            event("free", "09:15", "09:45", Availability::Available),
        ]);
        assert_eq!(conflicts, vec![(String::from("one"), String::from("two"))]);
    }

    #[test]
    fn proton_style_icalendar_import_preserves_manual_event_metadata() {
        let input = b"BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:proton-event-1\r\nDTSTART;TZID=Europe/Amsterdam:20260813T093000\r\nDTEND;TZID=Europe/Amsterdam:20260813T104500\r\nSUMMARY:Roadmap review\r\nDESCRIPTION:Line one\\nLine two\r\nLOCATION:Studio 4B\r\nRRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=3\r\nATTENDEE;CN=Ada Lovelace;PARTSTAT=ACCEPTED:mailto:ada@example.test\r\nURL:https://meet.example.test/room\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let imported = import_icalendar(input).expect("calendar import");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].event.start_time, "09:30");
        assert_eq!(imported[0].event.notes, "Line one\nLine two");
        assert_eq!(imported[0].integration.timezone, "Europe/Amsterdam");
        assert_eq!(
            imported[0].integration.attendees[0].status,
            ParticipationStatus::Accepted
        );
        let exported = export_icalendar(&imported);
        let round_trip = import_icalendar(&exported).expect("calendar round trip");
        assert_eq!(round_trip[0].event, imported[0].event);
        assert_eq!(
            round_trip[0].integration.recurrence,
            imported[0].integration.recurrence
        );
    }

    #[test]
    fn all_day_icalendar_uses_exclusive_end_and_rejects_oversized_input() {
        let imported = import_icalendar(b"BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:all-day\r\nDTSTART;VALUE=DATE:20260813\r\nDTEND;VALUE=DATE:20260814\r\nSUMMARY:Holiday\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n").unwrap();
        assert!(imported[0].event.all_day);
        assert_eq!(imported[0].event.end_date.iso(), "2026-08-14");
        assert_eq!(
            import_icalendar(&vec![b'x'; MAX_ICALENDAR_BYTES + 1]),
            Err(IcalendarError::Oversized)
        );
    }
}
