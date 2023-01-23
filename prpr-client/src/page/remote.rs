use super::{get_touched, trigger_grid, ChartItem, Page, SharedState, CARD_HEIGHT, ROW_NUM};
use crate::{
    cloud::{Client, Images, LCChartItem},
    data::BriefChartInfo,
    task::Task,
};
use anyhow::Result;
use macroquad::prelude::{Rect, Touch};
use prpr::{
    scene::{show_error, show_message},
    ui::{Scroll, Ui},
};

pub struct RemotePage {
    focus: bool,

    scroll_remote: Scroll,
    choose_remote: Option<u32>,

    task_load: Task<Result<Vec<ChartItem>>>,
    remote_first_time: bool,
    loading_remote: bool,
}

impl RemotePage {
    pub fn new() -> Self {
        Self {
            focus: false,

            scroll_remote: Scroll::new(),
            choose_remote: None,

            task_load: Task::pending(),
            remote_first_time: true,
            loading_remote: false,
        }
    }

    fn refresh_remote(&mut self, state: &mut SharedState) {
        if self.loading_remote {
            return;
        }
        state.charts_remote.clear();
        show_message("正在加载");
        self.loading_remote = true;
        self.task_load = Task::new({
            let tex = state.tex.clone();
            async move {
                let charts: Vec<LCChartItem> = Client::query().order("-updatedAt").send().await?;
                Ok(charts
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
                    .collect::<Vec<_>>())
            }
        });
    }
}

impl Page for RemotePage {
    fn label(&self) -> &'static str {
        "在线"
    }

    fn update(&mut self, focus: bool, state: &mut SharedState) -> Result<()> {
        if !self.focus && focus {
            if self.remote_first_time {
                self.remote_first_time = false;
                self.refresh_remote(state);
            }
        }
        self.focus = focus;

        let t = state.t;
        if self.scroll_remote.y_scroller.pulled {
            self.refresh_remote(state);
        }
        self.scroll_remote.update(t);
        if let Some(charts) = self.task_load.take() {
            self.loading_remote = false;
            match charts {
                Ok(charts) => {
                    show_message("加载完成");
                    state.charts_remote = charts;
                }
                Err(err) => {
                    self.remote_first_time = true;
                    show_error(err.context("加载失败"));
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool> {
        let t = state.t;
        if self.scroll_remote.touch(touch, t) {
            self.choose_remote = None;
            return Ok(true);
        } else {
            if let Some(pos) = self.scroll_remote.position(&touch) {
                let id = get_touched(pos);
                let trigger = trigger_grid(touch.phase, &mut self.choose_remote, id);
                if trigger {
                    let id = id.unwrap();
                    if id < state.charts_remote.len() as u32 {
                        state.transit = Some((true, id, t, Rect::default(), false));
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        SharedState::render_scroll(ui, state.content_size, &mut self.scroll_remote, &mut state.charts_remote);
        if let Some((true, id, _, rect, _)) = &mut state.transit {
            let width = state.content_size.0;
            *rect = ui.rect_to_global(Rect::new(
                (*id % ROW_NUM) as f32 * width / ROW_NUM as f32,
                (*id / ROW_NUM) as f32 * CARD_HEIGHT - self.scroll_remote.y_scroller.offset(),
                width / ROW_NUM as f32,
                CARD_HEIGHT,
            ));
        }
        Ok(())
    }
}
