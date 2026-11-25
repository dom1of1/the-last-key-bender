use eframe::egui::Key;

/// White keys from C2 to C7, left to right. Shift + key plays the sharp above it.
#[rustfmt::skip]
const WHITE_KEYS: [Key; WHITE_KEY_COUNT] = [
    Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7,
    Key::Num8, Key::Num9, Key::Num0, Key::Q, Key::W, Key::E, Key::R,
    Key::T, Key::Y, Key::U, Key::I, Key::O, Key::P, Key::A,
    Key::S, Key::D, Key::F, Key::G, Key::H, Key::J, Key::K,
    Key::L, Key::Z, Key::X, Key::C, Key::V, Key::B, Key::N,
    Key::M,
];

pub const WHITE_KEY_COUNT: usize = 36;
const WHITE_LABELS: &[u8; WHITE_KEY_COUNT] = b"1234567890qwertyuiopasdfghjklzxcvbnm";
// US-layout Shift symbols for 0-9, which is what letter sheets use for black keys.
const SHIFTED_DIGITS: &[u8; 10] = b")!@#$%^&*(";
const LOWEST_NOTE: u8 = 36;
const SCALE: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

pub fn white_note(index: usize) -> u8 {
    LOWEST_NOTE + 12 * (index / 7) as u8 + SCALE[index % 7]
}

pub fn has_sharp(index: usize) -> bool {
    !matches!(index % 7, 2 | 6) && index + 1 < WHITE_KEY_COUNT
}

pub fn label(index: usize, sharp: bool) -> char {
    let c = WHITE_LABELS[index];
    match (sharp, c) {
        (false, _) => c as char,
        (true, b'0'..=b'9') => SHIFTED_DIGITS[usize::from(c - b'0')] as char,
        (true, _) => c.to_ascii_uppercase() as char,
    }
}

pub fn note_for(key: Key, shift: bool) -> Option<u8> {
    let index = WHITE_KEYS.iter().position(|&k| k == key)?;
    match shift {
        false => Some(white_note(index)),
        true => has_sharp(index).then(|| white_note(index) + 1),
    }
}

pub fn note_for_label(c: char) -> Option<u8> {
    (0..WHITE_KEY_COUNT).find_map(|i| {
        if label(i, false) == c {
            Some(white_note(i))
        } else if has_sharp(i) && label(i, true) == c {
            Some(white_note(i) + 1)
        } else {
            None
        }
    })
}

/// The white key a note is played from, and whether it's the black key above it.
pub fn key_for_note(note: u8) -> Option<(usize, bool)> {
    (0..WHITE_KEY_COUNT).find_map(|i| {
        if white_note(i) == note {
            Some((i, false))
        } else if has_sharp(i) && white_note(i) + 1 == note {
            Some((i, true))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_keys() {
        assert_eq!(note_for(Key::Num1, false), Some(36));
        assert_eq!(note_for(Key::T, false), Some(60));
        assert_eq!(note_for(Key::M, false), Some(96));
    }

    #[test]
    fn shift_plays_sharps() {
        assert_eq!(note_for(Key::Num1, true), Some(37));
        assert_eq!(note_for(Key::T, true), Some(61));
        assert_eq!(note_for(Key::Num3, true), None);
        assert_eq!(note_for(Key::Num7, true), None);
        assert_eq!(note_for(Key::M, true), None);
    }

    #[test]
    fn covers_all_61_keys() {
        let mut notes: Vec<u8> = WHITE_KEYS
            .iter()
            .flat_map(|&k| [note_for(k, false), note_for(k, true)])
            .flatten()
            .collect();
        notes.sort();
        assert_eq!(notes, (36..=96).collect::<Vec<_>>());
    }

    #[test]
    fn labels_match_letter_sheets() {
        assert_eq!(label(0, false), '1');
        assert_eq!(label(0, true), '!');
        assert_eq!(label(9, true), ')');
        assert_eq!(label(14, false), 't');
        assert_eq!(label(14, true), 'T');
    }

    #[test]
    fn labels_and_notes_round_trip() {
        for note in 36..=96 {
            let (index, sharp) = key_for_note(note).unwrap();
            assert_eq!(note_for_label(label(index, sharp)), Some(note));
        }
        assert_eq!(note_for_label('-'), None);
        assert_eq!(key_for_note(35), None);
    }

    #[test]
    fn unmapped_keys_play_nothing() {
        assert_eq!(note_for(Key::Space, false), None);
    }
}
