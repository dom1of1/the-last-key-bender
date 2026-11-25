use eframe::egui::Key;

/// White keys from C2 to C7, left to right. Shift + key plays the sharp above it.
#[rustfmt::skip]
const WHITE_KEYS: [Key; 36] = [
    Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7,
    Key::Num8, Key::Num9, Key::Num0, Key::Q, Key::W, Key::E, Key::R,
    Key::T, Key::Y, Key::U, Key::I, Key::O, Key::P, Key::A,
    Key::S, Key::D, Key::F, Key::G, Key::H, Key::J, Key::K,
    Key::L, Key::Z, Key::X, Key::C, Key::V, Key::B, Key::N,
    Key::M,
];

const LOWEST_NOTE: u8 = 36;
const SCALE: [u8; 7] = [0, 2, 4, 5, 7, 9, 11];

pub fn note_for(key: Key, shift: bool) -> Option<u8> {
    let index = WHITE_KEYS.iter().position(|&k| k == key)?;
    let degree = index % 7;
    let note = LOWEST_NOTE + 12 * (index / 7) as u8 + SCALE[degree];

    if !shift {
        return Some(note);
    }
    let has_sharp = !matches!(degree, 2 | 6) && index + 1 < WHITE_KEYS.len();
    has_sharp.then_some(note + 1)
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
    fn unmapped_keys_play_nothing() {
        assert_eq!(note_for(Key::Space, false), None);
    }
}
