use super::Ui;
use crate::{
    core::{Matrix, Point},
    judge::VelocityTracker,
};
use macroquad::{
    prelude::{Touch, TouchPhase, Vec2},
    window::get_internal_gl,
};
use nalgebra::Translation2;

const THRESHOLD: f32 = 0.03;
const EXTEND: f32 = 0.33;

pub struct Scroller {
    touch: Option<(u64, f32, f32, bool)>,
    offset: f32,
    bound: f32,
    size: f32,
    speed: f32,
    last_time: f32,
    tracker: VelocityTracker,
    pub pulled: bool,
}

impl Default for Scroller {
    fn default() -> Self {
        Self::new()
    }
}

impl Scroller {
    pub fn new() -> Self {
        Self {
            touch: None,
            offset: 0.,
            bound: 0.,
            size: 0.,
            speed: 0.,
            last_time: 0.,
            tracker: VelocityTracker::empty(),
            pulled: false,
        }
    }

    pub fn touch(&mut self, id: u64, phase: TouchPhase, val: f32, t: f32) -> bool {
        match phase {
            TouchPhase::Started => {
                self.tracker.push(t, Point::new(val, 0.));
                self.speed = 0.;
                if self.touch.is_none() && 0. <= val && val < self.bound {
                    self.touch = Some((id, val, self.offset, false));
                }
            }
            TouchPhase::Stationary | TouchPhase::Moved => {
                self.tracker.push(t, Point::new(val, 0.));
                if let Some((sid, st, st_off, unlock)) = &mut self.touch {
                    if *sid == id {
                        if (*st - val).abs() > THRESHOLD {
                            *unlock = true;
                        }
                        if *unlock {
                            self.offset = (*st_off + (*st - val)).max(-EXTEND).min(self.size + EXTEND);
                        }
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.tracker.push(t, Point::new(val, 0.));
                let speed = self.tracker.speed().x;
                if speed.abs() > 0.2 {
                    self.speed = -speed * 0.4;
                    self.last_time = t;
                }
                if self.offset <= -EXTEND * 0.7 {
                    self.pulled = true;
                }
                let res = self.touch.map(|it| it.3).unwrap_or_default();
                self.touch = None;
                return res;
            }
        }
        self.touch.map(|it| it.3).unwrap_or_default()
    }

    pub fn update(&mut self, t: f32) {
        let dt = t - self.last_time;
        self.offset += self.speed * dt;
        const K: f32 = 3.;
        let unlock = self.touch.map(|it| it.3).unwrap_or_default();
        if !unlock && self.offset < 0. {
            self.speed = -self.offset * K;
        } else if !unlock && self.offset > self.size {
            self.speed = (self.size - self.offset) * K;
        } else {
            self.speed *= (0.5_f32).powf((t - self.last_time) / 0.9);
        }
        self.last_time = t;
        if self.pulled {
            self.pulled = false;
        }
    }

    pub fn offset(&self) -> f32 {
        self.offset
    }

    pub fn set_offset(&mut self, val: f32) {
        self.offset = val;
    }

    pub fn bound(&mut self, bound: f32) {
        self.bound = bound;
    }

    pub fn size(&mut self, size: f32) {
        self.size = size;
    }
}

pub struct Scroll {
    pub x_scroller: Scroller,
    pub y_scroller: Scroller,
    size: (f32, f32),
    matrix: Option<Matrix>,
}

impl Default for Scroll {
    fn default() -> Self {
        Self::new()
    }
}

impl Scroll {
    pub fn new() -> Self {
        Self {
            x_scroller: Scroller::new(),
            y_scroller: Scroller::new(),
            size: (2., 2.),
            matrix: None,
        }
    }

    pub fn set_offset(&mut self, x: f32, y: f32) {
        self.x_scroller.set_offset(x);
        self.y_scroller.set_offset(y);
    }

    pub fn touch(&mut self, touch: Touch, t: f32) -> bool {
        let Some(matrix) = self.matrix else { return false; };
        let pt = touch.position;
        let pt = matrix.transform_point(&Point::new(pt.x, pt.y));
        // self.x_scroller.touch(touch.id, touch.phase, pt.x, t) |
        self.y_scroller.touch(touch.id, touch.phase, pt.y, t)
    }

    pub fn update(&mut self, t: f32) {
        self.x_scroller.update(t);
        self.y_scroller.update(t);
    }

    pub fn position(&self, touch: &Touch) -> Option<(f32, f32)> {
        self.matrix.and_then(|mat| {
            let Vec2 { x, y } = touch.position;
            let p = mat.transform_point(&Point::new(x, y));
            if p.x < 0. || p.x >= self.size.0 || p.y < 0. || p.y >= self.size.1 {
                return None;
            }
            let (x, y) = (p.x + self.x_scroller.offset(), p.y + self.y_scroller.offset());
            Some((x, y))
        })
    }

    pub fn render(&mut self, ui: &mut Ui, content: impl FnOnce(&mut Ui) -> (f32, f32)) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        self.matrix = Some(ui.get_matrix().try_inverse().unwrap());
        let pt = ui.to_global((0., 0.));
        let vec = ui.vec_to_global(self.size);
        let vp = gl.get_viewport();
        let pt = (vp.0 as f32 + (pt.0 + 1.) / 2. * vp.2 as f32, vp.1 as f32 + (pt.1 * vp.2 as f32 / vp.3 as f32 + 1.) / 2. * vp.3 as f32);
        gl.scissor(Some((pt.0 as _, pt.1 as _, (vec.0 * vp.2 as f32 / 2.) as _, (vec.1 * vp.2 as f32 / 2.) as _)));
        let s = ui.with(Translation2::new(-self.x_scroller.offset(), -self.y_scroller.offset()).to_homogeneous(), content);
        gl.scissor(None);
        self.x_scroller.bound(s.0);
        self.y_scroller.bound(s.1);
        self.x_scroller.size((s.0 - self.size.0).max(0.));
        self.y_scroller.size((s.1 - self.size.1).max(0.));
    }

    pub fn size(&mut self, size: (f32, f32)) {
        self.size = size;
    }
}
