use super::{draw_background, EndingScene, NextScene, Scene};
use crate::{
    config::Config,
    core::{copy_fbo, BadNote, Chart, Effect, Point, Resource, UIElement, Vector, JUDGE_LINE_GOOD_COLOR, JUDGE_LINE_PERFECT_COLOR},
    ext::{draw_text_aligned, screen_aspect, RectExt, SafeTexture},
    fs::FileSystem,
    info::{ChartFormat, ChartInfo},
    judge::Judge,
    parse::{parse_pec, parse_phigros, parse_rpe},
    time::TimeManager,
    ui::Ui,
};
use anyhow::{bail, Context, Result};
use concat_string::concat_string;
use macroquad::{prelude::*, window::InternalGlContext};
use sasa::{Music, MusicParams};
use std::{ops::DerefMut, rc::Rc};

const WAIT_TIME: f32 = 0.5;
const AFTER_TIME: f32 = 0.7;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn on_game_start();
}

#[derive(PartialEq, Eq)]
pub enum GameMode {
    Normal,
    TweakOffset,
}

enum State {
    Starting,
    BeforeMusic,
    Playing,
    Ending,
}

pub struct GameScene {
    should_exit: bool,
    next_scene: Option<NextScene>,

    pub mode: GameMode,
    pub res: Resource,
    pub chart: Chart,
    pub judge: Judge,
    pub gl: InternalGlContext<'static>,
    info_offset: f32,
    compatible_mode: bool,
    effects: Vec<Effect>,

