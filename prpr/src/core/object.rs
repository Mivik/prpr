use super::{AnimFloat, AnimVector, Color, Matrix, Resource, Vector};
use macroquad::prelude::*;
use nalgebra::Rotation2;

#[derive(Default)]
pub struct Object {
    pub alpha: AnimFloat,
    pub scale: AnimVector,
    pub rotation: AnimFloat,
    pub translation: AnimVector,
}

impl Object {
    pub fn set_time(&mut self, time: f32) {
        self.alpha.set_time(time);
        self.scale.0.set_time(time);
        self.scale.1.set_time(time);
        self.rotation.set_time(time);
        self.translation.0.set_time(time);
        self.translation.1.set_time(time);
    }

    pub fn dead(&self) -> bool {
        self.alpha.dead()
            && self.scale.0.dead()
            && self.scale.1.dead()
            && self.rotation.dead()
            && self.translation.0.dead()
            && self.translation.1.dead()
    }

    pub fn now(&self, res: &Resource) -> Matrix {
        self.now_rotation().append_translation(&self.now_translation(res))
    }

    #[inline]
    pub fn now_rotation(&self) -> Matrix {
        Rotation2::new(self.rotation.now().to_radians()).to_homogeneous()
    }

    #[inline]
    pub fn now_translation(&self, res: &Resource) -> Vector {
        let mut tr = self.translation.now();
        tr.y /= res.aspect_ratio;
        tr
    }

    #[inline]
    pub fn now_alpha(&self) -> f32 {
        self.alpha.now_opt().unwrap_or(1.0).max(0.)
    }

    #[inline]
    pub fn now_color(&self) -> Color {
        Color::new(1.0, 1.0, 1.0, self.now_alpha())
    }

    #[inline]
    pub fn now_scale(&self) -> Matrix {
        Matrix::identity().append_nonuniform_scaling(&self.scale.now_with_def(1.0, 1.0))
    }
}
