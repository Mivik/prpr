use super::{draw_background, EndingScene, NextScene, Scene};
use crate::{
    audio::{Audio, AudioHandle, PlayParams},
    config::Config,
    core::{BadNote, Chart, Matrix, Point, Resource, Vector, JUDGE_LINE_GOOD_COLOR, JUDGE_LINE_PERFECT_COLOR},
    ext::{draw_text_aligned, screen_aspect, SafeTexture},
    fs::FileSystem,
    info::{ChartFormat, ChartInfo},
    judge::Judge,
    parse::{parse_pec, parse_phigros, parse_rpe},
    time::TimeManager, ui::Ui,
};
use anyhow::{bail, Context, Result};
use concat_string::concat_string;
use macroquad::{prelude::*, window::InternalGlContext};
use std::rc::Rc;

const WAIT_TIME: f32 = 0.5;
const AFTER_TIME: f32 = 0.7;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn on_game_start();
}

enum State {
    Starting,
    BeforeMusic,
    Playing,
    Ending,
}

pub struct GameScene {
    should_exit: bool,
    next_scene: Option<Box<dyn Scene>>,

    pub res: Resource,
    pub chart: Chart,
    pub judge: Judge,
    pub gl: InternalGlContext<'static>,

    pub audio_handle: AudioHandle,

    get_size_fn: Rc<dyn Fn() -> (u32, u32)>,

    state: State,
    last_update_time: f64,
    pause_rewind: Option<f64>,

    bad_notes: Vec<BadNote>,
}

macro_rules! reset {
    ($self:ident, $res:expr, $tm:ident) => {{
        $self.bad_notes.clear();
        $self.judge.reset();
        $self.chart.reset();
        $res.judge_line_color = JUDGE_LINE_PERFECT_COLOR;
        $res.audio.resume(&mut $self.audio_handle)?;
        $res.audio.seek_to(&mut $self.audio_handle, 0.)?;
        $res.audio.pause(&mut $self.audio_handle)?;
        $tm.reset();
        $self.last_update_time = $tm.now();
        $self.state = State::Starting;
    }};
}

impl GameScene {
    pub const BEFORE_TIME: f32 = 0.7;
    pub const FADEOUT_TIME: f32 = WAIT_TIME + AFTER_TIME + 0.3;

