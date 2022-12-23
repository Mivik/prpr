use super::main::ChartItem;
use crate::{dir, get_data};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    core::Tweenable,
    ext::{poll_future, screen_aspect, JoinToString, SafeTexture, ScaleType},
    fs,
    scene::{LoadingScene, NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Scroll, Ui},
};
use std::{future::Future, pin::Pin};

const FADEIN_TIME: f32 = 0.3;

pub struct TrashBin {
    icon_delete: SafeTexture,
    icon_question: SafeTexture,
    button: RectButton,
    pub clicked: bool,
    height: f32,
    offset: f32,
    time: f32,
}

impl TrashBin {
    pub const TRANSIT_TIME: f32 = 0.2;
    pub const WAIT_TIME: f32 = 1.;

    pub fn new(icon_delete: SafeTexture, icon_question: SafeTexture) -> Self {
        Self {
            icon_delete,
            icon_question,
            button: RectButton::new(),
            clicked: false,
            height: 0.,
            offset: 0.,
            time: f32::INFINITY,
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) {
        if self.button.touch(touch) {
            if (0.0..Self::WAIT_TIME).contains(&(t - self.time - Self::TRANSIT_TIME)) {
                // delete
                self.clicked = true;
            } else if self.time.is_infinite() {
                self.time = t;
            }
        }
    }

    pub fn update(&mut self, t: f32) {
        if self.time.is_infinite() {
            self.offset = 0.;
        } else {
            let p = ((t - self.time - Self::WAIT_TIME - Self::TRANSIT_TIME) / Self::TRANSIT_TIME).min(1.);
            if p >= 0. {
                self.offset = (1. - p).powi(3) * self.height;
                if p >= 1. {
                    self.time = f32::INFINITY;
                }
            } else {
                let p = 1. - (1. - ((t - self.time) / Self::TRANSIT_TIME).min(1.)).powi(3);
                self.offset = p * self.height;
            }
        }
    }

    pub fn render(&mut self, ui: &mut Ui, mut rect: Rect, color: Color) {
        self.button.set(ui, rect);
        self.height = rect.h;
        ui.scissor(Some(rect));
        rect.y -= self.offset;
        ui.fill_rect(rect, (*self.icon_delete, rect, ScaleType::Fit, color));
        rect.y += rect.h;
        ui.fill_rect(rect, (*self.icon_question, rect, ScaleType::Fit, color));
        ui.scissor(None);
    }
}

pub struct SongScene {
    chart: ChartItem,
    illustration: SafeTexture,
    icon_back: SafeTexture,
    icon_play: SafeTexture,
    bin: TrashBin,

    back_button: RectButton,
    play_button: RectButton,

    scroll: Scroll,

    future: Option<Pin<Box<dyn Future<Output = Result<LoadingScene>>>>>,

    target: Option<RenderTarget>,
    first_in: bool,

    next_scene: Option<NextScene>,
}

impl SongScene {
    pub fn new(chart: ChartItem, illustration: SafeTexture, icon_back: SafeTexture, icon_play: SafeTexture, bin: TrashBin) -> Self {
        Self {
            chart,
            illustration,
            icon_back,
            icon_play,
            bin,

            back_button: RectButton::new(),
            play_button: RectButton::new(),

            scroll: Scroll::new(),

            future: None,

            target: None,
            first_in: true,

            next_scene: None,
        }
    }

    fn scroll_progress(&self) -> f32 {
        (self.scroll.y_scroller.offset() / (1. / screen_aspect() * 0.7)).max(0.).min(1.)
    }

