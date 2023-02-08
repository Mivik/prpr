prpr::tl_file!("online");

use super::{get_touched, trigger_grid, ChartItem, Page, SharedState, CARD_HEIGHT, ROW_NUM};
use crate::{
    cloud::{Client, Images, LCChartItem, LCFile, QueryResult},
    data::BriefChartInfo,
    scene::{ChartOrder, ChartOrderBox, CHARTS_BAR_HEIGHT},
};
use anyhow::Result;
use macroquad::prelude::{Rect, Touch};
use prpr::{
    ext::SafeTexture,
    scene::{show_error, show_message, show_message_ex},
    task::Task,
    ui::{MessageHandle, Scroll, Ui, MessageKind},
};
use std::borrow::Cow;

const PAGE_NUM: usize = 28;

pub struct OnlinePage {
    focus: bool,

    scroll: Scroll,
    choose: Option<u32>,

    order_box: ChartOrderBox,

    page: usize,
    total_page: usize,

    task_load: Task<Result<(Vec<(ChartItem, LCFile)>, usize)>>,
    illu_files: Vec<LCFile>,
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
        self.loading = Some(show_message(tl!("loading")));
        let order = self.order_box.to_order();
        let page = self.page;
        self.task_load = Task::new({
            let tex = state.tex.clone();
            async move {
                let result: QueryResult<LCChartItem> = Client::query()
                    .order(match order {
                        (ChartOrder::Default, false) => "-updatedAt",
                        (ChartOrder::Default, true) => "updatedAt",
                        (ChartOrder::Name, false) => "name",
                        (ChartOrder::Name, true) => "-name",
                    })
                    .limit(PAGE_NUM)
                    .skip(page * PAGE_NUM)
                    .with_count()
                    .send()
                    .await?;
                let total_page = (result.count.unwrap() - 1) / PAGE_NUM + 1;
                let charts = result
                    .results
                    .into_iter()
                    .map(|it| {
                        let illu = it.illustration.clone();
                        (
                            ChartItem {
                                info: BriefChartInfo {
                                    id: it.id,
                                    ..it.info.clone()
                                },
                                path: it.file.url,
                                illustration: (tex.clone(), tex.clone()),
                                illustration_task: Some(Task::new(async move {
                                    let image = Images::load_lc_thumbnail(&illu).await?;
                                    Ok((image, None))
                                })),
                            },
                            it.illustration,
                        )
                    })
                    .collect::<Vec<_>>();
                Ok((charts, total_page))
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
                Ok((charts, total_page)) => {
                    show_message_ex(tl!("loaded"), MessageKind::Ok);
                    self.total_page = total_page;
                    (state.charts_online, self.illu_files) = charts.into_iter().unzip();
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
        SharedState::render_scroll(ui, content_size, &mut self.scroll, &mut state.charts_online);
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