    pub async fn load_chart(fs: &mut Box<dyn FileSystem>, info: &ChartInfo) -> Result<Chart> {
        async fn load_chart_bytes(fs: &mut Box<dyn FileSystem>, info: &ChartInfo) -> Result<Vec<u8>> {
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
        let text = String::from_utf8(load_chart_bytes(fs, info).await.context("Failed to load chart")?)?;
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
        match format {
            ChartFormat::Rpe => parse_rpe(&text).await,
            ChartFormat::Pgr => parse_phigros(&text),
            ChartFormat::Pec => parse_pec(&text),
        }
    }

    pub async fn new(
        info: ChartInfo,
        config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<SafeTexture>,
        background: SafeTexture,
        illustration: SafeTexture,
        font: Font,
        get_size_fn: Rc<dyn Fn() -> (u32, u32)>,
    ) -> Result<Self> {
        let chart = Self::load_chart(&mut fs, &info).await?;

        let mut res = Resource::new(config, info, fs, player, background, illustration, font)
            .await
            .context("Failed to load resources")?;

        let judge = Judge::new(&chart);

        let audio_handle = Self::new_handle(&mut res)?;
        Ok(Self {
            should_exit: false,
            next_scene: None,

            res,
            chart,
            judge,
            gl: unsafe { get_internal_gl() },

            audio_handle,

            get_size_fn,

            state: State::Starting,
            last_update_time: 0.,
            pause_rewind: None,

            bad_notes: Vec::new(),
        })
    }

    fn new_handle(res: &mut Resource) -> Result<AudioHandle> {
        let mut audio_handle = res.audio.play(
            &res.music,
            PlayParams {
                volume: res.config.volume_music as _,
                playback_rate: res.config.speed as _,
                ..Default::default()
            },
        )?;
        res.audio.pause(&mut audio_handle)?;
        Ok(audio_handle)
    }

    fn ui(&mut self, tm: &mut TimeManager) -> Result<()> {
        let c = Color::new(1., 1., 1., self.res.alpha);
        let t = tm.now();
        let res = &mut self.res;
        let eps = 2e-2 / res.aspect_ratio;
        let top = -1. / res.aspect_ratio;
        let pause_w = 0.015;
        let pause_h = pause_w * 3.2;
        let pause_center = Point::new(pause_w * 3.5 - 1., top + eps * 2.8 + pause_h / 2.);
        if Self::interactive(res, &self.state)
            && !tm.paused()
            && self.pause_rewind.is_none()
            && Judge::get_touches().into_iter().any(|touch| {
                matches!(touch.phase, TouchPhase::Started) && {
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    (pause_center - p).norm() < 0.05
                }
            })
        {
            res.audio.pause(&mut self.audio_handle)?;
            tm.pause();
        }
        res.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
            res.apply_model(|| {
                let margin = 0.03;
                draw_text_aligned(res.font, &format!("{:07}", self.judge.score()), 1. - margin, top + eps * 2.8, (1., 0.), 0.8, c);
                draw_rectangle(pause_w * 2.2 - 1., top + eps * 3.5, pause_w, pause_h, c);
                draw_rectangle(pause_w * 4.2 - 1., top + eps * 3.5, pause_w, pause_h, c);
                if self.judge.combo >= 3 {
                    let rect = draw_text_aligned(res.font, &self.judge.combo.to_string(), 0., top + eps * 2.6, (0.5, 0.), 1., c);
                    draw_text_aligned(
                        res.font,
                        if res.config.autoplay { "AUTOPLAY" } else { "COMBO" },
                        0.,
                        rect.y + rect.h + eps * 1.1,
                        (0.5, 0.),
                        0.4,
                        c,
                    );
                }
                draw_text_aligned(res.font, &res.info.name, -1. + margin, -top - eps * 2.8, (0., 1.), 0.5, c);
                draw_text_aligned(res.font, &res.info.level, 1. - margin, -top - eps * 2.8, (1., 1.), 0.5, c);
                let hw = 0.003;
                let height = eps * 1.2;
                let dest = 2. * res.time / res.track_length;
                draw_rectangle(-1., top, dest, height, Color::new(1., 1., 1., 0.6 * res.alpha));
                draw_rectangle(-1. + dest - hw, top, hw * 2., height, c);
            });
        });
        if tm.paused() {
            let h = 1. / res.aspect_ratio;
            draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.6));
            let s = 0.06;
            let w = 0.05;
            draw_texture_ex(
                *res.icon_back,
                -s * 3. - w,
                -s,
                c,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            draw_texture_ex(
                *res.icon_retry,
                -s,
                -s,
                c,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            draw_texture_ex(
                *res.icon_resume,
                s + w,
                -s,
                c,
                DrawTextureParams {
                    dest_size: Some(vec2(s * 2., s * 2.)),
                    ..Default::default()
                },
            );
            if Self::interactive(res, &self.state) {
                match Judge::get_touches()
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
                        reset!(self, res, tm);
                    }
                    Some(1) => {
                        res.audio.resume(&mut self.audio_handle)?;
                        res.time -= 3.;
                        let dst = (res.audio.position(&self.audio_handle)? - 3.).max(0.);
                        res.audio.seek_to(&mut self.audio_handle, dst)?;
                        tm.resume();
                        tm.seek_to(t - 3.);
                        self.pause_rewind = Some(tm.now() - 0.2);
                    }
                    _ => {}
                }
            }
        }
        if let Some(time) = self.pause_rewind {
            let dt = tm.now() - time;
            let t = 3 - dt.floor() as i32;
            if t <= 0 {
                self.pause_rewind = None;
            } else {
                let a = (1. - dt as f32 / 3.) * 1.;
                let h = 1. / res.aspect_ratio;
                draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., a));
                res.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
                    res.apply_model(|| {
                        draw_text_aligned(res.font, &t.to_string(), 0., 0., (0.5, 0.5), 1., c);
                    })
                });
            }
        }
        Ok(())
    }

    fn interactive(res: &Resource, state: &State) -> bool {
        res.config.interactive && matches!(state, State::Playing)
    }
}

