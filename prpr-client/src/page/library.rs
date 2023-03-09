prpr::tl_file!("library");

use super::{load_local, ChartItem, Fader, Page, SharedState};
use crate::{
    data::BriefChartInfo,
    phizone::{Client, PZChart, PZFile, PZSong},
    scene::{ChartOrder, SongScene},
};
use anyhow::Result;
use futures_util::future::join_all;
use macroquad::prelude::*;
use prpr::{
    core::Tweenable,
    ext::{semi_black, RectExt, SafeTexture, ScaleType, BLACK_TEXTURE},
    scene::{show_error, show_message, NextScene},
    task::Task,
    ui::{DRectButton, Scroll, Ui, button_hit_large},
};
use std::{
    any::Any,
    borrow::Cow,
    ops::{Deref, Range},
    sync::Arc,
};

const CHART_HEIGHT: f32 = 0.3;
const CHART_PADDING: f32 = 0.013;
const ROW_NUM: u32 = 4;
const PAGE_NUM: u64 = 28;
const TRANSIT_TIME: f32 = 0.4;
const BACK_FADE_IN_TIME: f32 = 0.2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChartListType {
    Local,
    Online,
    Popular,
}

type OnlineTaskResult = (Vec<(ChartItem, PZFile)>, Vec<Arc<PZChart>>, u64);
type OnlineTask = Task<Result<OnlineTaskResult>>;

struct TransitState {
    id: u32,
    rect: Option<Rect>,
    chart: ChartItem,
    start_time: f32,
    next_scene: Option<NextScene>,
    back: bool,
    done: bool,
}

pub struct LibraryPage {
    btn_local: DRectButton,
    btn_online: DRectButton,
    btn_popular: DRectButton,
    chosen: ChartListType,

    transit: Option<TransitState>,
    back_fade_in: Option<(u32, f32)>,

    scroll: Scroll,
    chart_btns: Vec<DRectButton>,
    charts_fader: Fader,
    current_page: u64,
    total_page: u64,
    prev_page_btn: DRectButton,
    next_page_btn: DRectButton,

    online_task: Option<OnlineTask>,
    online_charts: Option<Vec<ChartItem>>,

    icon_back: SafeTexture,
}

impl LibraryPage {
    pub fn new(icon_back: SafeTexture) -> Result<Self> {
        Ok(Self {
            btn_local: DRectButton::new(),
            btn_online: DRectButton::new(),
            btn_popular: DRectButton::new(),
            chosen: ChartListType::Local,

            transit: None,
            back_fade_in: None,

            scroll: Scroll::new(),
            chart_btns: Vec::new(),
            charts_fader: Fader::new().with_distance(0.12),
            current_page: 0,
            total_page: 0,
            prev_page_btn: DRectButton::new(),
            next_page_btn: DRectButton::new(),

            online_task: None,
            online_charts: None,

            icon_back,
        })
    }
}

impl LibraryPage {
    fn charts_display_range(&mut self, content_size: (f32, f32)) -> Range<u32> {
        let sy = self.scroll.y_scroller.offset();
        let start_line = (sy / CHART_HEIGHT) as u32;
        let end_line = ((sy + content_size.1) / CHART_HEIGHT).ceil() as u32;
        let res = (start_line * ROW_NUM)..((end_line + 1) * ROW_NUM);
        if let Some(need) = (res.end as usize).checked_sub(self.chart_btns.len()) {
            self.chart_btns.extend(std::iter::repeat_with(|| DRectButton::new().no_sound()).take(need));
        }
        res
    }

