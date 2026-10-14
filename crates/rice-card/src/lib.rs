//! Provider-neutral contacts and ranked recipient suggestions.

use serde::{Deserialize, Serialize};

const MAX_EXCHANGE_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONTACTS: usize = 50_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VcardError {
    Oversized,
    InvalidUtf8,
    NoContacts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    pub id: String,
    pub display_name: String,
    pub emails: Vec<String>,
    pub phone: String,
    pub organization: String,
    pub notes: String,
    pub provider_revision: String,
}

impl Contact {
    pub fn checked(mut self) -> Option<Self> {
        self.id = safe(self.id, 128);
        self.display_name = safe(self.display_name, 160);
        self.phone = safe(self.phone, 80);
        self.organization = safe(self.organization, 160);
        self.notes = safe_multiline(self.notes, 4_096);
        self.provider_revision = safe(self.provider_revision, 256);
        self.emails = self
            .emails
            .into_iter()
            .map(|email| email.trim().to_ascii_lowercase())
            .filter(|email| valid_email(email))
            .take(20)
            .collect();
        (!self.id.is_empty() && (!self.display_name.is_empty() || !self.emails.is_empty()))
            .then_some(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suggestion {
    pub contact_id: String,
    pub display_name: String,
    pub email: String,
    pub score: u32,
}

#[must_use]
pub fn ranked_suggestions(
    contacts: &[Contact],
    recent: &[(String, u32)],
    query: &str,
    limit: usize,
) -> Vec<Suggestion> {
    let query = query.trim().to_ascii_lowercase();
    let mut suggestions = contacts
        .iter()
        .flat_map(|contact| {
            contact.emails.iter().filter_map(|email| {
                let name = contact.display_name.to_ascii_lowercase();
                let email_lower = email.to_ascii_lowercase();
                let textual = if query.is_empty() {
                    1
                } else if email_lower.starts_with(&query) || name.starts_with(&query) {
                    1_000
                } else if email_lower.contains(&query) || name.contains(&query) {
                    500
                } else {
                    return None;
                };
                let recency = recent
                    .iter()
                    .find(|(candidate, _)| candidate.eq_ignore_ascii_case(email))
                    .map_or(0, |(_, score)| (*score).min(10_000));
                Some(Suggestion {
                    contact_id: contact.id.clone(),
                    display_name: contact.display_name.clone(),
                    email: email.clone(),
                    score: textual + recency,
                })
            })
        })
        .collect::<Vec<_>>();
    suggestions.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.email.cmp(&right.email))
    });
    suggestions.truncate(limit.clamp(1, 20));
    suggestions
}

#[must_use]
pub fn encode_contacts(contacts: &[Contact]) -> String {
    serde_json::to_vec(&contacts.iter().take(MAX_CONTACTS).collect::<Vec<_>>())
        .map(|bytes| bytes.iter().map(|byte| format!("{byte:02x}")).collect())
        .unwrap_or_default()
}

#[must_use]
pub fn decode_contacts(value: &str) -> Vec<Contact> {
    if value.len() > 2 * 1024 * 1024 || !value.len().is_multiple_of(2) {
        return Vec::new();
    }
    let bytes = value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            core::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
        })
        .collect::<Option<Vec<_>>>();
    bytes
        .and_then(|bytes| serde_json::from_slice::<Vec<Contact>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(Contact::checked)
        .take(MAX_CONTACTS)
        .collect()
}

