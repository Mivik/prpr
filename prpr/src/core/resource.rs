use super::{MSRenderTarget, Matrix, Point, JUDGE_LINE_PERFECT_COLOR, NOTE_WIDTH_RATIO_BASE};
use crate::{
    audio::{Audio, AudioClip, DefaultAudio, PlayParams},
    config::Config,
    ext::{nalgebra_to_glm, SafeTexture},
    fs::FileSystem,
    info::ChartInfo,
    particle::{AtlasConfig, ColorCurve, Emitter, EmitterConfig},
};
use anyhow::Result;
use macroquad::prelude::*;
use miniquad::{gl::GLuint, Texture};
use std::{cell::RefCell, collections::BTreeMap, sync::atomic::AtomicU32};

pub const MAX_SIZE: usize = 64; // needs tweaking
pub static DPI_VALUE: AtomicU32 = AtomicU32::new(250);

pub struct NoteStyle {
    pub click: SafeTexture,
    pub hold_head: SafeTexture,
    pub hold: SafeTexture,
    pub hold_tail: SafeTexture,
    pub flick: SafeTexture,
    pub drag: SafeTexture,
}

pub struct ParticleEmitter {
    emitter: Emitter,
    emitter_square: Emitter,
}

impl ParticleEmitter {
    pub async fn new(scale: f32) -> Result<Self> {
        let colors_curve = {
            let start = WHITE;
            let mut mid = start;
            let mut end = start;
            mid.a *= 0.7;
            end.a = 0.;
            ColorCurve { start, mid, end }
        };
        let mut res = Self {
            emitter: Emitter::new(EmitterConfig {
                local_coords: false,
                texture: Some(Texture2D::from_image(&load_image("hit_fx.png").await?)),
                lifetime: 0.5,
                lifetime_randomness: 0.0,
                initial_direction_spread: 0.0,
                initial_velocity: 0.0,
                atlas: Some(AtlasConfig::new(5, 6, ..)),
                emitting: false,
                colors_curve,
                ..Default::default()
            }),
            emitter_square: Emitter::new(EmitterConfig {
                local_coords: false,
                lifetime: 0.5,
                lifetime_randomness: 0.0,
                initial_direction_spread: 2. * std::f32::consts::PI,
                size_randomness: 0.3,
                emitting: false,
                initial_velocity: 2.5,
                initial_velocity_randomness: 1. / 10.,
                linear_accel: -6. / 1.,
                colors_curve,
                ..Default::default()
            }),
        };
        res.set_scale(scale);
        Ok(res)
    }

    pub fn emit_at(&mut self, pt: Vec2, color: Color) {
        self.emitter.config.base_color = color;
        self.emitter.emit(pt, 1);
        self.emitter_square.config.base_color = color;
        self.emitter_square.emit(pt, 4);
    }

    pub fn draw(&mut self, dt: f32) {
        self.emitter.draw(vec2(0., 0.), dt);
        self.emitter_square.draw(vec2(0., 0.), dt);
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.emitter.config.size = scale / 5.;
        self.emitter_square.config.size = scale / 44.;
    }
}

#[derive(Default)]
pub struct NoteBuffer(BTreeMap<(i8, GLuint), Vec<(Vec<Vertex>, Vec<u16>)>>);

impl NoteBuffer {
    pub fn push(&mut self, key: (i8, GLuint), vertices: [Vertex; 4]) {
        let meshes = self.0.entry(key).or_default();
        if meshes.last().map_or(true, |it| it.0.len() + 4 > MAX_SIZE * 4) {
            meshes.push(Default::default());
        }
        let last = meshes.last_mut().unwrap();
        let i = last.0.len() as u16;
        last.0.extend_from_slice(&vertices);
        last.1.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    }

    pub fn draw_all(&mut self) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let gl = gl.quad_gl;
        gl.draw_mode(DrawMode::Triangles);
        for ((_, tex_id), meshes) in std::mem::take(&mut self.0).into_iter() {
            gl.texture(Some(Texture2D::from_miniquad_texture(unsafe { Texture::from_raw_id(tex_id) })));
            for mesh in meshes {
                gl.geometry(&mesh.0, &mesh.1);
            }
        }
    }
}

pub struct Resource {
    pub config: Config,
    pub info: ChartInfo,
    pub aspect_ratio: f32,
    pub dpi: u32,
    pub last_screen_size: (u32, u32),
    pub note_width: f32,