    fn ui(&mut self, ui: &mut Ui, t: f32) {
        let sp = self.scroll_progress();
        let r = Rect::new(-1., -ui.top, 2., ui.top * 2.);
        ui.fill_rect(r, (*self.illustration, r));
        ui.fill_rect(r, Color::new(0., 0., 0., f32::tween(&0.55, &0.8, sp)));
        let p = ((t + FADEIN_TIME) / FADEIN_TIME).min(1.);
        let color = Color::new(1., 1., 1., p * (1. - sp));
        let r = Rect::new(-1. + 0.02, -ui.top + 0.02, 0.07, 0.07);
        ui.fill_rect(r, (*self.icon_back, r, ScaleType::Scale, color));
        self.back_button.set(ui, r);

        let s = 0.1;
        let r = Rect::new(-s, -s, s * 2., s * 2.);
        ui.fill_rect(r, (*self.icon_play, r, ScaleType::Fit, color));
        self.play_button.set(ui, r);

        ui.scope(|ui| {
            ui.dx(1. - 0.03);
            ui.dy(-ui.top + 0.03);
            let s = 0.08;
            let r = Rect::new(-s, 0., s, s);
            self.bin.render(ui, r, color);
        });

        let color = Color::new(1., 1., 1., p);
        ui.scope(|ui| {
            ui.dx(-1.);
            ui.dy(-ui.top);
            self.scroll.size((2., ui.top * 2.));
            self.scroll.render(ui, |ui| {
                ui.dx(0.06);
                let top = ui.top * 2.;
                let r = ui
                    .text(&self.chart.info.name)
                    .pos(0., top - 0.06)
                    .anchor(0., 1.)
                    .size(1.4)
                    .color(color)
                    .draw();
                ui.text(&self.chart.info.composer)
                    .pos(0., r.y - 0.02)
                    .anchor(0., 1.)
                    .size(0.4)
                    .color(Color::new(1., 1., 1., 0.77 * p))
                    .draw();
                ui.dy(top + 0.03);
                let r = ui
                    .text(format!(
                        "{}\n\n{}\n\n难度：{} ({:.1})\n曲师：{}\n插图：{}",
                        self.chart.info.intro,
                        self.chart.info.tags.iter().map(|it| format!("#{it}")).join(" "),
                        self.chart.info.level,
                        self.chart.info.difficulty,
                        self.chart.info.composer,
                        self.chart.info.illustrator
                    ))
                    .multiline()
                    .max_width(2. - 0.06 * 2.)
                    .size(0.5)
                    .color(Color::new(1., 1., 1., 0.77))
                    .draw();
                ui.dy(r.h + 0.02);
                (2., top + r.h + 0.1)
            });
        });
    }
}

impl Scene for SongScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.target = target;
        tm.reset();
        if self.first_in {
            self.first_in = false;
            tm.seek_to(-FADEIN_TIME as _);
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: Touch) -> Result<()> {
        if tm.now() < 0. {
            return Ok(());
        }
        if self.scroll_progress() < 0.4 {
            self.bin.touch(&touch, tm.now() as _);
            if self.back_button.touch(&touch) {
                self.next_scene = Some(NextScene::Pop);
            }
            if self.play_button.touch(&touch) {
                let fs = if let Some(name) = self.chart.path.strip_prefix(':') {
                    fs::fs_from_assets(name)?
                } else {
                    fs::fs_from_file(&std::path::Path::new(&format!("{}/{}", dir::charts()?, self.chart.path)))?
                };
                self.future = Some(Box::pin(async move {
                    let (info, fs) = fs::load_info(fs).await?;
                    LoadingScene::new(info, get_data().unwrap().config.clone(), fs, None).await
                }));
            }
        }
        self.scroll.touch(touch, tm.now() as _);
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if self.bin.clicked {
            self.next_scene = Some(NextScene::Pop);
            super::main::SHOULD_DELETE.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if let Some(future) = &mut self.future {
            if let Some(scene) = poll_future(future.as_mut()) {
                self.future = None;
                self.next_scene = Some(NextScene::Overlay(Box::new(scene?)));
            }
        }
        self.bin.update(tm.now() as _);
        self.scroll.update(tm.now() as _);
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: self.target,
            ..Default::default()
        });
        self.ui(&mut Ui::new(), tm.now() as _);
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