    pub music: Music,

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
        $self.music.pause()?;
        $self.music.seek_to(0.)?;
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
        let mut chart = match format {
            ChartFormat::Rpe => parse_rpe(&text, fs.deref_mut()).await,
            ChartFormat::Pgr => parse_phigros(&text),
            ChartFormat::Pec => parse_pec(&text),
        }?;
        chart.settings.hold_partial_cover = info.hold_partial_cover;
        Ok(chart)
    }

    pub async fn new(
        mode: GameMode,
        info: ChartInfo,
        mut config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<SafeTexture>,
        background: SafeTexture,
        illustration: SafeTexture,
        font: Font,
        get_size_fn: Rc<dyn Fn() -> (u32, u32)>,
    ) -> Result<Self> {
        if matches!(mode, GameMode::TweakOffset) {
            config.autoplay = true;
        }
        let mut chart = Self::load_chart(&mut fs, &info).await?;
        let effects = std::mem::take(&mut chart.global_effects);
        if config.fxaa {
            chart
                .effects
                .push(Effect::new(0.0..f32::INFINITY, include_str!("fxaa.glsl"), Vec::new(), false).unwrap());
        }

        let info_offset = info.offset;
        let mut res = Resource::new(config, info, fs, player, background, illustration, font, chart.effects.is_empty() && effects.is_empty())
            .await
            .context("Failed to load resources")?;

        let judge = Judge::new(&chart);

        let music = Self::new_music(&mut res)?;
        Ok(Self {
            should_exit: false,
            next_scene: None,

            mode,
            res,
            chart,
            judge,
            gl: unsafe { get_internal_gl() },
            compatible_mode: false,
            effects,
            info_offset,

            music,

            get_size_fn,

            state: State::Starting,
            last_update_time: 0.,
            pause_rewind: None,

            bad_notes: Vec::new(),
        })
    }

    fn new_music(res: &mut Resource) -> Result<Music> {
        res.audio.create_music(
            res.music.clone(),
            MusicParams {
                amplifier: res.config.volume_music as _,
                playback_rate: res.config.speed as _,
                ..Default::default()
            },
        )
    }

    fn ui(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let c = Color::new(1., 1., 1., self.res.alpha);
        let res = &mut self.res;
        let eps = 2e-2 / res.aspect_ratio;
        let top = -1. / res.aspect_ratio;
        let pause_w = 0.015;
        let pause_h = pause_w * 3.2;
        let pause_center = Point::new(pause_w * 3.5 - 1., top + eps * 2.8 + pause_h / 2.);
        if Self::interactive(res, &self.state)
            && !tm.paused()
            && self.pause_rewind.is_none()
            && Judge::get_touches().iter().any(|touch| {
                touch.phase == TouchPhase::Started && {
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    (pause_center - p).norm() < 0.05
                }
            })
        {
            self.music.pause()?;
            tm.pause();
        }
        let margin = 0.03;

        self.chart.with_element(ui, res, UIElement::Score, |ui, alpha, scale| {
            ui.text(format!("{:07}", self.judge.score()))
                .pos(1. - margin, top + eps * 2.8)
                .anchor(1., 0.)
                .size(0.8)
                .color(Color { a: c.a * alpha, ..c })
                .scale(scale)
                .draw();
        });
        self.chart.with_element(ui, res, UIElement::Pause, |ui, alpha, scale| {
            let mut r = Rect::new(pause_w * 3.0 - 1., top + eps * 3.5, pause_w, pause_h);
            let ct = Vector::new(r.x + pause_w, r.y + r.h / 2.);
            let c = Color { a: c.a * alpha, ..c };
            ui.with(scale.prepend_translation(&-ct).append_translation(&ct), |ui| {
                ui.fill_rect(r, c);
                r.x += pause_w * 2.;
                ui.fill_rect(r, c);
            });
        });
        if self.judge.combo >= 3 {
            self.chart.with_element(ui, res, UIElement::ComboNumber, |ui, alpha, scale| {
                ui.text(self.judge.combo.to_string())
                    .pos(0., top + eps * 2.6)
                    .anchor(0.5, 0.)
                    .color(Color { a: c.a * alpha, ..c })
                    .scale(scale)
                    .draw();
            });
            self.chart.with_element(ui, res, UIElement::Combo, |ui, alpha, scale| {
                ui.text(if res.config.autoplay { "AUTOPLAY" } else { "COMBO" })
                    .pos(0., top + 0.09 + eps * 1.1)
                    .anchor(0.5, 0.)
                    .size(0.4)
                    .color(Color { a: c.a * alpha, ..c })
                    .scale(scale)
                    .draw();
            });
        }
        let lf = -1. + margin;
        let bt = -top - eps * 2.8;
        self.chart.with_element(ui, res, UIElement::Name, |ui, alpha, scale| {
            ui.text(&res.info.name)
                .pos(lf, bt)
                .anchor(0., 1.)
                .size(0.5)
                .color(Color { a: c.a * alpha, ..c })
                .scale(scale)
                .draw();
        });
        self.chart.with_element(ui, res, UIElement::Level, |ui, alpha, scale| {
            ui.text(&res.info.level)
                .pos(-lf, bt)
                .anchor(1., 1.)
                .size(0.5)
                .color(Color { a: c.a * alpha, ..c })
                .scale(scale)
                .draw();
        });
        let hw = 0.003;
        let height = eps * 1.2;
        let dest = 2. * res.time / res.track_length;
        self.chart.with_element(ui, res, UIElement::Bar, |ui, alpha, scale| {
            let ct = Vector::new(0., top + height / 2.);
            ui.with(scale.prepend_translation(&-ct).append_translation(&ct), |ui| {
                ui.fill_rect(Rect::new(-1., top, dest, height), Color::new(1., 1., 1., 0.6 * res.alpha * alpha));
                ui.fill_rect(Rect::new(-1. + dest - hw, top, hw * 2., height), Color { a: c.a * alpha, ..c });
            });
        });
        Ok(())
    }

    fn overlay_ui(&mut self, tm: &mut TimeManager) -> Result<()> {
        let c = Color::new(1., 1., 1., self.res.alpha);
        let t = tm.now();
        let res = &mut self.res;
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
                let mut clicked = None;
                for touch in Judge::get_touches() {
                    if touch.phase != TouchPhase::Started {
                        continue;
                    }
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    for i in -1..=1 {
                        let ct = Point::new((s * 2. + w) * i as f32, 0.);
                        let d = p - ct;
                        if d.x.abs() <= s && d.y.abs() <= s {
                            clicked = Some(i);
                            break;
                        }
                    }
                }
                match clicked {
                    Some(-1) => {
                        self.should_exit = true;
                    }
                    Some(0) => {
                        reset!(self, res, tm);
                    }
                    Some(1) => {
                        self.music.play()?;
                        res.time -= 3.;
                        let dst = self.music.position() - 3.;
                        if dst < 0. {
                            self.music.pause()?;
                            self.state = State::BeforeMusic;
                        } else {
                            self.music.seek_to(dst)?;
                        }
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
                draw_text_aligned(res.font, &t.to_string(), 0., 0., (0.5, 0.5), 1., c);
            }
        }
        Ok(())
    }

    fn interactive(res: &Resource, state: &State) -> bool {
        res.config.interactive && matches!(state, State::Playing)
    }

    fn tweak_offset(&mut self, ui: &mut Ui, ita: bool) {
        ui.scope(|ui| {
            let width = 0.55;
            let height = 0.4;
            ui.dx(1. - width - 0.02);
            ui.dy(ui.top - height - 0.02);
            ui.fill_rect(Rect::new(0., 0., width, height), GRAY);
            ui.dy(0.02);
            ui.text("调整延迟").pos(width / 2., 0.).anchor(0.5, 0.).size(0.7).draw();
            ui.dy(0.16);
            let r = ui
                .text(format!("{}ms", (self.info_offset * 1000.).round() as i32))
                .pos(width / 2., 0.)
                .anchor(0.5, 0.)
                .size(0.6)
                .no_baseline()
                .draw();
            let d = 0.14;
            if ui.button("lg_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.026), "-") && ita {
                self.info_offset -= 0.05;
            }
            if ui.button("lg_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.026), "+") && ita {
                self.info_offset += 0.05;
            }
            let d = 0.08;
            if ui.button("sm_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.022), "-") && ita {
                self.info_offset -= 0.005;
            }
            if ui.button("sm_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.022), "+") && ita {
                self.info_offset += 0.005;
            }
            let d = 0.03;
            if ui.button("ti_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.017), "-") && ita {
                self.info_offset -= 0.001;
            }
            if ui.button("ti_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.017), "+") && ita {
                self.info_offset += 0.001;
            }
            ui.dy(0.14);
            let pad = 0.02;
            let spacing = 0.01;
            let mut r = Rect::new(pad, 0., (width - pad * 2. - spacing * 2.) / 3., 0.06);
            if ui.button("cancel", r, "取消") {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(None::<f32>)));
            }
            r.x += r.w + spacing;
            if ui.button("reset", r, "重置") {
                self.info_offset = 0.;
            }
            r.x += r.w + spacing;
            if ui.button("save", r, "保存") {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(Some(self.info_offset))));
            }
        });
    }
}

