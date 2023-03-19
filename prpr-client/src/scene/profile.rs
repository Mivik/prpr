use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{screen_aspect, SafeTexture},
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{button_hit, RectButton, Ui},
};

use crate::page::SFader;

pub struct ProfileScene {
    id: u64,

    icon_back: SafeTexture,
    btn_back: RectButton,

    sf: SFader,
}

impl ProfileScene {
    pub fn new(id: u64, icon_back: SafeTexture) -> Self {
        Self {
            id,

            icon_back,
            btn_back: RectButton::new(),

            sf: SFader::new(),
        }
    }
}

impl Scene for ProfileScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        self.sf.enter(tm.now() as _);
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        if self.btn_back.touch(touch) {
            button_hit();
            self.sf.next(t, NextScene::Pop);
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            ..Default::default()
        });
        let t = tm.now() as f32;
        ui.fill_rect(ui.screen_rect(), GRAY);
        let r = ui.back_rect();
        ui.fill_rect(r, (*self.icon_back, r));
        self.btn_back.set(ui, r);

        self.sf.render(ui, t);
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        self.sf.next_scene(tm.now() as f32).unwrap_or_default()
    }
}
