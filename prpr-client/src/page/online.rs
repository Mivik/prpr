prpr::tl_file!("online");

use super::{get_touched, trigger_grid, ChartItem, Page, SharedState, CARD_HEIGHT, ROW_NUM};
use crate::{
    data::BriefChartInfo,
    phizone::{Client, PZChart, PZFile, PZSong, Ptr},
    scene::{ChartOrder, ChartOrderBox, CHARTS_BAR_HEIGHT},
};
use anyhow::Result;
use futures_util::future::join_all;
use macroquad::prelude::{warn, Rect, Touch};
use prpr::{
    ext::SafeTexture,
    scene::{show_error, show_message},
    task::Task,
    ui::{MessageHandle, Scroll, Ui},
};
use std::{borrow::Cow, ops::Deref, sync::Arc};

const PAGE_NUM: u64 = 30;

pub struct OnlinePage {
    focus: bool,

    scroll: Scroll,
    choose: Option<u32>,

    order_box: ChartOrderBox,

    page: u64,
    total_page: u64,

    task_load: Task<Result<(Vec<(ChartItem, PZFile)>, Vec<Arc<PZChart>>, u64)>>,
    illu_files: Vec<PZFile>,
    first_time: bool,
    loading: Option<MessageHandle>,
}

impl OnlinePage {
    pub fn new(icon_play: SafeTexture) -> Self {
        Self {
            focus: false,

            scroll: Scroll::new(),
            choose: None,

            order_box: ChartOrderBox::new(icon_play),

            page: 0,
            total_page: 0,

            task_load: Task::pending(),
            illu_files: Vec::new(),
            first_time: true,
            loading: None,
        }
    }

    fn refresh(&mut self, state: &mut SharedState) {
        if self.loading.is_some() {
            return;
        }
        state.charts_online.clear();
        self.loading = Some(show_message(tl!("loading")).handle());
        let order = self.order_box.to_order();
        let page = self.page;
        self.task_load = Task::new({
            let tex = state.tex.clone();
            async move {
                let (charts, count) = Client::query::<PZChart>()
                    .flag("query_song")
                    .order(match order {
                        (ChartOrder::Default, false) => "-time",
                        (ChartOrder::Default, true) => "time",
                        (ChartOrder::Name, false) => todo!(),
                        (ChartOrder::Name, true) => todo!(),
                    })
                    .page(page)
                    .send()
                    .await?;
                let total_page = (count - 1) / PAGE_NUM + 1;
                let pz_charts = charts.iter().map(|it| Arc::new(it.clone())).collect();
                let charts: Vec<_> = join_all(charts.into_iter().map(|it| {
                    let tex = tex.clone();
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
                                    preview_time: song.preview_start.seconds as f32, // TODO
                                    intro: it.description.unwrap_or_default(),
                                    tags: Vec::new(), // TODO
                                    composer: song.composer,
                                    illustrator: song.illustrator,
                                },
                                path: it.chart.map(|it| it.url).unwrap_or_default(),
                                illustration: (tex.clone(), tex),
                                illustration_task: Some(Task::new({
                                    let illu = song.illustration.clone();
                                    async move {
                                        Ok((illu.load_thumbnail().await?, None))
                                    }
                                })),
                            },
                            song.illustration,
                        ))
                    }
                }))
                .await
                .into_iter()
                .collect::<Result<_>>()?;
                Ok((charts, pz_charts, total_page))
            }
        });
    }
}

impl Page for OnlinePage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn update(&mut self, focus: bool, state: &mut SharedState) -> Result<()> {
        if !self.focus && focus && self.first_time {
            self.first_time = false;
            self.refresh(state);
        }
        self.focus = focus;

        let t = state.t;
        if self.scroll.y_scroller.pulled {
            self.refresh(state);
        }
        self.scroll.update(t);
        if let Some(charts) = self.task_load.take() {
            self.loading.take().unwrap().cancel();
            match charts {
                Ok((charts, pz_charts, total_page)) => {
                    show_message(tl!("loaded")).ok().duration(1.);
                    self.total_page = total_page;
                    (state.charts_online, self.illu_files) = charts.into_iter().unzip();
                    state.pz_charts = pz_charts;
                }
                Err(err) => {
                    self.first_time = true;
                    show_error(err.context(tl!("load-failed")));
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool> {
        let t = state.t;
        if self.loading.is_none() && self.order_box.touch(touch) {
            self.page = 0;
            self.refresh(state);
            return Ok(true);
        }
        if self.scroll.touch(touch, t) {
            self.choose = None;
            return Ok(true);
        } else if let Some(pos) = self.scroll.position(touch) {
            let id = get_touched(pos);
            let trigger = trigger_grid(touch.phase, &mut self.choose, id);
            if trigger {
                let id = id.unwrap() as usize;
                if id < state.charts_online.len() {
                    let path = format!("download/{}", state.charts_online[id].info.id.as_ref().unwrap());
                    if let Some(index) = state.charts_local.iter().position(|it| it.path == path) {
                        let that = &state.charts_local[index].illustration.1;
                        if *that != state.tex {
                            state.charts_online[id].illustration.1 = that.clone();
                        }
                    }
                    state.transit = Some((Some(self.illu_files[id].clone()), id as u32, t, Rect::default(), false));
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        let r = self.order_box.render(ui);

        ui.scope(|ui| {
            ui.dx(r.w + 0.02);
            let tr = ui
                .text(tl!("page-indicator", "now" => self.page + 1, "total" => self.total_page))
                .size(0.6)
                .pos(0., r.h / 2.)
                .anchor(0., 0.5)
                .no_baseline()
                .draw();
            if self.loading.is_none() {
                ui.dx(tr.w + 0.02);
                let r = Rect::new(0., 0.01, 0.2, r.h - 0.02);
                if self.page != 0 {
                    if ui.button("prev_page", r, tl!("prev-page")) {
                        self.page -= 1;
                        self.scroll.y_scroller.set_offset(0.);
                        self.refresh(state);
                    }
                    ui.dx(r.w + 0.01);
                }
                if self.page + 1 < self.total_page && ui.button("next_page", r, tl!("next-page")) {
                    self.page += 1;
                    self.scroll.y_scroller.set_offset(0.);
                    self.refresh(state);
                }
            }
        });
        ui.dy(r.h);
        let content_size = (state.content_size.0, state.content_size.1 - CHARTS_BAR_HEIGHT);
        SharedState::render_charts(ui, content_size, &mut self.scroll, &mut state.charts_online);
        if let Some((Some(_), id, _, rect, _)) = &mut state.transit {
            let width = content_size.0;
            *rect = ui.rect_to_global(Rect::new(
                (*id % ROW_NUM) as f32 * width / ROW_NUM as f32,
                (*id / ROW_NUM) as f32 * CARD_HEIGHT - self.scroll.y_scroller.offset(),
                width / ROW_NUM as f32,
                CARD_HEIGHT,
            ));
        }
        Ok(())
    }
}
