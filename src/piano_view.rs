use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::keymap::{self, WHITE_KEY_COUNT};

const KEY_ASPECT: f32 = 5.0;
const MAX_HEIGHT_FRACTION: f32 = 0.4;
const BLACK_WIDTH: f32 = 0.6;
const BLACK_HEIGHT: f32 = 0.62;

#[derive(PartialEq)]
pub enum KeyState {
    Up,
    Down,
    /// Being played by the song.
    Sounding,
}

/// Horizontal position of a note's key, measured from the left edge of the keyboard.
fn note_span(note: u8, width: f32) -> Option<(f32, f32)> {
    let key_width = width / WHITE_KEY_COUNT as f32;
    (0..WHITE_KEY_COUNT).find_map(|i| {
        if keymap::white_note(i) == note {
            Some((i as f32 * key_width, (i + 1) as f32 * key_width))
        } else if keymap::has_sharp(i) && keymap::white_note(i) + 1 == note {
            let center = (i + 1) as f32 * key_width;
            let half = key_width * BLACK_WIDTH / 2.0;
            Some((center - half, center + half))
        } else {
            None
        }
    })
}

/// Draws the keyboard and returns the note under the mouse while the button is held.
/// With `no_shift`, black keys are labelled with the plain key that plays them and a dot,
/// as in songs, where Shift isn't needed.
pub fn show(ui: &mut egui::Ui, no_shift: bool, state: impl Fn(u8) -> KeyState) -> Option<u8> {
    let width = ui.available_width();
    let key_width = width / WHITE_KEY_COUNT as f32;
    let max_height = ui.ctx().content_rect().height() * MAX_HEIGHT_FRACTION;
    let height = (key_width * KEY_ASPECT).min(max_height);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::drag());

    let key_rect = |note: u8, height: f32| {
        let (x0, x1) = note_span(note, width).expect("note is on the keyboard");
        Rect::from_x_y_ranges(
            rect.left() + x0..=rect.left() + x1,
            rect.top()..=rect.top() + height,
        )
    };
    let white = |i: usize| key_rect(keymap::white_note(i), height);
    let black = |i: usize| key_rect(keymap::white_note(i) + 1, height * BLACK_HEIGHT);
    let sharps = || (0..WHITE_KEY_COUNT).filter(|&i| keymap::has_sharp(i));

    let painter = ui.painter_at(rect);
    let accent = ui.visuals().selection.bg_fill;
    let fill = |note: u8, up: Color32| match state(note) {
        KeyState::Up => up,
        KeyState::Down => accent,
        KeyState::Sounding => up.lerp_to_gamma(accent, 0.45),
    };
    let font = FontId::monospace(key_width * 0.45);

    for i in 0..WHITE_KEY_COUNT {
        let r = white(i);
        painter.rect_filled(r, 3.0, fill(keymap::white_note(i), Color32::WHITE));
        painter.rect_stroke(r, 3.0, Stroke::new(1.0, Color32::GRAY), StrokeKind::Inside);
        painter.text(
            r.center_bottom() - Vec2::Y * key_width * 0.5,
            Align2::CENTER_CENTER,
            keymap::label(i, false),
            font.clone(),
            Color32::DARK_GRAY,
        );
    }

    for i in sharps() {
        let r = black(i);
        painter.rect_filled(
            r,
            2.0,
            fill(keymap::white_note(i) + 1, Color32::from_gray(25)),
        );
        let label_pos = r.center_bottom() - Vec2::Y * key_width * 0.5;
        painter.text(
            label_pos,
            Align2::CENTER_CENTER,
            keymap::label(i, !no_shift),
            FontId::monospace(key_width * 0.35),
            Color32::LIGHT_GRAY,
        );
        if no_shift {
            painter.circle_filled(
                label_pos - Vec2::Y * key_width * 0.35,
                2.5,
                Color32::LIGHT_GRAY,
            );
        }
    }

    if !response.is_pointer_button_down_on() {
        return None;
    }
    let pos = response.interact_pointer_pos()?;
    sharps()
        .find(|&i| black(i).contains(pos))
        .map(|i| keymap::white_note(i) + 1)
        .or_else(|| {
            (0..WHITE_KEY_COUNT)
                .find(|&i| white(i).contains(pos))
                .map(keymap::white_note)
        })
}
