use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::keymap;
use crate::practice::{Judgement, Practice};

const HEIGHT: f32 = 110.0;
const PIXELS_PER_SECOND: f32 = 260.0;
const HIT_LINE: f32 = 140.0;
const TAIL_HEIGHT: f32 = 8.0;
const NOTE_RADIUS: f32 = 22.0;
// Missed notes keep scrolling past the hit line for a moment before going off screen.
const LOOKBACK: usize = 16;

/// Draws the melody scrolling right to left towards the hit line. Each note's tail shows how
/// long it rings.
pub fn show(ui: &mut egui::Ui, practice: &Practice) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    let visuals = ui.visuals();
    let accent = visuals.selection.bg_fill;
    let hit_x = rect.left() + HIT_LINE;
    let mid = rect.center().y;

    painter.rect_filled(rect, 6.0, visuals.extreme_bg_color);
    let line_color = if practice.showing_mistake() {
        visuals.error_fg_color
    } else {
        accent
    };
    painter.line_segment(
        [
            Pos2::new(hit_x, rect.top()),
            Pos2::new(hit_x, rect.bottom()),
        ],
        Stroke::new(3.0, line_color),
    );

    painter.circle_stroke(
        Pos2::new(hit_x, mid),
        NOTE_RADIUS + 4.0,
        Stroke::new(2.0, line_color.gamma_multiply(0.6)),
    );

    let position = practice.position();
    let x_of = |time: f32| hit_x + (time - position) / practice.speed() * PIXELS_PER_SECOND;
    let first = practice.next_index().saturating_sub(LOOKBACK);

    for (i, target) in practice.targets().iter().enumerate().skip(first) {
        let x = x_of(target.time);
        if x > rect.right() {
            break;
        }
        let end = x_of(target.time + target.length);
        let color = match target.judgement {
            _ if target.auto => continue,
            Some(Judgement::Perfect | Judgement::Good) => continue,
            Some(Judgement::Miss) => visuals.error_fg_color.gamma_multiply(0.6),
            None if i == practice.next_index() => accent,
            None => Color32::GRAY,
        };

        let tail =
            Rect::from_x_y_ranges(x..=end, mid - TAIL_HEIGHT / 2.0..=mid + TAIL_HEIGHT / 2.0);
        painter.rect_filled(tail, 4.0, color.gamma_multiply(0.35));
        let outline = match target.judgement {
            None if i == practice.next_index() => Stroke::new(3.0, accent),
            None => Stroke::new(2.0, Color32::WHITE),
            _ => Stroke::new(2.0, color),
        };
        painter.circle(
            Pos2::new(x, mid),
            NOTE_RADIUS,
            visuals.extreme_bg_color,
            outline,
        );
        // Songs don't need Shift, so show the plain key, with a dot for a black key.
        let Some((index, sharp)) = keymap::key_for_note(practice.key_to_press(target)) else {
            continue;
        };
        painter.text(
            Pos2::new(x, mid),
            Align2::CENTER_CENTER,
            keymap::label(index, false),
            FontId::monospace(30.0),
            color,
        );
        if sharp {
            painter.circle_filled(Pos2::new(x, mid - 24.0), 4.0, color);
        }
    }
}
