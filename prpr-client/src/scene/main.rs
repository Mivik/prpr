use super::{song::TrashBin, SongScene};
use crate::{
    cloud::UserManager,
    dir, get_data, get_data_mut,
    page::{self, ChartItem, Page, SharedState},
    save_data,
};
use anyhow::{anyhow, Result};
use lyon::{
    math as lm,
    path::{builder::BorderRadii, Path, Winding},
};
use macroquad::{prelude::*, texture::RenderTarget};
use prpr::{
    core::Tweenable,
    ext::{screen_aspect, SafeTexture, ScaleType},
    scene::{show_error, show_message, NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Scroll, Ui},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

const PAGE_NUM: usize = 6;
const SIDE_PADDING: f32 = 0.02;
const CARD_PADDING: f32 = 0.02;
pub const CHARTS_BAR_HEIGHT: f32 = 0.06;

const SWITCH_TIME: f32 = 0.4;
const TRANSIT_TIME: f32 = 0.4;

pub static SHOULD_DELETE: AtomicBool = AtomicBool::new(false);
pub static UPDATE_TEXTURE: Mutex<Option<SafeTexture>> = Mutex::new(None);
pub static UPDATE_INFO: AtomicBool = AtomicBool::new(false);

pub struct MainScene {
    target: Option<RenderTarget>,
    next_scene: Option<NextScene>,
    icon_back: SafeTexture,
    icon_download: SafeTexture,
    icon_play: SafeTexture,
    icon_edit: SafeTexture,
    icon_delete: SafeTexture,
    icon_question: SafeTexture,

    page_scroll: Scroll,
    page_index: usize,
    page_buttons: [RectButton; PAGE_NUM],
    switch_start_time: f32,
    page_from_index: usize,

    shared_state: SharedState,
    pages: [Box<dyn Page>; PAGE_NUM],
}

impl MainScene {
    pub async fn new() -> Result<Self> {
        if let Some(user) = &get_data().me {
            UserManager::request(&user.id);
        }
        let shared_state = SharedState::new().await?;
        macro_rules! load_tex {
            ($path:literal) => {
                SafeTexture::from(Texture2D::from_image(&load_image($path).await?))
            };
        }
        let icon_play = load_tex!("resume.png");
        Ok(Self {
            target: None,
            next_scene: None,
            icon_back: load_tex!("back.png"),
            icon_download: load_tex!("download.png"),
            icon_play: icon_play.clone(),
            icon_edit: load_tex!("edit.png"),
            icon_delete: load_tex!("delete.png"),
            icon_question: load_tex!("question.png"),

            page_scroll: Scroll::new(),
            page_index: 0,
            page_buttons: [RectButton::new(); PAGE_NUM],
            switch_start_time: f32::NEG_INFINITY,
            page_from_index: 0,

            shared_state,
            pages: [
                Box::new(page::LocalPage::new(icon_play.clone()).await?),
                Box::new(page::RemotePage::new(icon_play)),
                Box::new(page::AccountPage::new()),
                Box::new(page::MessagePage::new()),
                Box::new(page::SettingsPage::new().await?),
                Box::new(page::AboutPage::new()),
            ],
        })
    }

    fn ui(&mut self, ui: &mut Ui, t: f32, rt: f32) {
        let px = SIDE_PADDING;
        ui.scope(|ui| {
            ui.dx(-1. + px);
            ui.dy(-ui.top + 0.03);
            let mut dx = 0.;
            let mut max_height: f32 = 0.;
            let mut from_range = (0., 0.);
            let mut current_range = (0., 0.);
            for (id, page) in self.pages.iter().enumerate() {
                let r = ui.text(page.label()).pos(dx, 0.).size(0.9).draw();
                self.page_buttons[id].set(ui, Rect::new(r.x, r.y, r.w, r.h + 0.01));
                max_height = max_height.max(r.h);
                let range = (dx, dx + r.w);
                if self.page_from_index == id {
                    from_range = range;
                }
                if self.page_index == id {
                    current_range = range;
                }
                if page.has_new() {
                    ui.fill_circle(range.1 - 0.01, 0., 0.01, RED);
                }
                dx += r.w + 0.02;
            }
            let draw_range = if rt >= self.switch_start_time + SWITCH_TIME {
                current_range
            } else {
                let p = (rt - self.switch_start_time) / SWITCH_TIME;
                let p = 1. - (1. - p).powi(3);
                (f32::tween(&from_range.0, &current_range.0, p), f32::tween(&from_range.1, &current_range.1, p))
            };
            ui.fill_rect(Rect::new(draw_range.0, max_height + 0.02, draw_range.1 - draw_range.0, 0.01), WHITE);
            ui.dy(max_height + 0.04);
            let pos = ui.to_global((0., 0.)).1;
            let width = (1. - px) * 2.;
            let content_size = (width, ui.top - pos - 0.01);
            self.page_scroll.size(content_size);
            self.page_scroll.render(ui, |ui| {
                self.shared_state.t = t;
                self.shared_state.content_size = (content_size.0, content_size.1 - CHARTS_BAR_HEIGHT);
                let must_render = rt < self.switch_start_time + SWITCH_TIME;
                for (id, page) in self.pages.iter_mut().enumerate() {
                    if must_render || id == self.page_index {
                        ui.scope(|ui| page.render(ui, &mut self.shared_state)).unwrap();
                    }
                    ui.dx(content_size.0);
                }
                (content_size.0 * 3., content_size.1)
            });
        });
    }

    pub fn song_scene(&self, chart: &ChartItem, remote: bool) -> Option<NextScene> {
        Some(NextScene::Overlay(Box::new(SongScene::new(
            ChartItem {
                info: chart.info.clone(),
                path: chart.path.clone(),
                illustration: chart.illustration.clone(),
                illustration_task: None,
            },
            chart.illustration.clone(),
            self.icon_edit.clone(),
            self.icon_back.clone(),
            self.icon_download.clone(),
            self.icon_play.clone(),
            TrashBin::new(self.icon_delete.clone(), self.icon_question.clone()),
            remote,
        ))))
    }
}

impl Scene for MainScene {
    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        for page in &mut self.pages {
            page.pause()?;
        }
        Ok(())
    }

    fn resume(&mut self, _tm: &mut TimeManager) -> Result<()> {
        for page in &mut self.pages {
            page.resume()?;
        }
        Ok(())
    }

    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.switch_start_time = f32::NEG_INFINITY;
        self.target = target;
        if let Some((.., st, _, true)) = &mut self.shared_state.transit {
            *st = tm.now() as _;
        } else {
            show_message("欢迎回来");
        }
        if UPDATE_INFO.fetch_and(false, Ordering::SeqCst) {
            if let Some((false, id, ..)) = self.shared_state.transit {
                let chart = &mut self.shared_state.charts_local[id as usize];
                chart.info = get_data().charts[get_data().find_chart(chart).unwrap()].info.clone();
            }
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if tm.real_time() as f32 <= self.switch_start_time + SWITCH_TIME || self.shared_state.transit.is_some() {
            return Ok(false);
        }
        if let Some(page_id) = self.page_buttons.iter_mut().position(|it| it.touch(touch)) {
            if page_id != self.page_index {
                self.page_from_index = self.page_index;
                self.page_index = page_id;
                self.switch_start_time = tm.real_time() as f32;
            }
            return Ok(true);
        }
        self.shared_state.t = tm.now() as _;
        if self.pages[self.page_index].touch(touch, &mut self.shared_state)? {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let p = ((tm.real_time() as f32 - self.switch_start_time) / SWITCH_TIME).min(1.);
        if p < 1. {
            let p = 1. - (1. - p).powi(3);
            self.page_scroll
                .set_offset(f32::tween(&(self.page_from_index as f32), &(self.page_index as f32), p) * (1. - SIDE_PADDING) * 2., 0.);
        } else {
            self.page_scroll.set_offset(self.page_index as f32 * (1. - SIDE_PADDING) * 2., 0.);
        }
        if let Some(tex) = UPDATE_TEXTURE.lock().unwrap().take() {
            if let Some((true, id, ..)) = self.shared_state.transit {
                self.shared_state.charts_local[id as usize].illustration = tex;
            }
        }
        self.shared_state.t = tm.now() as _;
        for (id, page) in self.pages.iter_mut().enumerate() {
            page.update(id == self.page_index, &mut self.shared_state)?;
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: self.target,
            ..Default::default()
        });
        clear_background(GRAY);
        ui.scope(|ui| self.ui(ui, tm.now() as _, tm.real_time() as _));
        if let Some((remote, id, st, rect, back)) = &mut self.shared_state.transit {
            let remote = *remote;
            let id = *id as usize;
            let t = tm.now() as f32;
            let p = ((t - *st) / TRANSIT_TIME).min(1.);
            let mut p = 1. - (1. - p).powi(4);
            if *back {
                p = 1. - p;
            }
            let rect = Rect::new(
                f32::tween(&rect.x, &-1., p),
                f32::tween(&rect.y, &-ui.top, p),
                f32::tween(&rect.w, &2., p),
                f32::tween(&rect.h, &(ui.top * 2.), p),
            );
            let path = {
                let mut path = Path::builder();
                let pad = CARD_PADDING * (1. - p);
                path.add_rounded_rectangle(
                    &lm::Box2D::new(lm::point(rect.x + pad, rect.y + pad), lm::point(rect.right() - pad, rect.bottom() - pad)),
                    &BorderRadii::new(0.01 * (1. - p)),
                    Winding::Positive,
                );
                path.build()
            };
            let dst = if remote {
                &mut self.shared_state.charts_remote
            } else {
                &mut self.shared_state.charts_local
            };
            let chart = &dst[id];
            ui.fill_path(&path, (*chart.illustration, rect, ScaleType::Scale));
            ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
            if *back && p <= 0. {
                if SHOULD_DELETE.fetch_and(false, Ordering::SeqCst) {
                    let err: Result<_> = (|| {
                        let id = if remote {
                            let path = format!("download/{}", self.shared_state.charts_remote[id].info.id.as_ref().unwrap());
                            self.shared_state
                                .charts_local
                                .iter()
                                .position(|it| it.path == path)
                                .ok_or_else(|| anyhow!("找不到谱面"))?
                        } else {
                            id
                        };
                        let chart = &self.shared_state.charts_local[id];
                        let path = format!("{}/{}", dir::charts()?, chart.path);
                        let path = std::path::Path::new(&path);
                        if path.is_file() {
                            std::fs::remove_file(path)?;
                        } else {
                            std::fs::remove_dir_all(path)?;
                        }
                        get_data_mut().charts.remove(get_data().find_chart(chart).unwrap());
                        save_data()?;
                        self.shared_state.charts_local.remove(id);
                        Ok(())
                    })();
                    if let Err(err) = err {
                        show_error(err.context("删除失败"));
                    } else {
                        show_message("删除成功");
                    }
                }
                self.shared_state.transit = None;
            } else if !*back && p >= 1. {
                *back = true;
                self.next_scene = if remote {
                    let path = format!("download/{}", self.shared_state.charts_remote[id].info.id.as_ref().unwrap());
                    if let Some(index) = self.shared_state.charts_local.iter().position(|it| it.path == path) {
                        self.shared_state.charts_local[index].illustration = self.shared_state.charts_remote[id].illustration.clone();
                        self.song_scene(&self.shared_state.charts_local[index], false)
                    } else {
                        self.song_scene(&self.shared_state.charts_remote[id], true)
                    }
                } else {
                    self.song_scene(&self.shared_state.charts_local[id], false)
                };
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
