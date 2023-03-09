prpr::tl_file!("settings");

use crate::{
    get_data, get_data_mut,
    popup::{ChooseButton, Popup},
    save_data, sync_data,
};

use super::{Page, SharedState};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{semi_black, RectExt, SafeTexture, ScaleType},
    l10n::{LanguageIdentifier, LANG_IDENTS, LANG_NAMES},
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

    list_general: GeneralList,
    list_chart: ChartList,

    scroll: Scroll,
}

impl SettingsPage {
    pub fn new(icon_lang: SafeTexture) -> Self {
        Self {
            btn_general: DRectButton::new(),
            btn_chart: DRectButton::new(),
            chosen: SettingListType::General,

            list_general: GeneralList::new(icon_lang),
            list_chart: ChartList::new(),

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
}

impl Page for SettingsPage {
    fn label(&self) -> Cow<'static, str> {
        "SETTINGS".into()
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if match self.chosen {
            SettingListType::General => self.list_general.top_touch(touch, t),
            SettingListType::Chart => self.list_chart.top_touch(touch, t),
        } {
            return Ok(true);
        }

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
        if match self.chosen {
            SettingListType::General => self.list_general.touch(touch, t)?,
            SettingListType::Chart => self.list_chart.touch(touch, t)?,
        } {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        self.scroll.update(t);
        match self.chosen {
            SettingListType::General => self.list_general.update(t)?,
            SettingListType::Chart => self.list_chart.update(t)?,
        }
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
                self.scroll.render(ui, |ui| match self.chosen {
                    SettingListType::General => self.list_general.render(ui, r, t, c),
                    SettingListType::Chart => self.list_chart.render(ui, r, t, c),
                });
            });
        });
        Ok(())
    }
}

fn render_title<'a>(ui: &mut Ui, c: Color, title: impl Into<Cow<'a, str>>, subtitle: Option<Cow<'a, str>>) -> f32 {
    const TITLE_SIZE: f32 = 0.6;
    const SUBTITLE_SIZE: f32 = 0.35;
    const LEFT: f32 = 0.06;
    const PAD: f32 = 0.01;
    const SUB_MAX_WIDTH: f32 = 1.;
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
        let r1 = ui
            .text(subtitle)
            .pos(LEFT, (ITEM_HEIGHT + h) / 2.)
            .anchor(0., 1.)
            .size(SUBTITLE_SIZE)
            .max_width(SUB_MAX_WIDTH)
            .color(Color { a: c.a * 0.6, ..c })
            .draw()
            .right();
        let r2 = ui
            .text(title)
            .pos(LEFT, (ITEM_HEIGHT - h) / 2.)
            .no_baseline()
            .size(TITLE_SIZE)
            .color(c)
            .draw()
            .right();
        r1.max(r2)
    } else {
        ui.text(title.into())
            .pos(LEFT, ITEM_HEIGHT / 2.)
            .anchor(0., 0.5)
            .no_baseline()
            .size(TITLE_SIZE)
            .color(c)
            .draw()
            .right()
    }
}

#[inline]
fn render_switch<'a>(ui: &mut Ui, r: Rect, t: f32, c: Color, btn: &mut DRectButton, on: bool) {
    btn.render_text(ui, r, t, c.a, if on { tl!("switch-on") } else { tl!("switch-off") }, 0.5, on);
}

#[inline]
fn right_rect(w: f32) -> Rect {
    let rh = ITEM_HEIGHT * 2. / 3.;
    Rect::new(w - 0.3, (ITEM_HEIGHT - rh) / 2., 0.26, rh)
}

struct GeneralList {
    icon_lang: SafeTexture,

    lang_btn: ChooseButton,
    lowq_btn: DRectButton,
}

impl GeneralList {
    pub fn new(icon_lang: SafeTexture) -> Self {
        Self {
            icon_lang,

            lang_btn: ChooseButton::new()
                .with_options(LANG_NAMES.iter().map(|s| s.to_string()).collect())
                .with_selected(
                    get_data()
                        .language
                        .as_ref()
                        .and_then(|it| it.parse::<LanguageIdentifier>().ok())
                        .and_then(|ident| LANG_IDENTS.iter().position(|it| *it == ident))
                        .unwrap_or_default(),
                ),
            lowq_btn: DRectButton::new(),
        }
    }

    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.lang_btn.top_touch(touch, t) {
            return true;
        }
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<bool> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.lang_btn.touch(touch, t) {
            return Ok(true);
        }
        if self.lowq_btn.touch(touch, t) {
            config.sample_count = if config.sample_count == 1 { 2 } else { 1 };
            save_data()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn update(&mut self, t: f32) -> Result<()> {
        self.lang_btn.update(t);
        let data = get_data_mut();
        if self.lang_btn.changed() {
            data.language = Some(LANG_IDENTS[self.lang_btn.selected()].to_string());
            save_data()?;
            sync_data();
        }
        Ok(())
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32, c: Color) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT);
                h += ITEM_HEIGHT;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            let rt = render_title(ui, c, tl!("item-lang"), None);
            let w = 0.06;
            let r = Rect::new(rt + 0.01, (ITEM_HEIGHT - w) / 2., w, w);
            ui.fill_rect(r, (*self.icon_lang, r, ScaleType::Fit, c));
            self.lang_btn.render(ui, rr, t, c.a);
        }
        item! {
            render_title(ui, c, tl!("item-lowq"), Some(tl!("item-lowq-sub")));
            render_switch(ui, rr, t, c, &mut self.lowq_btn, config.sample_count == 1);
        }
        self.lang_btn.render_top(ui, t, c.a);
        (w, h)
    }
}

struct ChartList {
    autoplay_btn: DRectButton,
    dhint_btn: DRectButton,
    opt_btn: DRectButton,
}

impl ChartList {
    pub fn new() -> Self {
        Self {
            autoplay_btn: DRectButton::new(),
            dhint_btn: DRectButton::new(),
            opt_btn: DRectButton::new(),
        }
    }

    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<bool> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.autoplay_btn.touch(touch, t) {
            config.autoplay ^= true;
            save_data()?;
            return Ok(true);
        }
        if self.dhint_btn.touch(touch, t) {
            config.double_hint ^= true;
            save_data()?;
            return Ok(true);
        }
        if self.opt_btn.touch(touch, t) {
            config.aggressive ^= true;
            save_data()?;
            return Ok(true);
        }
        Ok(false)
    }

    pub fn update(&mut self, t: f32) -> Result<()> {
        Ok(())
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32, c: Color) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT);
                h += ITEM_HEIGHT;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            render_title(ui, c, tl!("item-autoplay"), Some(tl!("item-autoplay-sub")));
            render_switch(ui, rr, t, c, &mut self.autoplay_btn, config.autoplay);
        }
        item! {
            render_title(ui, c, tl!("item-dhint"), Some(tl!("item-dhint-sub")));
            render_switch(ui, rr, t, c, &mut self.dhint_btn, config.double_hint);
        }
        item! {
            render_title(ui, c, tl!("item-opt"), Some(tl!("item-opt-sub")));
            render_switch(ui, rr, t, c, &mut self.opt_btn, config.aggressive);
        }
        (w, h)
    }
}
