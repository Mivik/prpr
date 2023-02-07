use super::{Page, SharedState, SIDE_PADDING};
use anyhow::Result;
use macroquad::prelude::Touch;
use prpr::ui::{Scroll, Ui};
use std::borrow::Cow;

pub struct AboutPage {
    scroll: Scroll,
    text: String,
}

impl AboutPage {
    pub fn new() -> Self {
        Self {
            scroll: Scroll::new(),
            text: format!(
                r"prpr-client v{}
prpr 是一款 Phigros 模拟器，旨在为自制谱游玩提供一个统一化的平台。请自觉遵守社群相关要求，不恶意使用 prpr，不随意制作或传播低质量作品。

本软件使用的默认材质皮肤（包括音符材质和打击特效）来自于 @MisaLiu 的 phi-chart-render（https://github.com/MisaLiu/phi-chart-render），在 CC BY-NC 4.0 协议（https://creativecommons.org/licenses/by-nc/4.0/）下署名。在本软件的开发过程中，这些材质被调整尺寸并压缩以便使用。

prpr 是开源软件，遵循 GNU General Public License v3.0 协议。
测试群：660488396
GitHub：https://github.com/Mivik/prpr

欢迎在爱发电上支持 prpr 的开发：https://afdian.net/a/mivik",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }
}

impl Page for AboutPage {
    fn label(&self) -> Cow<'static, str> {
        "关于".into()
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
                .text(&self.text)
                .multiline()
                .max_width((1. - SIDE_PADDING) * 2. - 0.02)
                .size(0.5)
                .draw();
            (r.w, r.h)
        });
        Ok(())
    }
}
