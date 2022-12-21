use super::main::ChartItem;
use crate::{dir, get_data};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::core::Tweenable;
use prpr::ext::{poll_future, screen_aspect, JoinToString, ScaleType};
use prpr::scene::LoadingScene;
use prpr::ui::{RectButton, Scroll, Ui};
use prpr::{ext::SafeTexture, scene::Scene, time::TimeManager};
use prpr::{fs, scene::NextScene};
use std::future::Future;
use std::pin::Pin;

const FADEIN_TIME: f32 = 0.3;

pub struct SongScene {
    chart: ChartItem,
    illustration: SafeTexture,
    icon_back: SafeTexture,
    icon_play: SafeTexture,

    back_button: RectButton,
    play_button: RectButton,

    scroll: Scroll,

    future: Option<Pin<Box<dyn Future<Output = Result<LoadingScene>>>>>,

    target: Option<RenderTarget>,
    first_in: bool,

    next_scene: Option<NextScene>,
}

impl SongScene {
    pub fn new(chart: ChartItem, illustration: SafeTexture, icon_back: SafeTexture, icon_play: SafeTexture) -> Self {
        Self {
            chart,
            illustration,
            icon_back,
            icon_play,

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
                ui.text(format!(
                    "{}\n\n{}\n\n曲师：{}\n插图：{}",
                    self.chart.info.intro,
                    self.chart.info.tags.iter().map(|it| format!("#{it}")).join(" "),
                    self.chart.info.composer,
                    self.chart.info.illustrator
                ))
                .multiline()
                .max_width(2. - 0.06 * 2.)
                .size(0.5)
                .color(Color::new(1., 1., 1., 0.77))
                .draw();
                (2., ui.top * 3.)
            });
        });
    }
}

impl Scene for SongScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.target = target;
        if self.first_in {
            self.first_in = false;
            tm.reset();
            tm.seek_to(-FADEIN_TIME as _);
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: Touch) -> Result<()> {
        if tm.now() < 0. {
            return Ok(());
        }
        if self.scroll_progress() < 0.4 {
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
        if let Some(future) = &mut self.future {
            if let Some(scene) = poll_future(future.as_mut()) {
                self.future = None;
                self.next_scene = Some(NextScene::Overlay(Box::new(scene?)));
            }
        }
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
