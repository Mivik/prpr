use macroquad::prelude::Color;
use once_cell::sync::Lazy;
use std::{ops::Range, rc::Rc};

pub type TweenId = u8;

const PI: f32 = std::f32::consts::PI;

macro_rules! f1 {
    ($fn:ident) => {
        $fn
    };
}

macro_rules! f2 {
    ($fn:ident) => {
        |x| (1. - $fn(1. - x))
    };
}

macro_rules! f3 {
    ($fn:ident) => {
        |x| {
            let x = x * 2.;
            if x < 1. {
                $fn(x) / 2.
            } else {
                1. - $fn(2. - x) / 2.
            }
        }
    };
}

#[inline]
fn sine(x: f32) -> f32 {
    1. - ((x * PI) / 2.).cos()
}

#[inline]
fn quad(x: f32) -> f32 {
    x * x
}

#[inline]
fn cubic(x: f32) -> f32 {
    x * x * x
}

#[inline]
fn quart(x: f32) -> f32 {
    x * x * x * x
}

#[inline]
fn quint(x: f32) -> f32 {
    x * x * x * x * x
}

#[inline]
fn expo(x: f32) -> f32 {
    (2.0_f32).powf(10. * (x - 1.))
}

#[inline]
fn circ(x: f32) -> f32 {
    1. - (1. - x * x).sqrt()
}

#[inline]
fn back(x: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.;
    (C3 * x - C1) * x * x
}

#[inline]
fn elastic(x: f32) -> f32 {
    const C4: f32 = (2. * PI) / 3.;
    -((2.0_f32).powf(10. * x - 10.) * ((x * 10. - 10.75) * C4).sin())
}

#[inline]
fn bounce(x: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;

    let x = 1. - x;
    1. - (if x < 1. / D1 {
        N1 * x.powi(2)
    } else if x < 2. / D1 {
        N1 * (x - 1.5 / D1).powi(2) + 0.75
    } else if x < 2.5 / D1 {
        N1 * (x - 2.25 / D1).powi(2) + 0.9375
    } else {
        N1 * (x - 2.625 / D1).powi(2) + 0.984375
    })
}

#[rustfmt::skip]
pub static TWEEN_FUNCTIONS: [fn(f32) -> f32; 33] = [
	|_| 0., |_| 1., |x| x,
	f1!(sine), f2!(sine), f3!(sine),
	f1!(quad), f2!(quad), f3!(quad),
	f1!(cubic), f2!(cubic), f3!(cubic),
	f1!(quart), f2!(quart), f3!(quart),
	f1!(quint), f2!(quint), f3!(quint),
	f1!(expo), f2!(expo), f3!(expo),
	f1!(circ), f2!(circ), f3!(circ),
	f1!(back), f2!(back), f3!(back),
	f1!(elastic), f2!(elastic), f3!(elastic),
	f1!(bounce), f2!(bounce), f3!(bounce),
];

thread_local! {
    static TWEEN_FUNCTION_RCS: Lazy<Vec<Rc<dyn TweenFunction>>> = Lazy::new(|| {
        (0..33)
            .map(|it| -> Rc<dyn TweenFunction> { Rc::new(StaticTween(it)) })
            .collect()
    });
}

pub trait TweenFunction {
    fn y(&self, x: f32) -> f32;
}

pub struct StaticTween(pub TweenId);
impl TweenFunction for StaticTween {
    fn y(&self, x: f32) -> f32 {
        TWEEN_FUNCTIONS[self.0 as usize](x)
    }
}

impl StaticTween {
    pub fn get_rc(tween: TweenId) -> Rc<dyn TweenFunction> {
        TWEEN_FUNCTION_RCS.with(|rcs| Rc::clone(&rcs[tween as usize]))
    }
}

// TODO assuming monotone, but actually they're not (e.g. Back tween)
pub struct ClampedTween(TweenId, Range<f32>, Range<f32>);
impl TweenFunction for ClampedTween {
    fn y(&self, x: f32) -> f32 {
        (TWEEN_FUNCTIONS[self.0 as usize](f32::tween(&self.1.start, &self.1.end, x)) - self.2.start)
            / (self.2.end - self.2.start)
    }
}

impl ClampedTween {
    pub fn new(tween: TweenId, range: Range<f32>) -> Self {
        let f = TWEEN_FUNCTIONS[tween as usize];
        let y_range = f(range.start)..f(range.end);
        Self(tween, range, y_range)
    }
}

#[repr(u8)]
pub enum TweenMajor {
    Plain,
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

#[repr(u8)]
pub enum TweenMinor {
    In,
    Out,
    InOut,
}

pub const fn easing_from(major: TweenMajor, minor: TweenMinor) -> TweenId {
    major as u8 * 3 + minor as u8
}

pub trait Tweenable: Clone {
    fn tween(x: &Self, y: &Self, t: f32) -> Self;
    fn add(_x: &Self, _y: &Self) -> Self {
        unimplemented!()
    }
}

impl Tweenable for f32 {
    fn tween(x: &Self, y: &Self, t: f32) -> Self {
        x + (y - x) * t
    }

    fn add(x: &Self, y: &Self) -> Self {
        x + y
    }
}

impl Tweenable for Color {
    fn tween(x: &Self, y: &Self, t: f32) -> Self {
        Self::new(
            f32::tween(&x.r, &y.r, t),
            f32::tween(&x.g, &y.g, t),
            f32::tween(&x.b, &y.b, t),
            f32::tween(&x.a, &y.a, t),
        )
    }
}

impl Tweenable for String {
    fn tween(x: &Self, _y: &Self, _t: f32) -> Self {
        x.clone()
    }
}
