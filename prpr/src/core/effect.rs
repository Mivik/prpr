use super::{Anim, Resource, Tweenable};
use anyhow::Result;
use macroquad::prelude::*;
use miniquad::UniformType;
use phf::phf_map;
use std::ops::Range;

static SHADERS: phf::Map<&'static str, &'static str> = phf_map! {
    "chromatic" => include_str!("shaders/chromatic.glsl"),
    "circleBlur" => include_str!("shaders/circle_blur.glsl"),
    "glitch" => include_str!("shaders/glitch.glsl"),
    "noise" => include_str!("shaders/noise.glsl"),
    "pixel" => include_str!("shaders/pixel.glsl"),
    "radialBlur" => include_str!("shaders/radial_blur.glsl"),
};

pub trait UniformValue: Tweenable + Default {
    const UNIFORM_TYPE: UniformType;
}

impl UniformValue for f32 {
    const UNIFORM_TYPE: UniformType = UniformType::Float1;
}

impl UniformValue for Color {
    const UNIFORM_TYPE: UniformType = UniformType::Float4;
}

pub trait Uniform {
    fn uniform_pair(&self) -> (String, UniformType);
    fn set_time(&mut self, t: f32);
    fn apply(&self, material: &Material);
}

impl<T: UniformValue> Uniform for (String, Anim<T>) {
    fn uniform_pair(&self) -> (String, UniformType) {
        (self.0.clone(), T::UNIFORM_TYPE)
    }

    fn set_time(&mut self, t: f32) {
        self.1.set_time(t);
    }

    fn apply(&self, material: &Material) {
        material.set_uniform(&self.0, self.1.now());
    }
}

pub struct Effect {
    time_range: Range<f32>,
    t: f32,
    material: Material,
    uniforms: Vec<Box<dyn Uniform>>,
}

impl Effect {
    pub fn get_preset(name: &str) -> Option<&'static str> {
        SHADERS.get(name).copied()
    }

    pub fn new(time_range: Range<f32>, shader: &str, uniforms: Vec<Box<dyn Uniform>>) -> Result<Self> {
        let version_line = "#version 130\n";
        Ok(Self {
            time_range,
            t: f32::NEG_INFINITY,
            material: load_material(
                if cfg!(target_os = "android") { VERTEX_SHADER.strip_prefix(version_line).unwrap() } else { VERTEX_SHADER },
                if cfg!(target_os = "android") { shader.strip_prefix(version_line).unwrap() } else { shader },
                MaterialParams {
                    uniforms: uniforms
                        .iter()
                        .map(|it| it.uniform_pair())
                        .chain(std::iter::once(("time".to_owned(), UniformType::Float1)))
                        .collect(),
                    ..Default::default()
                },
            )?,
            uniforms,
        })
    }

    pub fn update(&mut self, res: &Resource) {
        let t = res.time;
        self.t = t;
        if self.time_range.contains(&t) {
            for uniform in &mut self.uniforms {
                uniform.set_time(t);
            }
        }
    }

    pub fn render(&self, res: &Resource) {
        if !self.time_range.contains(&self.t) {
            return;
        }
        for uniform in &self.uniforms {
            uniform.apply(&self.material);
        }
        self.material
            .set_uniform("time", self.t);

        gl_use_material(self.material);
        let top = 1. / res.aspect_ratio;
        draw_rectangle(-1., -top, 2., top * 2., WHITE);
        gl_use_default_material();
    }
}

const VERTEX_SHADER: &str = r#"#version 130
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;
