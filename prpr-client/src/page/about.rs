use super::{Page, SIDE_PADDING, SharedState};
use anyhow::Result;
use macroquad::prelude::Touch;
use prpr::ui::Ui;

pub struct AboutPage {
    text: String,
}

impl AboutPage {
    pub fn new() -> Self {
        Self {
            text: format!(
                r"prpr-client v{}
prpr 是一款 Phigros 模拟器，旨在为自制谱游玩提供一个统一化的平台。请自觉遵守社群相关要求，不恶意使用 prpr，不随意制作或传播低质量作品。

本软件使用的默认材质皮肤（包括音符材质和打击特效）来自于 @MisaLiu 的 phi-chart-render（https://github.com/MisaLiu/phi-chart-render），在 CC BY-NC 4.0 协议（https://creativecommons.org/licenses/by-nc/4.0/）下署名。在本软件的开发过程中，这些材质被调整尺寸并压缩以便使用。

prpr 是开源软件，遵循 GNU General Public License v3.0 协议。
测试群：660488396
GitHub: https://github.com/Mivik/prpr",
                env!("CARGO_PKG_VERSION")
            ),
        }
    }
}

impl Page for AboutPage {
    fn label(&self) -> &'static str {
        "关于"
    }

    fn update(&mut self, _focus: bool, _state: &mut SharedState) -> Result<()> {
        Ok(())
    }
    fn touch(&mut self, _touch: &Touch, _state: &mut SharedState) -> Result<bool> {
        Ok(false)
    }
    fn render(&mut self, ui: &mut Ui, _state: &mut SharedState) -> Result<()> {
        ui.dx(0.02);
        ui.dy(0.01);
        ui.text(&self.text)
            .multiline()
            .max_width((1. - SIDE_PADDING) * 2. - 0.02)
            .size(0.5)
            .draw();
        Ok(())
    }
}
