use anyhow::Result;
use prpr::{scene::Scene, time::TimeManager, ui::Ui, ext::screen_aspect};
use macroquad::prelude::*;

use crate::page::SFader;

pub struct ProfileScene {
	id: u64,
	sf: SFader,
}

impl ProfileScene {
	pub fn new(id: u64) -> Self {
		Self {
			id,
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

	fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
		set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            ..Default::default()
        });
        let t = tm.now() as f32;
        ui.fill_rect(ui.screen_rect(), WHITE);
        self.sf.render(ui, t);

		Ok(())
	}
}
