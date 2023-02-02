use crate::page::ChartItem;
use macroquad::prelude::*;
use prpr::{
    ext::{RectExt, SafeTexture},
    ui::{RectButton, Ui},
};

use super::main::CHARTS_BAR_HEIGHT;

pub enum ChartOrder {
    Default,
    Name,
}

impl ChartOrder {
    pub fn apply(&self, charts: &mut [ChartItem]) {
        self.apply_delegate(charts, |it| it)
    }

    pub fn apply_delegate<T>(&self, charts: &mut [T], f: impl Fn(&T) -> &ChartItem) {
        match self {
            Self::Default => {
                charts.reverse();
            }
            Self::Name => {
                charts.sort_by(|x, y| f(x).info.name.cmp(&f(y).info.name));
            }
        }
    }
}

const ORDER_NUM: usize = 4;
const ORDER_LABELS: [&str; ORDER_NUM] = ["从新到旧", "从旧到新", "名字正序", "名字倒序"];
static ORDERS: [(ChartOrder, bool); ORDER_NUM] = [
    (ChartOrder::Default, false),
    (ChartOrder::Default, true),
    (ChartOrder::Name, false),
    (ChartOrder::Name, true),
];

pub struct ChartOrderBox {
    icon_play: SafeTexture,
    button: RectButton,
    index: usize,
}

impl ChartOrderBox {
    pub fn new(icon_play: SafeTexture) -> Self {
        Self {
            icon_play,
            button: RectButton::new(),
            index: 0,
        }
    }

    pub fn touch(&mut self, touch: &Touch) -> bool {
        if self.button.touch(touch) {
            self.index += 1;
            if self.index == ORDER_NUM {
                self.index = 0;
            }
            return true;
        }
        false
    }

    pub fn render(&mut self, ui: &mut Ui) -> Rect {
        ui.scope(|ui| {
            let h = CHARTS_BAR_HEIGHT;
            let r = Rect::new(0., 0., 0.22, h);
            self.button.set(ui, r);
            ui.fill_rect(r, Color::new(1., 1., 1., if self.button.touching() { 0.1 } else { 0.4 }));
            let icon = Rect::new(0.02, h / 2., 0., 0.).feather(0.02);
            ui.fill_rect(icon, (*self.icon_play, icon));
            ui.dx(icon.w);
            ui.text(ORDER_LABELS[self.index])
                .pos(0., h / 2.)
                .anchor(0., 0.5)
                .no_baseline()
                .size(0.5)
                .draw();
            r
        })
    }

    pub fn to_order(&self) -> &'static (ChartOrder, bool) {
        &ORDERS[self.index]
    }
}
