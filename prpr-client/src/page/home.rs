prpr::tl_file!("home");

use super::{LibraryPage, NextPage, Page, SharedState};
use crate::{get_data, phizone::UserManager};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{semi_black, semi_white, RectExt, SafeTexture, ScaleType},
    scene::show_message,
    ui::{DRectButton, Ui},
};

pub struct HomePage {
    character: SafeTexture,
    song: SafeTexture,
    icon_play: SafeTexture,
    icon_medal: SafeTexture,
    icon_respack: SafeTexture,
    icon_msg: SafeTexture,
    icon_settings: SafeTexture,
    icon_back: SafeTexture,

    btn_play: DRectButton,
    btn_event: DRectButton,
    btn_respack: DRectButton,
    btn_msg: DRectButton,
    btn_settings: DRectButton,

    next_page: Option<NextPage>,
}

impl HomePage {
    pub async fn new(icon_back: SafeTexture) -> Result<Self> {
        let character = SafeTexture::from(load_texture("char.png").await?).with_mipmap();
        let song = SafeTexture::from(load_texture("player.jpg").await?).with_mipmap();
        if let Some(u) = &get_data().me {
            UserManager::request(u.id);
        }
        Ok(Self {
            character,
            song,
            icon_play: load_texture("resume.png").await?.into(),
            icon_medal: load_texture("medal.png").await?.into(),
            icon_respack: load_texture("respack.png").await?.into(),
            icon_msg: load_texture("message.png").await?.into(),
            icon_settings: load_texture("settings.png").await?.into(),
            icon_back,

            btn_play: DRectButton::new().with_delta(-0.01),
            btn_event: DRectButton::new().with_elevation(0.002),
            btn_respack: DRectButton::new().with_elevation(0.002),
            btn_msg: DRectButton::new().with_radius(0.03).with_delta(-0.003).with_elevation(0.002),
            btn_settings: DRectButton::new().with_radius(0.03).with_delta(-0.003).with_elevation(0.002),

            next_page: None,
        })
    }
}

impl Page for HomePage {
    fn label(&self) -> std::borrow::Cow<'static, str> {
        "PHIRA".into()
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if self.btn_play.touch(touch, t) {
            self.next_page = Some(NextPage::Overlay(Box::new(LibraryPage::new(self.icon_back.clone())?)));
            return Ok(true);
        }
        if self.btn_event.touch(touch, t) {
            show_message(tl!("not-opened")).warn();
            return Ok(true);
        }
        if self.btn_respack.touch(touch, t) {
            return Ok(true);
        }
        if self.btn_msg.touch(touch, t) {
            return Ok(true);
        }
        if self.btn_settings.touch(touch, t) {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let pad = 0.04;

        s.render_fader(ui, |ui, c| {
            let r = Rect::new(-0.8, -ui.top + 0.1, 0.7, 1.3);
            ui.fill_rect(r, (*self.character, r, ScaleType::CropCenter, c));
        });

        // play button
        let top = s.render_fader(ui, |ui, c| {
            let r = Rect::new(0., -0.25, 0.8, 0.43);
            let top = r.bottom() + 0.02;
            let (r, path) = self
                .btn_play
                .render_shadow(ui, r, t, c.a, |_| (*self.song, r.feather(0.05), ScaleType::CropCenter, c));
            ui.fill_path(&path, (semi_black(0.7 * c.a), (r.x, r.y), Color::default(), (r.x + 0.6, r.y)));
            ui.text(tl!("play")).pos(r.x + pad, r.y + pad).color(c).draw();
            let r = Rect::new(r.x + 0.02, r.bottom() - 0.18, 0.17, 0.17);
            ui.fill_rect(r, (*self.icon_play, r, ScaleType::Fit, semi_white(0.6 * c.a)));
            top
        });

        let text_and_icon = |ui: &mut Ui, r: Rect, btn: &mut DRectButton, text, icon, c: Color| {
            let ow = r.w;
            let (r, _) = btn.render_shadow(ui, r, t, c.a, |_| semi_black(0.4 * c.a));
            let ir = Rect::new(r.x + 0.02, r.bottom() - 0.08, 0.14, 0.14);
            ui.text(text).pos(r.x + 0.026, r.y + 0.026).size(0.7 * r.w / ow).color(c).draw();
            ui.fill_rect(
                {
                    let mut ir = ir;
                    ir.h = ir.h.min(r.bottom() - ir.y);
                    ir
                },
                (icon, ir, ScaleType::Fit, semi_white(0.4 * c.a)),
            );
        };

        let r = s.render_fader(ui, |ui, c| {
            let r = Rect::new(0., top, 0.38, 0.23);
            text_and_icon(ui, r, &mut self.btn_event, tl!("event"), *self.icon_medal, c);
            r
        });

        let r = s.render_fader(ui, |ui, c| {
            let r = Rect::new(r.right() + 0.02, top, 0.27, 0.23);
            text_and_icon(ui, r, &mut self.btn_respack, tl!("respack"), *self.icon_respack, c);
            r
        });

        let lf = r.right() + 0.02;

        s.render_fader(ui, |ui, c| {
            let r = Rect::new(lf, top, 0.11, 0.11);
            let (r, _) = self.btn_msg.render_shadow(ui, r, t, c.a, |_| semi_black(0.4 * c.a));
            let r = r.feather(-0.01);
            ui.fill_rect(r, (*self.icon_msg, r, ScaleType::Fit, c));

            let r = Rect::new(lf, top + 0.12, 0.11, 0.11);
            let (r, _) = self.btn_settings.render_shadow(ui, r, t, c.a, |_| semi_black(0.4 * c.a));
            let r = r.feather(0.004);
            ui.fill_rect(r, (*self.icon_settings, r, ScaleType::Fit, c));
        });

        if let Some(u) = &get_data().me {
            s.render_fader(ui, |ui, c| {
                ui.avatar(0.92, -ui.top + 0.08, 0.05, c, t, UserManager::get_avatar(u.id));
            });
        }
        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or_default()
    }
}