impl Scene for GameScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        on_game_start();
        self.audio_handle = Self::new_handle(&mut self.res)?;
        self.res.camera.render_target = target;
        tm.speed = self.res.config.speed as _;
        reset!(self, self.res, tm);
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !tm.paused() {
            self.res.audio.pause(&mut self.audio_handle)?;
            tm.pause();
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if matches!(self.state, State::Playing) {
            tm.update(self.res.audio.position(&self.audio_handle)?);
        }
        let offset = self.chart.offset + self.res.config.offset;
        let time = tm.now() as f32;
        let time = match self.state {
            State::Starting => {
                if time >= Self::BEFORE_TIME {
                    self.res.alpha = 1.;
                    self.state = State::BeforeMusic;
                    tm.reset();
                    tm.seek_to(offset.min(0.) as f64);
                    self.last_update_time = tm.real_time();
                    tm.now() as f32
                } else {
                    self.res.alpha = 1. - (1. - time / Self::BEFORE_TIME).powi(3);
                    offset
                }
            }
            State::BeforeMusic => {
                if time >= 0.0 {
                    self.res.audio.resume(&mut self.audio_handle)?;
                    self.res.audio.seek_to(&mut self.audio_handle, time as f64)?;
                    self.state = State::Playing;
                }
                time
            }
            State::Playing => {
                if time > self.res.track_length + WAIT_TIME {
                    self.state = State::Ending;
                }
                time
            }
            State::Ending => {
                let t = time - self.res.track_length - WAIT_TIME;
                if t >= AFTER_TIME + 0.3 {
                    self.next_scene = Some(Box::new(EndingScene::new(
                        self.res.background.clone(),
                        self.res.illustration.clone(),
                        self.res.player.clone(),
                        self.res.font,
                        self.res.icons.clone(),
                        self.res.icon_retry.clone(),
                        self.res.icon_proceed.clone(),
                        self.res.info.clone(),
                        self.judge.result(),
                        self.res.challenge_icons[self.res.config.challenge_color.clone() as usize].clone(),
                        &self.res.config,
                        self.res.ending_bgm_bytes.clone(),
                    )?));
                }
                self.res.alpha = 1. - (t / AFTER_TIME).min(1.).powi(2);
                self.res.track_length
            }
        };
        let time = (time - offset).max(0.);
        self.res.time = time;
        if !tm.paused() && self.pause_rewind.is_none() {
            self.judge.update(&mut self.res, &mut self.chart, &mut self.bad_notes);
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
        self.res.judge_line_color.a *= self.res.alpha;
        self.chart.update(&mut self.res);
        let res = &mut self.res;
        if Self::interactive(res, &self.state) {
            if is_key_pressed(KeyCode::Space) {
                if res.audio.paused(&self.audio_handle)? {
                    res.audio.resume(&mut self.audio_handle)?;
                    tm.resume();
                } else {
                    res.audio.pause(&mut self.audio_handle)?;
                    tm.pause();
                }
            }
            if is_key_pressed(KeyCode::Left) {
                res.time -= 1.;
                let dst = (res.audio.position(&self.audio_handle)? - 1.).max(0.);
                res.audio.seek_to(&mut self.audio_handle, dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Right) {
                res.time += 5.;
                let dst = (res.audio.position(&self.audio_handle)? + 5.).min(res.track_length as f64);
                res.audio.seek_to(&mut self.audio_handle, dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Q) {
                self.should_exit = true;
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, _ui: &mut Ui) -> Result<()> {
        let res = &mut self.res;
        let dim = (self.get_size_fn)();
        if res.update_size(dim) {
            set_camera(&res.camera);
        }
        push_camera_state();
        self.gl.quad_gl.viewport(None);
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: res.camera.render_target,
            ..Default::default()
        });
        draw_background(*res.background);
        pop_camera_state();

        self.gl.quad_gl.viewport(res.camera.viewport);
        let h = 1. / res.aspect_ratio;
        draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., res.alpha * 0.6));
        self.chart.render(res);
        self.bad_notes.retain(|dummy| dummy.render(res));
        let t = tm.real_time();
        let dt = (t - std::mem::replace(&mut self.last_update_time, t)) as f32;
        if res.config.particle {
            res.emitter.draw(dt);
        }
        self.ui(tm)
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.should_exit {
            NextScene::Pop
        } else if let Some(scene) = self.next_scene.take() {
            tm.speed = 1.0;
            NextScene::Overlay(scene)
        } else {
            NextScene::None
        }
    }
}
