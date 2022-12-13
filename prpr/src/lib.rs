mod ext;

pub mod audio;
pub mod config;
pub mod core;
pub mod fs;
pub mod info;
pub mod judge;
pub mod parse;
pub mod particle;

use crate::{
    audio::{Audio, PlayParams},
    config::Config,
    core::{
        draw_text_aligned, BadNote, Chart, Matrix, Point, Resource, Vector, JUDGE_LINE_GOOD_COLOR,
        JUDGE_LINE_PERFECT_COLOR,
    },
    info::ChartFormat,
    judge::Judge,
    parse::{parse_pec, parse_phigros, parse_rpe},
};
use anyhow::{bail, Context, Result};
use audio::AudioHandle;
use circular_queue::CircularQueue;
use concat_string::concat_string;
use fs::FileSystem;
use info::ChartInfo;
use macroquad::prelude::*;

const ADJUST_TIME_SAMPLE_NUM: usize = 64;
const ADJUST_TIME_THRESHOLD: f64 = 0.02;

pub fn build_conf() -> macroquad::window::Conf {
    Conf {
        window_title: "prpr".to_string(),
        window_width: 1080,
        window_height: 608,
        ..Default::default()
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn on_game_start();
}

pub struct Prpr {
    pub should_exit: bool,

    pub res: Resource,
    pub chart: Chart,
    pub judge: Judge,
    pub gl: InternalGlContext<'static>,

    pub audio_handle: AudioHandle,

    get_time_fn: Box<dyn Fn() -> f64>,
    get_size_fn: Box<dyn Fn() -> (u32, u32)>,

    start_time: f64,
    time_errors: CircularQueue<f64>,
    time_errors_sum: f64,
    pause_time: Option<f64>,
    pause_rewind: Option<f64>,

    bad_notes: Vec<BadNote>,
}

impl Prpr {
    pub async fn new(
        info: ChartInfo,
        config: Config,
        mut fs: Box<dyn FileSystem>,
        get_size_fn: Option<Box<dyn Fn() -> (u32, u32)>>,
    ) -> Result<Self> {
        simulate_mouse_with_touch(false);

        async fn load_chart_bytes(
            fs: &mut Box<dyn FileSystem>,
            info: &ChartInfo,
        ) -> Result<Vec<u8>> {
            if let Ok(bytes) = fs.load_file(&info.chart).await {
                return Ok(bytes);
            }
            if let Some(name) = info.chart.strip_suffix(".pec") {
                if let Ok(bytes) = fs.load_file(&concat_string!(name, ".json")).await {
                    return Ok(bytes);
                }
            }
            bail!("Cannot find chart file")
        }
        let text = String::from_utf8(
            load_chart_bytes(&mut fs, &info)
                .await
                .context("Failed to load chart")?,
        )?;
        let format = info.format.clone().unwrap_or_else(|| {
            if text.starts_with('{') {
                if text.contains("\"META\"") {
                    ChartFormat::Rpe
                } else {
                    ChartFormat::Pgr
                }
            } else {
                ChartFormat::Pec
            }
        });
        let chart = match format {
            ChartFormat::Rpe => parse_rpe(&text).await?,
            ChartFormat::Pgr => parse_phigros(&text)?,
            ChartFormat::Pec => parse_pec(&text)?,
        };

        let mut res = Resource::new(config, info, fs)
            .await
            .context("Failed to load resources")?;

        let judge = Judge::new(&chart);

        // we use performance.now() on web since audioContext.currentTime is not stable
        // and may cause serious latency problem
        #[cfg(target_arch = "wasm32")]
        let get_time = {
            let perf = web_sys::window().unwrap().performance().unwrap();
            let speed = res.config.speed;
            move || perf.now() / 1000. * speed
        };
        #[cfg(not(target_arch = "wasm32"))]
        let get_time = {
            let start = std::time::Instant::now();
            let speed = res.config.speed;
            move || start.elapsed().as_secs_f64() * speed
        };

        #[cfg(target_arch = "wasm32")]
        on_game_start();

        let audio_handle = res.audio.play(
            &res.music,
            PlayParams {
                volume: res.config.volume_music,
                playback_rate: res.config.speed,
                ..Default::default()
            },
        )?;

        let start_time = get_time();
        Ok(Self {
            should_exit: false,

            res,
            chart,
            judge,
            gl: unsafe { get_internal_gl() },

            audio_handle,

            get_time_fn: Box::new(get_time),
            get_size_fn: get_size_fn
                .unwrap_or_else(|| Box::new(|| (screen_width() as u32, screen_height() as u32))),

            start_time,
            time_errors: CircularQueue::with_capacity(ADJUST_TIME_SAMPLE_NUM),
            time_errors_sum: 0.,
            pause_time: None,
            pause_rewind: None,

            bad_notes: Vec::new(),
        })
    }

    #[inline]
    pub fn get_time(&self) -> f64 {
        (self.get_time_fn)()
    }

    pub fn update(&mut self, time: Option<f64>) -> Result<()> {
        let mut time = time
            .unwrap_or_else(|| self.pause_time.unwrap_or_else(&self.get_time_fn) - self.start_time);
        if self.res.config.adjust_time {
            let music_time = self.res.audio.position(&self.audio_handle)?;
            let error = music_time - time;
            if self.time_errors.is_full() {
                self.time_errors_sum -= *self.time_errors.asc_iter().next().unwrap();
            }
            self.time_errors.push(error);
            self.time_errors_sum += error;
            if self.time_errors.is_full()
                && self.pause_time.is_none()
                && self.pause_rewind.is_none()
            {
                let delta = self.time_errors_sum / ADJUST_TIME_SAMPLE_NUM as f64;
                if delta.abs() > ADJUST_TIME_THRESHOLD {
                    warn!(
                        "Time misalignment detected. Syncing time by offset {}...",
                        delta
                    );
                    self.start_time -= delta;
                    time += delta;
                    self.time_errors.clear();
                    self.time_errors_sum = 0.;
                }
            }
        }
        let time = time as f32;

        let time = (time as f32 - self.chart.offset - self.res.config.offset).max(0.0);
        if time > self.res.track_length + 0.8 {
            self.should_exit = true;
        }
        self.res.time = time;
        if self.pause_time.is_none() && self.pause_rewind.is_none() {
            self.judge
                .update(&mut self.res, &mut self.chart, &mut self.bad_notes);
        }
        self.res.judge_line_color = if self.judge.counts[2] + self.judge.counts[3] == 0 {
            if self.judge.counts[1] == 0 {
                JUDGE_LINE_PERFECT_COLOR
            } else {
                JUDGE_LINE_GOOD_COLOR
            }
        } else {
            WHITE
        };
        self.chart.update(&mut self.res);
        Ok(())
    }

    pub fn render(&mut self, dt: Option<f32>) -> Result<()> {
        let res = &mut self.res;
        let dim = (self.get_size_fn)();
        if res.update_size(dim) {
            set_camera(&res.camera);
        }
        push_camera_state();
        set_default_camera();
        self.gl.quad_gl.render_pass(res.camera.render_pass());
        self.gl
            .quad_gl
            .viewport(Some((0, 0, dim.0 as _, dim.1 as _)));
        self.gl.quad_gl.push_model_matrix(Mat4::from_scale(vec3(
            screen_width() / dim.0 as f32,
            screen_height() / dim.1 as f32,
            1.,
        )));
        {
            let sw = dim.0 as f32;
            let sh = dim.1 as f32;
            let bw = res.background.width();
            let bh = res.background.height();
            let s = (sw / bw).max(sh / bh);
            draw_texture_ex(
                res.background,
                (sw - bw * s) / 2.,
                (sh - bh * s) / 2.,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(bw * s, bh * s)),
                    ..Default::default()
                },
            );
            draw_rectangle(0., 0., sw, sh, Color::new(0., 0., 0., 0.3));
        }
        self.gl.quad_gl.pop_model_matrix();
        pop_camera_state();

        self.gl.quad_gl.viewport(res.camera.viewport);
        let h = 1. / res.aspect_ratio;
        draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.6));
        self.chart.render(res);
        self.bad_notes.retain(|dummy| dummy.render(res));
        let dt = dt.unwrap_or_else(get_frame_time);
        if res.config.particle {
            res.emitter.draw(vec2(0., 0.), dt);
            res.emitter_square.draw(vec2(0., 0.), dt);
        }
        Ok(())
    }

    pub fn ui(&mut self, interactive: bool) -> Result<()> {
        let t = self.get_time();
        let res = &mut self.res;
        let eps = 2e-2 / res.aspect_ratio;
        let top = -1. / res.aspect_ratio;
        let pause_w = 0.015;
        let pause_h = pause_w * 3.2;
        let pause_center = Point::new(pause_w * 3.5 - 1., top + eps * 2.8 + pause_h / 2.);
        if interactive
            && self.pause_time.is_none()
            && Judge::get_touches(res).into_iter().any(|touch| {
                matches!(touch.phase, TouchPhase::Started) && {
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    (pause_center - p).norm() < 0.05
                }
            })
        {
            res.audio.pause(&mut self.audio_handle)?;
            self.pause_time = Some(t);
        }
        res.with_model(
            Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)),
            |res| {
                res.apply_model(|| {
                    let margin = 0.03;
                    draw_text_aligned(
                        res,
                        &format!("{:07}", self.judge.score()),
                        1. - margin,
                        top + eps * 2.8,
                        (1., 0.),
                        0.8,
                        WHITE,
                    );
                    draw_rectangle(pause_w * 2.2 - 1., top + eps * 3.5, pause_w, pause_h, WHITE);
                    draw_rectangle(pause_w * 4.2 - 1., top + eps * 3.5, pause_w, pause_h, WHITE);
                    if self.judge.combo >= 2 {
                        let rect = draw_text_aligned(
                            res,
                            &self.judge.combo.to_string(),
                            0.,
                            top + eps * 2.6,
                            (0.5, 0.),
                            1.,
                            WHITE,
                        );
                        draw_text_aligned(
                            res,
                            if res.config.autoplay {
                                "AUTOPLAY"
                            } else {
                                "COMBO"
                            },
                            0.,
                            rect.y + eps * 2.1,
                            (0.5, 0.),
                            0.4,
                            WHITE,
                        );
                    }
                    draw_text_aligned(
                        res,
                        &res.info.name,
                        -1. + margin,
                        -top - eps * 2.8,
                        (0., 1.),
                        0.5,
                        WHITE,
                    );
                    draw_text_aligned(
                        res,
                        &res.info.level,
                        1. - margin,
                        -top - eps * 2.8,
                        (1., 1.),
                        0.5,
                        WHITE,
                    );
                    let hw = 0.003;
                    let height = eps * 1.2;
                    let dest = 2. * res.time / res.track_length;
                    draw_rectangle(-1., top, dest, height, Color::new(1., 1., 1., 0.6));
                    draw_rectangle(-1. + dest - hw, top, hw * 2., height, WHITE);
                });
            },
        );
        if self.pause_time.is_some() {
            let h = 1. / res.aspect_ratio;
            draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.6));
            let s = 0.06;
            let w = 0.05;
            draw_texture_ex(
                res.icon_back,
                -s * 3. - w,
                -s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            draw_texture_ex(
                res.icon_retry,
                -s,
                -s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            draw_texture_ex(
                res.icon_resume,
                s + w,
                -s,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            if interactive {
                match Judge::get_touches(res)
                    .into_iter()
                    .filter_map(|touch| {
                        if !matches!(touch.phase, TouchPhase::Started) {
                            return None;
                        }
                        let p = touch.position;
                        let p = Point::new(p.x, p.y);
                        for i in -1..=1 {
                            let ct = Point::new((s * 2. + w) * i as f32, 0.);
                            let d = p - ct;
                            if d.x.abs() <= s && d.y.abs() <= s {
                                return Some(i);
                            }
                        }
                        None
                    })
                    .next()
                {
                    Some(-1) => {
                        self.should_exit = true;
                    }
                    Some(0) => {
                        self.judge.reset();
                        self.chart.reset();
                        res.judge_line_color = JUDGE_LINE_PERFECT_COLOR;
                        res.audio.resume(&mut self.audio_handle)?;
                        res.audio.seek_to(&mut self.audio_handle, 0.)?;
                        self.start_time = t;
                        self.pause_time = None;
                        self.time_errors.clear();
                        self.time_errors_sum = 0.;
                    }
                    Some(1) => {
                        self.pause_time = None;
                        res.audio.resume(&mut self.audio_handle)?;
                        res.time -= 3.;
                        let dst = (res.audio.position(&self.audio_handle)? - 3.).max(0.);
                        res.audio.seek_to(&mut self.audio_handle, dst)?;
                        self.start_time = t - dst;
                        self.pause_rewind = Some(self.start_time + dst - 0.2);
                    }
                    _ => {}
                }
            }
        }
        if let Some(time) = self.pause_rewind {
            let dt = t - time;
            let t = 3 - dt.floor() as i32;
            if t <= 0 {
                self.pause_rewind = None;
            } else {
                let a = (1. - dt as f32 / 3.) * 0.6;
                let h = 1. / res.aspect_ratio;
                draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., a));
                res.with_model(
                    Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)),
                    |res| {
                        res.apply_model(|| {
                            draw_text_aligned(res, &t.to_string(), 0., 0., (0.5, 0.5), 1., WHITE);
                        })
                    },
                );
            }
        }
        Ok(())
    }

    pub fn process_keys(&mut self) -> Result<()> {
        let t = self.get_time();
        let res = &mut self.res;
        if is_key_pressed(KeyCode::Space) {
            if res.audio.paused(&self.audio_handle)? {
                res.audio.resume(&mut self.audio_handle)?;
                self.start_time += t - self.pause_time.take().unwrap();
            } else {
                res.audio.pause(&mut self.audio_handle)?;
                self.pause_time = Some(t);
                self.pause_rewind = None;
            }
        }
        if is_key_pressed(KeyCode::Left) {
            res.time -= 1.;
            let dst = (res.audio.position(&self.audio_handle)? - 1.).max(0.);
            res.audio.seek_to(&mut self.audio_handle, dst)?;
            self.start_time = t - dst;
        }
        if is_key_pressed(KeyCode::Right) {
            res.time += 1.;
            let dst = res.audio.position(&self.audio_handle)? + 1.;
            res.audio.seek_to(&mut self.audio_handle, dst)?;
            self.start_time = t - dst;
        }
        if is_key_pressed(KeyCode::Q) {
            self.should_exit = true;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if self.pause_time.is_none() {
            self.res.audio.pause(&mut self.audio_handle)?;
            self.pause_time = Some(self.get_time());
            self.pause_rewind = None;
        }
        Ok(())
    }
}
