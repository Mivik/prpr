pub use macroquad::color::Color;

pub const NOTE_WIDTH_RATIO_BASE: f32 = 0.13175016;
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

mod effect;
pub use effect::{Effect, Uniform};

mod line;
pub use line::{JudgeLine, JudgeLineCache, JudgeLineKind, UIElement};

mod note;
use macroquad::prelude::set_pc_assets_folder;
pub use note::{BadNote, Note, NoteKind, RenderConfig};

mod object;
pub use object::Object;

mod resource;
pub use resource::{ParticleEmitter, Resource, DPI_VALUE};

mod tween;
pub use tween::{easing_from, ClampedTween, StaticTween, TweenFunction, TweenId, TweenMajor, TweenMinor, Tweenable, TWEEN_FUNCTIONS};

pub fn init_assets() {
    if let Ok(exe) = std::env::current_exe() {
        std::env::set_current_dir(exe.parent().unwrap()).unwrap();
    }
    set_pc_assets_folder("assets");
}
