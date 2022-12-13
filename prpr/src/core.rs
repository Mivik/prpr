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
