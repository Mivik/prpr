prpr::tl_file!("local");

use super::{get_touched, load_local, trigger_grid, Page, SharedState, CARD_HEIGHT, ROW_NUM, SHOULD_UPDATE};
use crate::{
    data::{BriefChartInfo, LocalChart},
    dir, get_data_mut, save_data,
    scene::{ChartOrderBox, CHARTS_BAR_HEIGHT},
};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{
    ext::SafeTexture,
    fs,
    scene::{request_file, return_file, show_error, show_message, take_file},
    task::Task,
    ui::{RectButton, Scroll, Ui},
};
use std::{borrow::Cow, ops::DerefMut, path::Path, sync::atomic::Ordering};

pub struct LocalPage {
    scroll: Scroll,
    choose: Option<u32>,

    order_box: ChartOrderBox,

    import_button: RectButton,
    import_task: Task<Result<LocalChart>>,
}

impl LocalPage {
    pub async fn new(icon_play: SafeTexture) -> Result<Self> {
        Ok(Self {
            scroll: Scroll::new(),
            choose: None,

            order_box: ChartOrderBox::new(icon_play),

            import_button: RectButton::new(),
            import_task: Task::pending(),
        })
    }
}

impl Page for LocalPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn update(&mut self, _focus: bool, state: &mut SharedState) -> Result<()> {
        let t = state.t;
        self.scroll.update(t);
        if SHOULD_UPDATE.fetch_and(false, Ordering::SeqCst) {
            state.charts_local = load_local(&state.tex, self.order_box.to_order());
        }
        if let Some((id, file)) = take_file() {
            if id == "chart" || id == "_import" {
                async fn import(from: String) -> Result<LocalChart> {
                    let name = uuid7::uuid7().to_string();
                    let file = Path::new(&dir::custom_charts()?).join(&name);
                    std::fs::copy(from, &file).context("Failed to save")?;
                    let mut fs = fs::fs_from_file(std::path::Path::new(&file))?;
                    let info = fs::load_info(fs.deref_mut()).await?;
                    Ok(LocalChart {
                        info: BriefChartInfo {
                            id: Option::None,
                            ..info.into()
                        },
                        path: format!("custom/{name}"),
                    })
                }
                self.import_task = Task::new(import(file));
            } else {
                return_file(id, file);
            }
        }
        if let Some(result) = self.import_task.take() {
            match result {
                Err(err) => {
                    show_error(err.context(tl!("import-failed")));
                }
                Ok(chart) => {
                    get_data_mut().charts.push(chart);
                    save_data()?;
                    state.charts_local = load_local(&state.tex, self.order_box.to_order());
                    show_message(tl!("import-success"));
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool> {
        if self.order_box.touch(touch) {
            state.charts_local = load_local(&state.tex, self.order_box.to_order());
            return Ok(true);
        }
        if self.import_button.touch(touch) {
            request_file("chart");
            return Ok(true);
        }
        let t = state.t;
        if self.scroll.touch(touch, t) {
            self.choose = None;
            return Ok(true);
        } else if let Some(pos) = self.scroll.position(touch) {
            let id = get_touched(pos);
            let trigger = trigger_grid(touch.phase, &mut self.choose, id);
            if trigger {
                let id = id.unwrap();
                if let Some(chart) = state.charts_local.get(id as usize) {
                    if chart.illustration_task.is_none() {
                        state.transit = Some((None, id, t, Rect::default(), false));
                    } else {
                        show_message(tl!("not-loaded"));
                    }
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        let r = self.order_box.render(ui);
        ui.dy(r.h);
        let content_size = (state.content_size.0, state.content_size.1 - CHARTS_BAR_HEIGHT);
        SharedState::render_scroll(ui, content_size, &mut self.scroll, &mut state.charts_local);
        if let Some((None, id, _, rect, _)) = &mut state.transit {
            let width = content_size.0;
            *rect = ui.rect_to_global(Rect::new(
                (*id % ROW_NUM) as f32 * width / ROW_NUM as f32,
                (*id / ROW_NUM) as f32 * CARD_HEIGHT - self.scroll.y_scroller.offset(),
                width / ROW_NUM as f32,
                CARD_HEIGHT,
            ));
        }
        {
            let pad = 0.03;
            let rad = 0.06;
            let r = Rect::new(content_size.0 - pad - rad * 2., content_size.1 - pad - rad * 2., rad * 2., rad * 2.);
            let ct = r.center();
            ui.fill_circle(ct.x, ct.y, rad, ui.accent());
            self.import_button.set(ui, r);
            ui.text("+").pos(ct.x, ct.y).anchor(0.5, 0.5).size(1.4).no_baseline().draw();
        }
        Ok(())
    }
}
