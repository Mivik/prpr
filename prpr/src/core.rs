pub use macroquad::color::Color;

pub const NOTE_WIDTH_RATIO: f32 = 0.13175016;
pub const HEIGHT_RATIO: f32 = 0.83175;

pub const EPS: f32 = 1e-5;

pub const JUDGE_LINE_PERFECT_COLOR: Color = Color::new(1., 0.921875, 0.623, 0.8823529);
pub const JUDGE_LINE_GOOD_COLOR: Color = Color::new(0.7058823, 0.8823529, 1., 0.9215686);

pub type Point = nalgebra::Point2<f32>;
pub type Vector = nalgebra::Vector2<f32>;
pub type Matrix = nalgebra::Matrix3<f32>;

mod anim;
pub use anim::{Anim, AnimFloat, AnimVector, Keyframe};

mod chart;
pub use chart::Chart;

mod line;
pub use line::{JudgeLine, JudgeLineKind, JudgeLineCache};

mod note;
use macroquad::prelude::Rect;
pub use note::{BadNote, Note, NoteKind, RenderConfig};

mod object;
pub use object::Object;

mod resource;
pub use resource::Resource;

mod tween;
pub use tween::{
    easing_from, ClampedTween, StaticTween, TweenFunction, TweenId, TweenMajor, TweenMinor,
    Tweenable, TWEEN_FUNCTIONS,
};

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
