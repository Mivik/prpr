use super::{get_touched, trigger_grid, ChartItem, Page, SharedState, CARD_HEIGHT, ROW_NUM};
use crate::{
    cloud::{Client, Images, LCChartItem},
    data::BriefChartInfo,
    scene::ChartOrderBox,
    task::Task,
};
use anyhow::Result;
use macroquad::prelude::{Rect, Touch};
use prpr::{
    ext::SafeTexture,
    scene::{show_error, show_message},
    ui::{Scroll, Ui},
};

pub struct RemotePage {
    focus: bool,

    scroll: Scroll,
    choose: Option<u32>,

    order_box: ChartOrderBox,

    task_load: Task<Result<Vec<ChartItem>>>,
    first_time: bool,
    loading: bool,
}

impl RemotePage {
    pub fn new(icon_play: SafeTexture) -> Self {
        Self {
            focus: false,

            scroll: Scroll::new(),
            choose: None,

            order_box: ChartOrderBox::new(icon_play),

            task_load: Task::pending(),
            first_time: true,
            loading: false,
        }
    }

    fn refresh_remote(&mut self, state: &mut SharedState) {
        if self.loading {
            return;
        }
        state.charts_remote.clear();
        show_message("正在加载");
        self.loading = true;
        let order = self.order_box.to_order();
        self.task_load = Task::new({
            let tex = state.tex.clone();
            async move {
                let charts: Vec<LCChartItem> = Client::query().order("updatedAt").send().await?;
                let mut charts = charts
                    .into_iter()
                    .map(|it| {
                        let illu = it.illustration;
                        ChartItem {
                            info: BriefChartInfo {
                                id: it.id,
                                ..it.info.clone()
                            },
                            path: it.file.url,
                            illustration: tex.clone(),
                            illustration_task: Some(Task::new(async move { Images::load(&illu).await })),
                        }
                    })
                    .collect::<Vec<_>>();
                order.0.apply(&mut charts);
                if order.1 {
                    charts.reverse();
                }
                Ok(charts)
            }
        });
    }
}

impl Page for RemotePage {
    fn label(&self) -> &'static str {
        "在线"
    }

    fn update(&mut self, focus: bool, state: &mut SharedState) -> Result<()> {
        if !self.focus && focus && self.first_time {
            self.first_time = false;
            self.refresh_remote(state);
        }
        self.focus = focus;

        let t = state.t;
        if self.scroll.y_scroller.pulled {
            self.refresh_remote(state);
        }
        self.scroll.update(t);
        if let Some(charts) = self.task_load.take() {
            self.loading = false;
            match charts {
                Ok(charts) => {
                    show_message("加载完成");
                    state.charts_remote = charts;
                }
                Err(err) => {
                    self.first_time = true;
                    show_error(err.context("加载失败"));
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool> {
        let t = state.t;
        if !self.loading && self.order_box.touch(touch) {
            self.refresh_remote(state);
            return Ok(true);
        }
        if self.scroll.touch(touch, t) {
            self.choose = None;
            return Ok(true);
        } else if let Some(pos) = self.scroll.position(touch) {
            let id = get_touched(pos);
            let trigger = trigger_grid(touch.phase, &mut self.choose, id);
            if trigger {
                let id = id.unwrap();
                if id < state.charts_remote.len() as u32 {
                    state.transit = Some((true, id, t, Rect::default(), false));
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        let r = self.order_box.render(ui);
        ui.dy(r.h);
        SharedState::render_scroll(ui, state.content_size, &mut self.scroll, &mut state.charts_remote);
        if let Some((true, id, _, rect, _)) = &mut state.transit {
            let width = state.content_size.0;
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