impl Scene for GameScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        on_game_start();
        self.music = Self::new_music(&mut self.res)?;
        self.res.camera.render_target = target;
        tm.speed = self.res.config.speed as _;
        reset!(self, self.res, tm);
        set_camera(&self.res.camera);
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !tm.paused() {
            self.pause_rewind = None;
            self.music.pause()?;
            tm.pause();
        }
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !matches!(self.state, State::Playing) {
            tm.resume();
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.res.audio.recover_if_needed()?;
        if matches!(self.state, State::Playing) {
            tm.update(self.music.position() as f64);
        }
        let offset = self.chart.offset + self.res.config.offset + self.info_offset;
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
                    self.music.seek_to(time)?;
                    self.music.play()?;
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
                    self.next_scene = Some(match self.mode {
                        GameMode::Normal => NextScene::Overlay(Box::new(EndingScene::new(
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
                        )?)),
                        GameMode::TweakOffset => NextScene::PopWithResult(Box::new(None::<f32>)),
                    });
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
                if self.music.paused() {
                    self.music.play()?;
                    tm.resume();
                } else {
                    self.music.pause()?;
                    tm.pause();
                }
            }
            if is_key_pressed(KeyCode::Left) {
                res.time -= 1.;
                let dst = (self.music.position() - 1.).max(0.);
                self.music.seek_to(dst)?;
                tm.seek_to(dst as f64);
            }
            if is_key_pressed(KeyCode::Right) {
                res.time += 5.;
                let dst = (self.music.position() + 5.).min(res.track_length);
                self.music.seek_to(dst)?;
                tm.seek_to(dst as f64);
            }
            if is_key_pressed(KeyCode::Q) {
                self.should_exit = true;
            }
        }
        for e in &mut self.effects {
            e.update(&self.res);
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let res = &mut self.res;
        let asp = screen_aspect();
        let dim = (self.get_size_fn)();
        if res.update_size(dim) {
            set_camera(&res.camera);
        }

        let msaa = res.config.sample_count > 1;

        let chart_onto = res
            .chart_target
            .as_ref()
            .map(|it| if msaa { it.input() } else { it.output() })
            .or(res.camera.render_target);
        push_camera_state();
        self.gl.quad_gl.viewport(None);
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: chart_onto,
            ..Default::default()
        });
        clear_background(BLACK);
        draw_background(*res.background);
        pop_camera_state();

        self.gl.quad_gl.render_pass(chart_onto.map(|it| it.render_pass));
        self.gl.quad_gl.viewport(res.camera.viewport);

        let h = 1. / res.aspect_ratio;
        draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., res.alpha * 0.6));

        self.chart.render(res);

        self.gl.quad_gl.render_pass(
            res.chart_target
                .as_ref()
                .map(|it| it.output().render_pass)
                .or_else(|| res.camera.render_pass()),
        );

        self.bad_notes.retain(|dummy| dummy.render(res));
        let t = tm.real_time();
        let dt = (t - std::mem::replace(&mut self.last_update_time, t)) as f32;
        if res.config.particle {
            res.emitter.draw(dt);
        }
        self.ui(tm, ui)?;
        self.overlay_ui(tm)?;

        if self.mode == GameMode::TweakOffset {
            push_camera_state();
            self.gl.quad_gl.viewport(None);
            set_camera(&Camera2D {
                zoom: vec2(1., -screen_aspect()),
                render_target: self
                    .res
                    .chart_target
                    .as_ref()
                    .map(|it| it.output())
                    .or_else(|| self.res.camera.render_target),
                ..Default::default()
            });
            self.tweak_offset(ui, Self::interactive(&self.res, &self.state));
            pop_camera_state();
        }

        if !self.res.no_effect && !self.effects.is_empty() {
            push_camera_state();
            set_camera(&Camera2D {
                zoom: vec2(1., asp),
                ..Default::default()
            });
            for e in &self.effects {
                e.render(&mut self.res);
            }
            pop_camera_state();
        }
        if msaa || !self.res.no_effect {
            // render the texture onto screen
            if let Some(target) = &self.res.chart_target {
                self.gl.flush();
                if !self.compatible_mode {
                    if !copy_fbo(
                        target.output().render_pass.gl_internal_id(self.gl.quad_context),
                        self.res
                            .camera
                            .render_target
                            .map_or(0, |it| it.render_pass.gl_internal_id(self.gl.quad_context)),
                        dim,
                    ) {
                        self.compatible_mode = true;
                    }
                }
                if self.compatible_mode {
                    push_camera_state();
                    self.gl.quad_gl.viewport(None);
                    set_camera(&Camera2D {
                        zoom: vec2(1., screen_aspect()),
                        render_target: self.res.camera.render_target,
                        ..Default::default()
                    });
                    draw_texture_ex(
                        target.output().texture,
                        -1.,
                        -ui.top,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(2., ui.top * 2.)),
                            ..Default::default()
                        },
                    );
                    pop_camera_state();
                }
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.should_exit {
            if tm.paused() {
                tm.resume();
            }
            tm.speed = 1.0;
            match self.mode {
                GameMode::Normal => NextScene::Pop,
                GameMode::TweakOffset => NextScene::PopWithResult(Box::new(None::<f32>)),
            }
        } else if let Some(next_scene) = self.next_scene.take() {
            tm.speed = 1.0;
            next_scene
        } else {
            NextScene::None
        }
    }
}
