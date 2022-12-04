use super::{AnimFloat, AnimVector, Color, Matrix, Point, Resource, ASPECT_RATIO};
use macroquad::prelude::*;
use nalgebra::Rotation2;
use std::cell::RefCell;

#[derive(Default)]
pub struct Object {
    pub alpha: AnimFloat,
    pub target: AnimVector,
    pub scale: AnimVector,
    pub rotation: AnimFloat,
    pub translation: AnimVector,
}

impl Object {
    pub fn set_time(&mut self, time: f32) {
        self.alpha.set_time(time);
        self.target.0.set_time(time);
        self.target.1.set_time(time);
        self.scale.0.set_time(time);
        self.scale.1.set_time(time);
        self.rotation.set_time(time);
        self.translation.0.set_time(time);
        self.translation.1.set_time(time);
    }

    pub fn now(&self) -> Matrix {
        let mut tr = self.translation.now();
        tr.y /= ASPECT_RATIO;
        (Matrix::identity().append_translation(&-self.target.now())
            * Rotation2::new(self.rotation.now().to_radians()).to_homogeneous())
        .append_translation(&tr)
    }

    pub fn now_color(&self) -> Color {
        Color::new(1.0, 1.0, 1.0, self.alpha.now_opt().unwrap_or(1.0))
    }

    pub fn now_scale(&self) -> Matrix {
        Matrix::identity().append_nonuniform_scaling(&self.scale.now_with_def(1.0, 1.0))
    }
}

thread_local! {
    static MODEL_STACK: RefCell<Vec<Matrix>> = RefCell::new(vec![Matrix::identity()]);
}

pub fn world_to_screen(res: &Resource, pt: Point) -> Point {
    let pt = MODEL_STACK.with(|it| it.borrow().last().unwrap().transform_point(&pt));
    let screen = res.camera_matrix.transform_point3(vec3(pt.x, pt.y, 0.0));
    Point::new(screen.x, screen.y)
}

pub trait ScopedTransform {
    fn apply_render(&self, f: impl FnOnce());
}

impl ScopedTransform for Matrix {
    fn apply_render(&self, f: impl FnOnce()) {
        let gl = unsafe { get_internal_gl() }.quad_gl;
        /*
            [11] [12]  0  [13]
            [21] [22]  0  [23]
              0    0   1    0
            [31] [32]  0  [33]
        */
        MODEL_STACK.with(|it| {
            let mat = it.borrow().last().unwrap() * self;
            it.borrow_mut().push(mat);
        });
        let mat = Mat4::from_cols_array(&[
            self.m11, self.m21, 0., self.m31, self.m12, self.m22, 0., self.m32, 0., 0., 1., 0.,
            self.m13, self.m23, 0., self.m33,
        ]);
        gl.push_model_matrix(mat);
        f();
        gl.pop_model_matrix();
        MODEL_STACK.with(|it| it.borrow_mut().pop());
    }
}