    pub fn render_charts(&mut self, ui: &mut Ui, c: Color, t: f32, local: &Vec<ChartItem>, r: Rect) {
        let content_size = (r.w, r.h);
        let range = self.charts_display_range(content_size);
        self.scroll.size(content_size);
        let charts = match self.chosen {
            ChartListType::Local => Some(local),
            ChartListType::Online => self.online_charts.as_ref(),
            _ => unreachable!(),
        };
        let Some(charts) = charts else {
            let ct = r.center();
            ui.loading(ct.x, ct.y, t, c, ());
            return;
        };
        if charts.is_empty() {
            let ct = r.center();
            ui.text(tl!("list-empty")).pos(ct.x, ct.y).anchor(0.5, 0.5).no_baseline().color(c).draw();
            return;
        }
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            self.scroll.render(ui, |ui| {
                let cw = r.w / ROW_NUM as f32;
                let ch = CHART_HEIGHT;
                let p = CHART_PADDING;
                let r = Rect::new(p, p, cw - p * 2., ch - p * 2.);
                self.charts_fader.reset();
                self.charts_fader.for_sub(|f| {
                    ui.hgrids(content_size.0, ch, ROW_NUM, charts.len() as u32, |ui, id| {
                        if let Some(transit) = &mut self.transit {
                            if transit.id == id {
                                transit.rect = Some(ui.rect_to_global(r));
                            }
                        }
                        if !range.contains(&id) {
                            if let Some(btn) = self.chart_btns.get_mut(id as usize) {
                                btn.invalidate();
                            }
                            return;
                        }
                        f.render(ui, t, |ui, nc| {
                            let mut c = Color { a: nc.a * c.a, ..nc };
                            let chart = &charts[id as usize];
                            let (r, path) = self.chart_btns[id as usize]
                                .render_shadow(ui, r, t, c.a, |r| (*chart.illustration.0, r.feather(0.01), ScaleType::CropCenter, c));
                            if let Some((that_id, start_time)) = &self.back_fade_in {
                                if id == *that_id {
                                    let p = ((t - start_time) / BACK_FADE_IN_TIME).max(0.);
                                    if p > 1. {
                                        self.back_fade_in = None;
                                    } else {
                                        ui.fill_path(&path, semi_black(0.55 * (1. - p)));
                                        c.a *= p;
                                    }
                                }
                            }
                            ui.fill_path(&path, (semi_black(0.4 * c.a), (0., 0.), semi_black(0.8 * c.a), (0., ch)));
                            ui.text(&chart.info.name)
                                .pos(r.x + 0.01, r.bottom() - 0.02)
                                .max_width(r.w)
                                .anchor(0., 1.)
                                .size(0.6 * r.w / cw)
                                .color(c)
                                .draw();
                        });
                    })
                })
            });
        });
    }

    pub fn load_online(&mut self) {
        if self.online_task.is_some() {
            return;
        }
        self.scroll.y_scroller.set_offset(0.);
        self.online_charts = None;
        let page = self.current_page;
        self.online_task = Some(Task::new(async move {
            let (charts, count) = Client::query::<PZChart>()
                .flag("query_song")
                .order("-time")
                .page(page)
                .page_num(PAGE_NUM)
                .send()
                .await?;
            let total_page = (count - 1) / PAGE_NUM + 1;
            let pz_charts = charts.iter().map(|it| Arc::new(it.clone())).collect();
            let charts: Vec<_> = join_all(charts.into_iter().map(|it| {
                let tex = BLACK_TEXTURE.clone();
                async move {
                    // let illu = it.illustration.clone();
                    let song: PZSong = it.song.load().await?.deref().clone();
                    Result::<_>::Ok((
                        ChartItem {
                            info: BriefChartInfo {
                                id: Some(it.id),
                                uploader: Some(it.owner),
                                name: song.name,
                                level: format!("{} Lv.{}", it.level, it.difficulty as u16),
                                difficulty: it.difficulty,
                                preview_start: song.preview_start.seconds as f32,
                                preview_end: song.preview_end.seconds as f32,
                                intro: it.description.unwrap_or_default(),
                                tags: Vec::new(), // TODO
                                composer: song.composer,
                                illustrator: song.illustrator,
                            },
                            path: it.chart.map(|it| it.url).unwrap_or_default(),
                            illustration: (tex.clone(), tex),
                            illustration_task: Some(Task::new({
                                let illu = song.illustration.clone();
                                async move { Ok((illu.load_thumbnail().await?, None)) }
                            })),
                            loaded_illustration: Arc::default(),
                        },
                        song.illustration,
                    ))
                }
            }))
            .await
            .into_iter()
            .collect::<Result<_>>()?;
            Ok((charts, pz_charts, total_page))
        }));
    }

    #[inline]
    fn switch_to_type(&mut self, ty: ChartListType) {
        if self.chosen != ty {
            self.chosen = ty;
            self.chart_btns.clear();
        }
    }
}

