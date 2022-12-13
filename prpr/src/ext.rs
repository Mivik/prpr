use crate::core::Resource;
use macroquad::prelude::*;
use ordered_float::{Float, NotNan};

pub trait NotNanExt: Sized {
    fn not_nan(self) -> NotNan<Self>;
}

impl<T: Sized + Float> NotNanExt for T {
    fn not_nan(self) -> NotNan<Self> {
        NotNan::new(self).unwrap()
    }
}

pub fn draw_text_aligned(
    res: &Resource,
    text: &str,
    x: f32,
    y: f32,
    anchor: (f32, f32),
    scale: f32,
    color: Color,
) -> Rect {
    use macroquad::prelude::*;
    let size = (screen_width() / 25. * scale) as u16;
    let scale = 0.08 * scale / size as f32;
    let dim = measure_text(text, Some(res.font), size, scale);
    let rect = Rect::new(
        x - dim.width * anchor.0,
        y + dim.offset_y - dim.height * anchor.1,
        dim.width,
        dim.height,
    );
    draw_text_ex(
        text,
        rect.x,
        rect.y,
        TextParams {
            font: res.font,
            font_size: size,
            font_scale: scale,
            color,
            ..Default::default()
        },
    );
    rect
}

pub fn draw_parallelogram(rect: Rect, ratio: f32, color: Color) {
    let len = rect.w * ratio;
    draw_triangle(
        vec2(rect.x + len, rect.y),
        vec2(rect.right() - len, rect.bottom()),
        vec2(rect.x, rect.bottom()),
        color,
    );
    draw_triangle(
        vec2(rect.x + len, rect.y),
        vec2(rect.right() - len, rect.bottom()),
        vec2(rect.right(), rect.y),
        color,
    );
}
