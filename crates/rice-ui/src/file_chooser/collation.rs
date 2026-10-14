//! Small deterministic root-locale collation for the Wasm UI.
//!
//! This intentionally avoids host locale APIs: identical directory data must
//! sort identically in browsers, workers and native tests. Latin diacritics
//! receive their base-letter primary weight, Unicode lowercase is used for
//! other scripts, digit runs sort numerically, and original text is the final
//! stable tie-breaker.

use core::cmp::Ordering;

pub(super) fn compare(left: &str, right: &str) -> Ordering {
    primary_compare(left, right)
        .then_with(|| secondary_key(left).cmp(&secondary_key(right)))
        .then_with(|| left.cmp(right))
}

fn primary_compare(left: &str, right: &str) -> Ordering {
    let left = primary_sequence(left);
    let right = primary_sequence(right);
    let (mut a, mut b) = (0, 0);
    while a < left.len() && b < right.len() {
        if left[a].is_ascii_digit() && right[b].is_ascii_digit() {
            let (a_end, b_end) = (digit_end(&left, a), digit_end(&right, b));
            let ordering = compare_digits(&left[a..a_end], &right[b..b_end]);
            if ordering != Ordering::Equal {
                return ordering;
            }
            a = a_end;
            b = b_end;
            continue;
        }
        let ordering = left[a].cmp(&right[b]);
        if ordering != Ordering::Equal {
            return ordering;
        }
        a += 1;
        b += 1;
    }
    (left.len() - a).cmp(&(right.len() - b))
}

fn primary_sequence(value: &str) -> Vec<char> {
    let mut result = Vec::with_capacity(value.chars().count());
    for character in value.chars().flat_map(char::to_lowercase) {
        if is_combining_mark(character) {
            continue;
        }
        match character {
            'æ' => result.extend(['a', 'e']),
            'œ' => result.extend(['o', 'e']),
            'ß' => result.extend(['s', 's']),
            'þ' => result.extend(['t', 'h']),
            value => result.push(primary_char(value)),
        }
    }
    result
}

fn is_combining_mark(character: char) -> bool {
    matches!(
        character,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe20}'..='\u{fe2f}'
    )
}

fn digit_end(chars: &[char], start: usize) -> usize {
    chars[start..]
        .iter()
        .position(|character| !character.is_ascii_digit())
        .map_or(chars.len(), |offset| start + offset)
}

fn compare_digits(left: &[char], right: &[char]) -> Ordering {
    let left_zeroes = left
        .iter()
        .take_while(|character| **character == '0')
        .count();
    let right_zeroes = right
        .iter()
        .take_while(|character| **character == '0')
        .count();
    let left_value = &left[left_zeroes.min(left.len().saturating_sub(1))..];
    let right_value = &right[right_zeroes.min(right.len().saturating_sub(1))..];
    left_value
        .len()
        .cmp(&right_value.len())
        .then_with(|| left_value.cmp(right_value))
        .then_with(|| left.len().cmp(&right.len()))
}

fn secondary_key(value: &str) -> Vec<u32> {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(u32::from)
        .collect()
}

fn primary_char(character: char) -> char {
    let lower = character.to_lowercase().next().unwrap_or(character);
    match lower {
        'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' => 'n',
        'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' => 's',
        'ţ' | 'ť' | 'ŧ' => 't',
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        value => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_locale_groups_case_accents_and_orders_numbers_naturally() {
        let mut names = vec![
            "file10",
            "Éclair",
            "file2",
            "ångström",
            "Banana",
            "eclair",
            "E\u{301}clair",
            "Angstrom",
        ];
        names.sort_by(|left, right| compare(left, right));
        assert_eq!(
            names,
            [
                "Angstrom",
                "ångström",
                "Banana",
                "eclair",
                "E\u{301}clair",
                "Éclair",
                "file2",
                "file10"
            ]
        );
    }
}
