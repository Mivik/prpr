prpr::tl_file!("settings");

use super::{Page, SharedState};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{semi_black, RectExt},
    ui::{DRectButton, Scroll, Ui},
};
use std::borrow::Cow;

const ITEM_HEIGHT: f32 = 0.15;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingListType {
    General,
    Chart,
}

pub struct SettingsPage {
    btn_general: DRectButton,
    btn_chart: DRectButton,
    chosen: SettingListType,

    scroll: Scroll,
}

impl SettingsPage {
    pub fn new() -> Self {
        Self {
            btn_general: DRectButton::new(),
            btn_chart: DRectButton::new(),
            chosen: SettingListType::General,

            scroll: Scroll::new(),
        }
    }

    #[inline]
    fn switch_to_type(&mut self, ty: SettingListType) {
        if self.chosen != ty {
            self.chosen = ty;
            self.scroll.y_scroller.set_offset(0.);
        }
    }

    fn title<'a>(ui: &mut Ui, c: Color, title: impl Into<Cow<'a, str>>, subtitle: Option<Cow<'a, str>>) {
        const TITLE_SIZE: f32 = 0.7;
        const SUBTITLE_SIZE: f32 = 0.45;
        const LEFT: f32 = 0.06;
        const PAD: f32 = 0.01;
        const SUB_MAX_WIDTH: f32 = 0.5;
        if let Some(subtitle) = subtitle {
            let title = title.into();
            let r1 = ui.text(Cow::clone(&title)).size(TITLE_SIZE).measure();
            let r2 = ui
                .text(Cow::clone(&subtitle))
                .size(SUBTITLE_SIZE)
                .max_width(SUB_MAX_WIDTH)
                .no_baseline()
                .measure();
            let h = r1.h + PAD + r2.h;
            ui.text(subtitle)
                .pos(LEFT, (ITEM_HEIGHT + h) / 2.)
                .anchor(0., 1.)
                .size(SUBTITLE_SIZE)
                .max_width(SUB_MAX_WIDTH)
                .color(Color { a: c.a * 0.6, ..c })
                .draw();
            ui.text(title)
                .pos(LEFT, (ITEM_HEIGHT - h) / 2.)
                .no_baseline()
                .size(TITLE_SIZE)
                .color(c)
                .draw();
        } else {
            ui.text(title.into())
                .pos(LEFT, ITEM_HEIGHT / 2.)
                .anchor(0., 0.5)
                .no_baseline()
                .size(TITLE_SIZE)
                .color(c)
                .draw();
        }
    }

    fn list_general(&mut self, ui: &mut Ui, r: Rect, c: Color) {
        self.scroll.render(ui, |ui| {
            let w = r.w;
            let mut h = 0.;
            macro_rules! item {
                ($($b:tt)*) => {{
                    $($b)*
                    ui.dy(ITEM_HEIGHT);
                    h += ITEM_HEIGHT;
                }}
            }
            item! {
                Self::title(ui, c, "语言", None);
            }
            (w, h)
        });
    }

    fn list_chart(&mut self, ui: &mut Ui, r: Rect, c: Color) {
        self.scroll.render(ui, |ui| {
            let w = r.w;
            (w, 0.)
        });
    }
}

impl Page for SettingsPage {
    fn label(&self) -> Cow<'static, str> {
        "SETTINGS".into()
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if self.btn_general.touch(touch, t) {
            self.switch_to_type(SettingListType::General);
            return Ok(true);
        }
        if self.btn_chart.touch(touch, t) {
            self.switch_to_type(SettingListType::Chart);
            return Ok(true);
        }
        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        self.scroll.update(s.t);
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        s.render_fader(ui, |ui, c| {
            ui.tab_rects(
                c,
                t,
                [
                    (&mut self.btn_general, tl!("general"), SettingListType::General),
                    (&mut self.btn_chart, tl!("chart"), SettingListType::Chart),
                ]
                .into_iter()
                .map(|(btn, text, ty)| (btn, text, ty == self.chosen)),
            );
        });
        let r = ui.content_rect();
        s.fader.render(ui, t, |ui, c| {
            let path = r.rounded(0.02);
            ui.fill_path(&path, semi_black(0.4 * c.a));
            let r = r.feather(-0.01);
            self.scroll.size((r.w, r.h));
            ui.scope(|ui| {
                ui.dx(r.x);
                ui.dy(r.y);
                match self.chosen {
                    SettingListType::General => self.list_general(ui, r, c),
                    SettingListType::Chart => self.list_chart(ui, r, c),
                }
            });
        });
        Ok(())
    }
}