    pub time: f32,

    pub alpha: f32,
    pub judge_line_color: Color,

    pub camera: Camera2D,
    pub camera_matrix: Mat4,

    pub font: Font,
    pub background: SafeTexture,
    pub illustration: SafeTexture,
    pub icons: [SafeTexture; 8],
    pub challenge_icons: [SafeTexture; 6],
    pub note_style: NoteStyle,
    pub note_style_mh: NoteStyle,
    pub player: SafeTexture,
    pub icon_back: SafeTexture,
    pub icon_retry: SafeTexture,
    pub icon_resume: SafeTexture,
    pub icon_proceed: SafeTexture,

    pub emitter: ParticleEmitter,

    pub audio: DefaultAudio,
    pub music: AudioClip,
    pub ending_bgm_bytes: Vec<u8>,
    pub track_length: f32,
    pub sfx_click: AudioClip,
    pub sfx_drag: AudioClip,
    pub sfx_flick: AudioClip,

    pub chart_target: Option<MSRenderTarget>,

    pub note_buffer: RefCell<NoteBuffer>,

    pub model_stack: Vec<Matrix>,
}

impl Resource {
    pub async fn load_icons() -> Result<[SafeTexture; 8]> {
        macro_rules! loads {
            ($($path:literal),*) => {
                [$(loads!(@detail $path)),*]
            };

            (@detail $path:literal) => {
                Texture2D::from_image(&load_image($path).await?).into()
            };
        }
        Ok(loads![
            "rank/F.png",
            "rank/C.png",
            "rank/B.png",
            "rank/A.png",
            "rank/S.png",
            "rank/V.png",
            "rank/FC.png",
            "rank/phi.png"
        ])
    }

    pub async fn load_challenge_icons() -> Result<[SafeTexture; 6]> {
        macro_rules! loads {
            ($($path:literal),*) => {
                [$(loads!(@detail $path)),*]
            };

            (@detail $path:literal) => {
                Texture2D::from_image(&load_image($path).await?).into()
            };
        }
        Ok(loads![
            "rank/white.png",
            "rank/green.png",
            "rank/blue.png",
            "rank/red.png",
            "rank/golden.png",
            "rank/rainbow.png"
        ])
    }

    pub async fn new(
        config: Config,
        info: ChartInfo,
        mut fs: Box<dyn FileSystem>,
        player: Option<SafeTexture>,
        background: SafeTexture,
        illustration: SafeTexture,
        font: Font,
    ) -> Result<Self> {
        macro_rules! load_tex {
            ($path:literal) => {
                SafeTexture::from(Texture2D::from_image(&load_image($path).await?))
            };
        }
        let note_style = NoteStyle {
            click: load_tex!("click.png"),
            hold_head: load_tex!("hold_head.png"),
            hold: load_tex!("hold.png"),
            hold_tail: load_tex!("hold_tail.png"),
            flick: load_tex!("flick.png"),
            drag: load_tex!("drag.png"),
        };
        let camera = Camera2D {
            target: vec2(0., 0.),
            zoom: vec2(1., -config.aspect_ratio.unwrap_or(info.aspect_ratio)),
            ..Default::default()
        };

        let audio = DefaultAudio::new()?;
        macro_rules! load_sfx {
            ($path:literal) => {
                audio.create_clip(load_file($path).await?)?.0
            };
        }
        let (music, track_length) = audio.create_clip(fs.load_file(&info.music).await?)?;
        let track_length = track_length as f32;
        let sfx_click = load_sfx!("click.ogg");
        let sfx_drag = load_sfx!("drag.ogg");
        let sfx_flick = load_sfx!("flick.ogg");

        let aspect_ratio = config.aspect_ratio.unwrap_or(info.aspect_ratio);
        let note_width = config.note_scale * NOTE_WIDTH_RATIO_BASE;
        let note_scale = config.note_scale;

        macroquad::window::gl_set_drawcall_buffer_capacity(MAX_SIZE * 4, MAX_SIZE * 6);
        Ok(Self {
            config,
            info,
            aspect_ratio,
            dpi: DPI_VALUE.load(std::sync::atomic::Ordering::SeqCst),
            last_screen_size: (0, 0),
            note_width,

            time: 0.,

            alpha: 1.,
            judge_line_color: JUDGE_LINE_PERFECT_COLOR,

            camera,
            camera_matrix: camera.matrix(),

            font,
            background,
            illustration,
            icons: Self::load_icons().await?,
            challenge_icons: Self::load_challenge_icons().await?,
            note_style,
            note_style_mh: NoteStyle {
                click: load_tex!("click_mh.png"),
                hold_head: load_tex!("hold_head_mh.png"),
                hold: load_tex!("hold_mh.png"),
                hold_tail: load_tex!("hold_tail_mh.png"),
                flick: load_tex!("flick_mh.png"),
                drag: load_tex!("drag_mh.png"),
            },
            player: if let Some(player) = player { player } else { load_tex!("player.jpg") },
            icon_back: load_tex!("back.png"),
            icon_retry: load_tex!("retry.png"),
            icon_resume: load_tex!("resume.png"),
            icon_proceed: load_tex!("proceed.png"),

            emitter: ParticleEmitter::new(note_scale).await?,

            audio,
            music,
            ending_bgm_bytes: load_file("ending.mp3").await?,
            track_length,
            sfx_click,
            sfx_drag,
            sfx_flick,

            chart_target: None,

            note_buffer: RefCell::new(NoteBuffer::default()),

            model_stack: vec![Matrix::identity()],
        })
    }