/// Parse a bounded vCard 3/4 address-book export without executing or loading
/// any referenced content. Invalid cards are skipped; a completely unusable
/// file is rejected so the UI can explain the failure instead of importing an
/// empty address book.
pub fn import_vcards(bytes: &[u8]) -> Result<Vec<Contact>, VcardError> {
    if bytes.len() > MAX_EXCHANGE_BYTES {
        return Err(VcardError::Oversized);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| VcardError::InvalidUtf8)?;
    let lines = unfold_lines(text);
    let mut contacts = Vec::new();
    let mut card: Option<Vec<(String, String, String)>> = None;
    for line in lines {
        if line.eq_ignore_ascii_case("BEGIN:VCARD") {
            card = Some(Vec::new());
            continue;
        }
        if line.eq_ignore_ascii_case("END:VCARD") {
            if let Some(fields) = card.take()
                && let Some(contact) = contact_from_vcard(&fields, contacts.len())
            {
                contacts.push(contact);
                if contacts.len() >= MAX_CONTACTS {
                    break;
                }
            }
            continue;
        }
        let Some(fields) = card.as_mut() else {
            continue;
        };
        let Some((left, raw)) = line.split_once(':') else {
            continue;
        };
        let property = left
            .split(';')
            .next()
            .unwrap_or_default()
            .rsplit('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let parameters = left[property.len()..].to_ascii_uppercase();
        let value = decode_vcard_value(raw, &parameters);
        fields.push((property, parameters, value));
    }
    (!contacts.is_empty())
        .then_some(contacts)
        .ok_or(VcardError::NoContacts)
}

/// Serialize contacts as a standards-oriented vCard 4.0 stream suitable for
/// Proton Contacts' manual import path.
#[must_use]
pub fn export_vcards(contacts: &[Contact]) -> Vec<u8> {
    let mut output = String::new();
    for contact in contacts
        .iter()
        .take(MAX_CONTACTS)
        .filter_map(|value| value.clone().checked())
    {
        output.push_str("BEGIN:VCARD\r\nVERSION:4.0\r\n");
        output.push_str("UID:");
        output.push_str(&vcard_escape(&contact.id));
        output.push_str("\r\nFN:");
        output.push_str(&vcard_escape(if contact.display_name.is_empty() {
            contact.emails.first().map_or("Contact", String::as_str)
        } else {
            &contact.display_name
        }));
        output.push_str("\r\n");
        for email in &contact.emails {
            output.push_str("EMAIL:");
            output.push_str(&vcard_escape(email));
            output.push_str("\r\n");
        }
        if !contact.phone.is_empty() {
            output.push_str("TEL:");
            output.push_str(&vcard_escape(&contact.phone));
            output.push_str("\r\n");
        }
        if !contact.organization.is_empty() {
            output.push_str("ORG:");
            output.push_str(&vcard_escape(&contact.organization));
            output.push_str("\r\n");
        }
        if !contact.notes.is_empty() {
            output.push_str("NOTE:");
            output.push_str(&vcard_escape(&contact.notes));
            output.push_str("\r\n");
        }
        if !contact.provider_revision.is_empty() {
            output.push_str("REV:");
            output.push_str(&vcard_escape(&contact.provider_revision));
            output.push_str("\r\n");
        }
        output.push_str("END:VCARD\r\n");
    }
    output.into_bytes()
}

fn unfold_lines(text: &str) -> Vec<String> {
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

fn decode_vcard_value(raw: &str, parameters: &str) -> String {
    let decoded = if parameters.contains("ENCODING=B") || parameters.contains("ENCODING=BASE64") {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(raw.trim())
            .ok()
            .and_then(|value| String::from_utf8(value).ok())
            .unwrap_or_default()
    } else if parameters.contains("QUOTED-PRINTABLE") {
        decode_quoted_printable(raw)
    } else {
        raw.to_owned()
    };
    vcard_unescape(&decoded)
}

fn decode_quoted_printable(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'='
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2]))
        {
            output.push((high << 4) | low);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

const fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn contact_from_vcard(fields: &[(String, String, String)], index: usize) -> Option<Contact> {
    let get = |name: &str| {
        fields
            .iter()
            .find(|field| field.0 == name)
            .map(|field| field.2.clone())
    };
    let mut display_name = get("FN").unwrap_or_default();
    if display_name.is_empty() {
        display_name = get("N")
            .map(|value| {
                value
                    .split(';')
                    .rev()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
    }
    let emails = fields
        .iter()
        .filter(|field| field.0 == "EMAIL")
        .map(|field| field.2.clone())
        .collect();
    let id = get("UID")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("vcf:{}", index + 1));
    Contact {
        id,
        display_name,
        emails,
        phone: get("TEL").unwrap_or_default(),
        organization: get("ORG")
            .map(|value| value.replace(';', " "))
            .unwrap_or_default(),
        notes: get("NOTE").unwrap_or_default(),
        provider_revision: get("REV").unwrap_or_default(),
    }
    .checked()
}

fn vcard_unescape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match characters.next() {
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

fn vcard_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}

fn valid_email(value: &str) -> bool {
    value.len() <= 320 && value.contains('@') && !value.contains(['\0', '\r', '\n', ' ', ',', ';'])
}

fn safe(value: String, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn safe_multiline(value: String, maximum: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .take(maximum)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contacts_round_trip_and_rank_exact_prefix_plus_recency() {
        let contacts = vec![
            Contact {
                id: "people/ada".into(),
                display_name: "Ada Lovelace".into(),
                emails: vec!["ADA@Example.test".into()],
                phone: String::new(),
                organization: "Analytical Engines".into(),
                notes: String::new(),
                provider_revision: "etag-1".into(),
            }
            .checked()
            .unwrap(),
        ];
        assert_eq!(decode_contacts(&encode_contacts(&contacts)), contacts);
        let suggestions = ranked_suggestions(
            &contacts,
            &[(String::from("ada@example.test"), 50)],
            "ada",
            8,
        );
        assert_eq!(suggestions[0].score, 1_050);
        assert_eq!(suggestions[0].email, "ada@example.test");
    }

    #[test]
    fn proton_style_vcards_import_and_export_without_active_content() {
        let input = b"BEGIN:VCARD\r\nVERSION:3.0\r\nUID:proton-1\r\nFN:Ada \r\n Lovelace\r\nEMAIL;TYPE=HOME:ADA@example.test\r\nORG:Analytical\\; Engines\r\nNOTE:Line one\\nLine two\r\nEND:VCARD\r\n";
        let contacts = import_vcards(input).expect("vCard import");
        assert_eq!(contacts.len(), 1);
        assert_eq!(contacts[0].emails, ["ada@example.test"]);
        assert_eq!(contacts[0].notes, "Line one\nLine two");
        let exported = export_vcards(&contacts);
        let round_trip = import_vcards(&exported).expect("vCard round trip");
        assert_eq!(round_trip, contacts);
    }

    #[test]
    fn vcard_import_rejects_oversized_or_empty_input() {
        assert_eq!(
            import_vcards(&vec![b'x'; MAX_EXCHANGE_BYTES + 1]),
            Err(VcardError::Oversized)
        );
        assert_eq!(import_vcards(b"not a vcard"), Err(VcardError::NoContacts));
    }
}