impl Page for LibraryPage {
    fn label(&self) -> Cow<'static, str> {
        "LIBRARY".into()
    }

    fn on_result(&mut self, res: Box<dyn Any>, s: &mut SharedState) -> Result<()> {
        let _res = match res.downcast::<()>() {
            Err(res) => res,
            Ok(_) => {
                let transit = self.transit.as_mut().unwrap();
                transit.start_time = s.t;
                transit.back = true;
                transit.done = false;
                return Ok(());
            }
        };
        Ok(())
    }

    fn enter(&mut self, s: &mut SharedState) -> Result<()> {
        s.charts_local = load_local(&*BLACK_TEXTURE, &(ChartOrder::Default, false));
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        if self.transit.is_some() {
            return Ok(false);
        }
        let t = s.t;
        if self.btn_local.touch(touch, t) {
            self.switch_to_type(ChartListType::Local);
            return Ok(true);
        }
        if self.btn_online.touch(touch, t) {
            self.switch_to_type(ChartListType::Online);
            if self.online_charts.is_none() {
                self.load_online();
            }
            return Ok(true);
        }
        if self.btn_popular.touch(touch, t) {
            // self.chosen = ChartListType::Popular;
            show_message(tl!("not-opened")).warn();
            return Ok(true);
        }
        if self.online_task.is_none() {
            if self.prev_page_btn.touch(touch, t) {
                if self.current_page != 0 {
                    self.current_page -= 1;
                    self.chart_btns.clear();
                    self.load_online();
                }
                return Ok(true);
            }
            if self.next_page_btn.touch(touch, t) {
                if self.current_page + 1 < self.total_page {
                    self.current_page += 1;
                    self.chart_btns.clear();
                    self.load_online();
                }
                return Ok(true);
            }
        }
        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        if self.scroll.contains(touch) {
            let charts = match self.chosen {
                ChartListType::Local => Some(&s.charts_local),
                ChartListType::Online => self.online_charts.as_ref(),
                _ => unreachable!(),
            };
            for (id, (btn, chart)) in self.chart_btns.iter_mut().zip(charts.into_iter().flatten()).enumerate() {
                if btn.touch(touch, t) {
                    button_hit_large();
                    let scene = SongScene::new(chart.clone(), self.icon_back.clone());
                    self.transit = Some(TransitState {
                        id: id as _,
                        rect: None,
                        chart: chart.clone(),
                        start_time: t,
                        next_scene: Some(NextScene::Overlay(Box::new(scene))),
                        back: false,
                        done: false,
                    });
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        if let Some(task) = &mut self.online_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => show_error(err.context(tl!("failed-to-load-online"))),
                    Ok(res) => {
                        self.total_page = res.2;
                        self.online_charts = Some(res.0.into_iter().map(|it| it.0).collect());
                        self.charts_fader.sub(t);
                    }
                }
                self.online_task = None;
            }
        }
        self.scroll.update(t);
        for chart in &mut s.charts_local {
            chart.settle();
        }
        if let Some(charts) = &mut self.online_charts {
            for chart in charts {
                chart.settle();
            }
        }
        if let Some(transit) = &mut self.transit {
            if t > transit.start_time + TRANSIT_TIME {
                if transit.back {
                    self.back_fade_in = Some((transit.id, t));
                    self.transit = None;
                } else {
                    transit.done = true;
                }
            }
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
                    (&mut self.btn_local, tl!("local"), ChartListType::Local),
                    (&mut self.btn_online, tl!("online"), ChartListType::Online),
                    (&mut self.btn_popular, tl!("popular"), ChartListType::Popular),
                ]
                .into_iter()
                .map(|(btn, text, ty)| (btn, text, ty == self.chosen)),
            );
        });
        let mut r = ui.content_rect();
        r.h -= 0.08;
        s.fader.render(ui, t, |ui, c| {
            let path = r.rounded(0.02);
            ui.fill_path(&path, semi_black(0.4 * c.a));
            self.render_charts(ui, c, s.t, &s.charts_local, r.feather(-0.01))
        });
        s.render_fader(ui, |ui, c| {
            let cx = r.center().x;
            let r = ui
                .text(tl!("page", "current" => self.current_page + 1, "total" => self.total_page))
                .pos(cx, r.bottom() + 0.034)
                .anchor(0.5, 0.)
                .no_baseline()
                .size(0.5)
                .color(c)
                .draw();
            let dist = 0.3;
            let ft = 0.024;
            let prev_page = tl!("prev-page");
            let r = ui.text(prev_page.deref()).pos(cx - dist, r.y).anchor(0.5, 0.).size(0.5).measure();
            self.prev_page_btn.render_text(ui, r.feather(ft), t, c.a, prev_page, 0.5, false);
            let next_page = tl!("next-page");
            let r = ui.text(next_page.deref()).pos(cx + dist, r.y).anchor(0.5, 0.).size(0.5).measure();
            self.next_page_btn.render_text(ui, r.feather(ft), t, c.a, next_page, 0.5, false);
        });
        if let Some(transit) = &self.transit {
            if let Some(fr) = transit.rect {
                let p = ((t - transit.start_time) / TRANSIT_TIME).clamp(0., 1.);
                let p = (1. - p).powi(4);
                let p = if transit.back { p } else { 1. - p };
                let r = Rect::new(
                    f32::tween(&fr.x, &-1., p),
                    f32::tween(&fr.y, &-ui.top, p),
                    f32::tween(&fr.w, &2., p),
                    f32::tween(&fr.h, &(ui.top * 2.), p),
                );
                let path = r.rounded(0.02 * (1. - p));
                ui.fill_path(&path, (*transit.chart.illustration.1, r.feather(0.01 * (1. - p))));
                ui.fill_path(&path, semi_black(0.55));
            }
        }
        Ok(())
    }

    fn next_scene(&mut self) -> NextScene {
        if let Some(transit) = &mut self.transit {
            if transit.done {
                return transit.next_scene.take().unwrap_or_default();
            }
        }
        NextScene::None
    }
}
