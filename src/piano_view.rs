use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::keymap::{self, WHITE_KEY_COUNT};

const KEY_ASPECT: f32 = 5.0;
const BLACK_WIDTH: f32 = 0.6;
const BLACK_HEIGHT: f32 = 0.62;

/// Draws the keyboard and returns the note under the mouse while the button is held.
pub fn show(ui: &mut egui::Ui, is_pressed: impl Fn(u8) -> bool) -> Option<u8> {
    let width = ui.available_width();
    let key_width = width / WHITE_KEY_COUNT as f32;
    let height = (key_width * KEY_ASPECT).min(ui.available_height());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::drag());

    let white = |i: usize| {
        Rect::from_min_size(
            rect.min + Vec2::new(i as f32 * key_width, 0.0),
            Vec2::new(key_width, height),
        )
    };
    let black = |i: usize| {
        let size = Vec2::new(key_width * BLACK_WIDTH, height * BLACK_HEIGHT);
        Rect::from_min_size(
            rect.min + Vec2::new((i + 1) as f32 * key_width - size.x / 2.0, 0.0),
            size,
        )
    };
    let sharps = || (0..WHITE_KEY_COUNT).filter(|&i| keymap::has_sharp(i));

    let painter = ui.painter_at(rect);
    let accent = ui.visuals().selection.bg_fill;
    let font = FontId::monospace(key_width * 0.45);

    for i in 0..WHITE_KEY_COUNT {
        let r = white(i);
        let fill = if is_pressed(keymap::white_note(i)) {
            accent
        } else {
            Color32::WHITE
        };
        painter.rect_filled(r, 3.0, fill);
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
        let fill = if is_pressed(keymap::white_note(i) + 1) {
            accent
        } else {
            Color32::from_gray(25)
        };
        painter.rect_filled(r, 2.0, fill);
        painter.text(
            r.center_bottom() - Vec2::Y * key_width * 0.5,
            Align2::CENTER_CENTER,
            keymap::label(i, true),
            FontId::monospace(key_width * 0.35),
            Color32::LIGHT_GRAY,
        );
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
