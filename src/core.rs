pub use macroquad::color::Color;

// NOT CREDIT uses this
// pub const ASPECT_RATIO: f32 = 1.7009803921568627;
pub const ASPECT_RATIO: f32 = 16. / 9.;
pub const NOTE_WIDTH_RATIO: f32 = 0.12175016;
pub const HEIGHT_RATIO: f32 = 0.83175;

pub type Point = nalgebra::Point2<f32>;
pub type Vector = nalgebra::Vector2<f32>;
pub type Matrix = nalgebra::Matrix3<f32>;

mod anim;
pub use anim::{Anim, AnimFloat, AnimVector, Keyframe};

mod chart;
pub use chart::Chart;

mod line;
pub use line::{JudgeLine, JudgeLineKind};

mod note;
pub use note::{Note, NoteKind};

mod object;
pub use object::{Object, ScopedTransform};

mod resource;
pub use resource::Resource;

mod tween;
pub use tween::{
    easing_from, ClampedTween, StaticTween, TweenFunction, TweenId, TweenMajor, TweenMinor,
    Tweenable, TWEEN_FUNCTIONS,
};
