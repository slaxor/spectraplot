use eframe::egui::{
    self, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2,
};

const FILL: Color32 = Color32::from_rgb(255, 238, 170);
const EDGE: Color32 = Color32::from_rgb(220, 150, 30);
const INK: Color32 = Color32::from_rgb(30, 30, 30);
pub const PADDING: egui::Vec2 = vec2(6.0, 4.0);
const GAP: f32 = 14.0;
const TAIL_HALF_WIDTH: f32 = 5.0;
const MAX_WIDTH: f32 = 240.0;
const EDITING_MIN_WIDTH: f32 = 120.0;

/// Draws a speech bubble whose tail ends at `tip`, kept inside the painter's clip rect,
/// and returns the box. While `editing`, the text is left to a text field laid over the box.
pub fn draw(painter: &Painter, tip: Pos2, text: &str, editing: bool) -> Rect {
    let galley = painter.layout(text.to_string(), FontId::proportional(12.0), INK, MAX_WIDTH);
    let min_width = if editing { EDITING_MIN_WIDTH } else { 0.0 };
    let size = vec2(galley.size().x.max(min_width), galley.size().y) + PADDING * 2.0;
    let area = painter.clip_rect();
    let above = tip.y - GAP - size.y >= area.top();
    let top = if above {
        tip.y - GAP - size.y
    } else {
        tip.y + GAP
    };
    let left = (tip.x - size.x * 0.25).clamp(area.left(), (area.right() - size.x).max(area.left()));
    let body = Rect::from_min_size(pos2(left, top), size);
    let edge_y = if above { body.bottom() } else { body.top() };
    let base_x = tip.x.clamp(
        body.left() + TAIL_HALF_WIDTH + 2.0,
        body.right() - TAIL_HALF_WIDTH - 2.0,
    );
    let (a, b) = (
        pos2(base_x - TAIL_HALF_WIDTH, edge_y),
        pos2(base_x + TAIL_HALF_WIDTH, edge_y),
    );
    let stroke = Stroke::new(1.0, EDGE);
    painter.rect(body, 4.0, FILL, stroke, StrokeKind::Middle);
    painter.add(Shape::convex_polygon(vec![a, b, tip], FILL, Stroke::NONE));
    painter.line_segment([a, tip], stroke);
    painter.line_segment([b, tip], stroke);
    painter.line_segment([a, b], Stroke::new(1.5, FILL));
    if !editing {
        painter.galley(body.min + PADDING, galley, INK);
    }
    body
}

/// The text colour of a bubble, for the field that edits it.
pub const fn ink() -> Color32 {
    INK
}
