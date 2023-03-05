use crate::page::ChartItem;
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{screen_aspect, semi_black, semi_white, SafeTexture, ScaleType},
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Ui},
};

const FADE_IN_TIME: f32 = 0.3;

pub struct SongScene {
    chart: ChartItem,

    first_in: bool,

    back_btn: RectButton,
    icon_back: SafeTexture,

    next_scene: Option<NextScene>,
}

impl SongScene {
    pub fn new(chart: ChartItem, icon_back: SafeTexture) -> Self {
        Self {
            chart,

            first_in: true,

            back_btn: RectButton::new(),
            icon_back,

            next_scene: None,
        }
    }
}

impl Scene for SongScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.first_in {
            self.first_in = false;
            tm.seek_to(-FADE_IN_TIME as _);
        }
        Ok(())
    }

    fn touch(&mut self, _tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if self.back_btn.touch(touch) {
            self.next_scene = Some(NextScene::PopWithResult(Box::new(())));
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.chart.settle();
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            ..Default::default()
        });
        ui.fill_rect(ui.screen_rect(), (*self.chart.illustration.1, ui.screen_rect()));
        ui.fill_rect(ui.screen_rect(), semi_black(0.55));

        let c = semi_white((tm.now() as f32 / FADE_IN_TIME).clamp(-1., 0.) + 1.);

        let r = ui.back_rect();
        self.back_btn.set(ui, r);
        ui.fill_rect(r, (*self.icon_back, r, ScaleType::Fit, c));

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
