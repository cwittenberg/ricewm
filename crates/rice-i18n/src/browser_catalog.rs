//! Bounded, immutable language packs uploaded by the browser on selection.
//! Published strings live as long as the Wasm instance, matching `t`'s borrowed
//! return value. Each of the finite locale codes can be installed only once.
use std::{cell::RefCell, collections::BTreeMap};
const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 16_384;
type Messages = &'static [(&'static str, &'static str)];
thread_local! {
    static CATALOGS: RefCell<BTreeMap<u8, Messages>> = const { RefCell::new(BTreeMap::new()) };
    static STAGING: RefCell<(u8, Vec<u8>)> = const { RefCell::new((0, Vec::new())) };
}
fn valid(locale: u8) -> bool { locale != 0 && crate::LOCALE_CODES.contains(&locale) }
pub fn loaded(locale: u8) -> bool { locale == 0 || CATALOGS.with(|catalogs| catalogs.borrow().contains_key(&locale)) }
pub fn lookup(message: &str, locale: u8) -> Option<&'static str> {
    CATALOGS.with(|catalogs| {
        let catalogs = catalogs.borrow();
        let entries = catalogs.get(&locale)?;
        entries.binary_search_by(|entry| entry.0.cmp(message)).ok().map(|index| entries[index].1)
    })
}
pub fn begin(locale: u32, bytes: u32) -> *mut u8 {
    let Ok(locale) = u8::try_from(locale) else { return std::ptr::null_mut(); };
    let bytes = bytes as usize;
    if !valid(locale) || !(4..=MAX_BYTES).contains(&bytes) { return std::ptr::null_mut(); }
    STAGING.with(|staging| {
        let mut staging = staging.borrow_mut();
        let mut buffer = Vec::new();
        if buffer.try_reserve_exact(bytes).is_err() { return std::ptr::null_mut(); }
        buffer.resize(bytes, 0);
        *staging = (locale, buffer);
        staging.1.as_mut_ptr()
    })
}
pub fn finish(locale: u32, bytes: u32) -> bool {
    let Ok(locale) = u8::try_from(locale) else { return false; };
    STAGING.with(|staging| {
        let (staged_locale, buffer) = std::mem::take(&mut *staging.borrow_mut());
        staged_locale == locale && buffer.len() == bytes as usize && install(locale, &buffer)
    })
}
fn install(locale: u8, bytes: &[u8]) -> bool {
    if !valid(locale) || !(4..=MAX_BYTES).contains(&bytes.len()) { return false; }
    let Some(entries) = decode(bytes) else { return false; };
    CATALOGS.with(|catalogs| {
        let mut catalogs = catalogs.borrow_mut();
        if catalogs.contains_key(&locale) { return true; }
        let entries: Vec<(&'static str, &'static str)> = entries.into_iter().map(|(key, value)| {
            (&*Box::leak(key.into_boxed_str()), &*Box::leak(value.into_boxed_str()))
        }).collect();
        catalogs.insert(locale, Box::leak(entries.into_boxed_slice()));
        true
    })
}
fn decode(mut bytes: &[u8]) -> Option<Vec<(String, String)>> {
    fn word(bytes: &mut &[u8]) -> Option<usize> {
        let value = u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) as usize;
        *bytes = &bytes[4..]; Some(value)
    }
    fn text(bytes: &mut &[u8]) -> Option<String> {
        let length = word(bytes)?;
        let value = std::str::from_utf8(bytes.get(..length)?).ok()?.to_owned();
        *bytes = &bytes[length..]; Some(value)
    }
    let count = word(&mut bytes)?;
    if count > MAX_ENTRIES { return None; }
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        let key = text(&mut bytes)?; let value = text(&mut bytes)?;
        if key.is_empty() || value.trim().is_empty() { return None; }
        entries.push((key, value));
    }
    if !bytes.is_empty() { return None; }
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    if entries.windows(2).any(|pair| pair[0].0 == pair[1].0) { return None; }
    Some(entries)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn pack(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut bytes = (entries.len() as u32).to_le_bytes().to_vec();
        for (key, value) in entries { for text in [key, value] {
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes()); bytes.extend_from_slice(text.as_bytes());
        }} bytes
    }
    #[test] fn installs_selected_language_once_and_retains_borrowed_strings() {
        let bytes = pack(&[("z", "fin"), ("Save", "Guardar"), ("common.save", "Guardar")]);
        assert!(install(1, &bytes)); let saved = lookup("Save", 1).unwrap();
        assert_eq!(lookup("common.save", 1), Some("Guardar")); assert_eq!(lookup("missing", 1), None);
        assert!(loaded(1)); assert!(!loaded(2));
        assert!(install(1, &pack(&[("Save", "changed")]))); assert_eq!(saved, "Guardar");
        assert_eq!(lookup("Save", 1), Some("Guardar"));
    }
    #[test] fn rejects_malformed_duplicate_and_unbounded_packs_before_publication() {
        for bytes in [vec![], vec![0, 0, 0], (MAX_ENTRIES as u32 + 1).to_le_bytes().to_vec(),
            pack(&[("x", "one"), ("x", "two")]), pack(&[("", "empty key")]), pack(&[("x", " ")]),
            { let mut p = pack(&[("x", "one")]); p.push(0); p },
            { let mut p = pack(&[("x", "one")]); p.pop(); p },
            { let mut p = pack(&[("x", "one")]); *p.last_mut().unwrap() = 255; p }] {
            assert!(!install(2, &bytes)); assert!(!loaded(2));
        }
        assert!(!install(255, &pack(&[("x", "one")])));
        assert!(begin(2, MAX_BYTES as u32 + 1).is_null()); assert!(begin(256, 4).is_null());
        let bytes = pack(&[("Save", "保存")]); let pointer = begin(2, bytes.len() as u32);
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len()); }
        assert!(finish(2, bytes.len() as u32)); assert_eq!(lookup("Save", 2), Some("保存"));
        assert!(!finish(2, bytes.len() as u32));
    }
}
