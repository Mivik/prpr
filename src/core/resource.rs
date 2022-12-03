use std::io::Cursor;

use crate::{
    core::{ASPECT_RATIO, JUDGE_LINE_PERFECT_COLOR},
    particle::{AtlasConfig, ColorCurve, Emitter, EmitterConfig},
};
use anyhow::Result;
use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
};
use macroquad::{
    prelude::{load_file, vec2, warn, Camera, Camera2D, Mat4},
    text::{load_ttf_font, Font},
    texture::{load_image, Texture2D},
    window::{screen_height, screen_width},
};

use super::{object::world_to_screen, Point};

const FONT_PATH: &str = "font.ttf";

pub struct NoteStyle {
    pub click: Texture2D,
    pub hold_head: Texture2D,
    pub hold: Texture2D,
    pub hold_tail: Texture2D,
    pub flick: Texture2D,
    pub drag: Texture2D,
}

pub struct Resource {
    pub time: f32,
    pub camera: Camera2D,
    pub camera_matrix: Mat4,

    pub font: Font,
    pub note_style: NoteStyle,
    pub note_style_mh: NoteStyle,

    pub emitter: Emitter,
    pub emitter_square: Emitter,

    pub audio_manager: AudioManager,
    pub sfx_click: StaticSoundData,
    pub sfx_drag: StaticSoundData,
    pub sfx_flick: StaticSoundData,
}

impl Resource {
    pub async fn new() -> Result<Self> {
        async fn load_tex(path: &str) -> Result<Texture2D> {
            Ok(Texture2D::from_image(&load_image(path).await?))
        }
        async fn load_sfx(path: &str) -> Result<StaticSoundData> {
            Ok(StaticSoundData::from_cursor(
                Cursor::new(load_file(&path).await?),
                StaticSoundSettings::default(),
            )?)
        }
        let hold_tail = load_tex("hold_tail.png").await?;
        let note_style = NoteStyle {
            click: load_tex("click.png").await?,
            hold_head: load_tex("hold_head.png").await?,
            hold: load_tex("hold.png").await?,
            hold_tail,
            flick: load_tex("flick.png").await?,
            drag: load_tex("drag.png").await?,
        };
        let camera = Camera2D {
            target: vec2(0., 0.),
            zoom: vec2(1., ASPECT_RATIO),
            ..Default::default()
        };
        let colors_curve = {
            let start = JUDGE_LINE_PERFECT_COLOR;
            let mut mid = start;
            let mut end = start;
            mid.a *= 0.7;
            end.a = 0.;
            ColorCurve { start, mid, end }
        };
        Ok(Self {
            time: 0.0,
            camera,
            camera_matrix: camera.matrix(),

            font: match load_ttf_font(FONT_PATH).await {
                Err(err) => {
                    warn!("Failed to load font from {FONT_PATH}, falling back to default\n{err:?}");
                    Font::default()
                }
                Ok(font) => font,
            },
            note_style,
            note_style_mh: NoteStyle {
                click: load_tex("click_mh.png").await?,
                hold_head: load_tex("hold_head_mh.png").await?,
                hold: load_tex("hold_mh.png").await?,
                hold_tail,
                flick: load_tex("flick_mh.png").await?,
                drag: load_tex("drag_mh.png").await?,
            },

            emitter: Emitter::new(EmitterConfig {
                local_coords: false,
                texture: Some(load_tex("hit_fx.png").await?),
                lifetime: 0.5,
                lifetime_randomness: 0.0,
                initial_direction_spread: 0.0,
                initial_velocity: 0.0,
                size: 0.,
                atlas: Some(AtlasConfig::new(1, 30, ..)),
                emitting: false,
                colors_curve,
                ..Default::default()
            }),
            emitter_square: Emitter::new(EmitterConfig {
                local_coords: false,
                lifetime: 0.5,
                lifetime_randomness: 0.0,
                initial_direction_spread: 2. * std::f32::consts::PI,
                size: 0.,
                emitting: false,
                colors_curve,
                ..Default::default()
            }),

            audio_manager: AudioManager::<CpalBackend>::new(AudioManagerSettings::default())?,
            sfx_click: load_sfx("click.ogg").await?,
            sfx_drag: load_sfx("drag.ogg").await?,
            sfx_flick: load_sfx("flick.ogg").await?,
        })
    }

    pub fn emit_at_origin(&mut self) {
        let pt = world_to_screen(self, Point::default());
        let pt = vec2(
            (pt.x / 2. + 0.5) * screen_width(),
            (0.5 - pt.y / 2.) * screen_height(),
        );
        self.emitter.emit(pt, 1);
        self.emitter_square.emit(pt, 4);
    }

    pub fn update_size(&mut self) -> bool {
        fn viewport() -> (i32, i32, i32, i32) {
            let (w, h) = (screen_width(), screen_height());
            let (rw, rh) = {
                let ew = h * ASPECT_RATIO;
                if ew > w {
                    let eh = w / ASPECT_RATIO;
                    (w, eh)
                } else {
                    (ew, h)
                }
            };
            (
                ((w - rw) / 2.).round() as i32,
                ((h - rh) / 2.).round() as i32,
                rw as i32,
                rh as i32,
            )
        }
        let vp = viewport();
        if Some(vp) != self.camera.viewport {
            self.camera.viewport = Some(vp);
            let s = vp.2 as f32;
            self.emitter.config.size = s / 11.;
            self.emitter_square.config.size = s / 100.;
            self.emitter_square.config.initial_velocity = s * 3. / 4.;
            self.emitter_square.config.initial_velocity_randomness = 1. / 10.;
            self.emitter_square.config.linear_accel = -s / 135.;
            true
        } else {
            false
        }
    }
}