    pub fn emit_at_origin(&mut self, color: Color) {
        if !self.config.particle {
            return;
        }
        let pt = self.world_to_screen(Point::default());
        self.emitter.emit_at(vec2(pt.x, -pt.y), color);
    }

    pub fn update_size(&mut self, dim: (u32, u32)) -> bool {
        if self.last_screen_size == dim {
            return false;
        }
        self.last_screen_size = dim;
        self.chart_target = {
            let upscale = self.config.upscale;
            let dim = ((dim.0 as f32 * upscale) as u32, (dim.1 as f32 * upscale) as u32);
            Some(MSRenderTarget::new(dim, self.config.sample_count))
        };
        fn viewport(aspect_ratio: f32, (w, h): (u32, u32)) -> (i32, i32, i32, i32) {
            let w = w as f32;
            let h = h as f32;
            let (rw, rh) = {
                let ew = h * aspect_ratio;
                if ew > w {
                    let eh = w / aspect_ratio;
                    (w, eh)
                } else {
                    (ew, h)
                }
            };
            (((w - rw) / 2.).round() as i32, ((h - rh) / 2.).round() as i32, rw as i32, rh as i32)
        }
        let aspect_ratio = self.config.aspect_ratio.unwrap_or(self.info.aspect_ratio);
        if self.config.fix_aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            self.camera.viewport = Some(viewport(aspect_ratio, dim));
        } else {
            self.aspect_ratio = aspect_ratio.min(dim.0 as f32 / dim.1 as f32);
            self.camera.zoom = vec2(1., -self.aspect_ratio);
            self.camera_matrix = self.camera.matrix();
            self.camera.viewport = Some(viewport(self.aspect_ratio, dim));
        };
        true
    }

    pub fn play_sfx(&mut self, sfx: &AudioClip) {
        if self.config.volume_sfx <= 1e-2 {
            return;
        }
        let _ = self.audio.play(
            sfx,
            PlayParams {
                volume: self.config.volume_sfx as _,
                ..Default::default()
            },
        );
    }

    pub fn world_to_screen(&self, pt: Point) -> Point {
        self.model_stack.last().unwrap().transform_point(&pt)
    }

    pub fn screen_to_world(&self, pt: Point) -> Point {
        self.model_stack.last().unwrap().try_inverse().unwrap().transform_point(&pt)
    }

    #[inline]
    pub fn with_model(&mut self, model: Matrix, f: impl FnOnce(&mut Self)) {
        let model = self.model_stack.last().unwrap() * model;
        self.model_stack.push(model);
        f(self);
        self.model_stack.pop();
    }

    #[inline]
    pub fn apply_model(&mut self, f: impl FnOnce(&mut Self)) {
        self.apply_model_of(&self.model_stack.last().unwrap().clone(), f);
    }

    #[inline]
    pub fn apply_model_of(&mut self, mat: &Matrix, f: impl FnOnce(&mut Self)) {
        unsafe { get_internal_gl() }.quad_gl.push_model_matrix(nalgebra_to_glm(mat));
        f(self);
        unsafe { get_internal_gl() }.quad_gl.pop_model_matrix();
    }
}
