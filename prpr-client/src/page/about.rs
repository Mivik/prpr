prpr::tl_file!("about");

use super::{Page, SharedState, SIDE_PADDING};
use anyhow::Result;
use macroquad::prelude::Touch;
use prpr::ui::{Scroll, Ui};
use std::borrow::Cow;

pub struct AboutPage {
    scroll: Scroll,
}

impl AboutPage {
    pub fn new() -> Self {
        Self { scroll: Scroll::new() }
    }
}

impl Page for AboutPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn update(&mut self, _focus: bool, state: &mut SharedState) -> Result<()> {
        self.scroll.update(state.t);
        Ok(())
    }
    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool> {
        if self.scroll.touch(touch, state.t) {
            return Ok(true);
        }
        Ok(false)
    }
    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        ui.dx(0.02);
        ui.dy(0.01);
        self.scroll.size(state.content_size);
        self.scroll.render(ui, |ui| {
            let r = ui
                .text(tl!("about", "version" => env!("CARGO_PKG_VERSION")))
                .multiline()
                .max_width((1. - SIDE_PADDING) * 2. - 0.02)
                .size(0.5)
                .draw();
            (r.w, r.h + 0.02)
        });
        Ok(())
    }
}
