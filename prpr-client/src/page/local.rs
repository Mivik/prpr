use super::{get_touched, load_local, trigger_grid, Page, SharedState, CARD_HEIGHT, ROW_NUM, TRANSIT_ID};
use crate::{
    data::{BriefChartInfo, LocalChart},
    dir, get_data_mut, save_data,
    task::Task,
};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{
    fs,
    scene::{request_file, return_file, show_error, show_message, take_file},
    ui::{RectButton, Scroll, Ui},
};
use std::{ops::DerefMut, sync::atomic::Ordering};
use tempfile::NamedTempFile;

pub struct LocalPage {
    scroll: Scroll,
    choose: Option<u32>,

    import_button: RectButton,
    import_task: Task<Result<LocalChart>>,
}

impl LocalPage {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            scroll: Scroll::new(),
            choose: None,

            import_button: RectButton::new(),
            import_task: Task::pending(),
        })
    }
}

impl Page for LocalPage {
    fn label(&self) -> &'static str {
        "本地"
    }

    fn update(&mut self, _focus: bool, state: &mut SharedState) -> Result<()> {
        let t = state.t;
        self.scroll.update(t);
        if let Some((id, file)) = take_file() {
            if id == "chart" || id == "_import" {
                async fn import(from: String) -> Result<LocalChart> {
                    let file = NamedTempFile::new_in(dir::custom_charts()?)?.keep()?.1;
                    std::fs::copy(from, &file).context("Failed to save")?;
                    let mut fs = fs::fs_from_file(std::path::Path::new(&file))?;
                    let info = fs::load_info(fs.deref_mut()).await?;
                    Ok(LocalChart {
                        info: BriefChartInfo {
                            id: Option::None,
                            ..info.into()
                        },
                        path: format!("custom/{}", file.file_name().unwrap().to_str().unwrap()),
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
                    show_error(err.context("导入失败"));
                }
                Ok(chart) => {
                    get_data_mut().add_chart(chart);
                    save_data()?;
                    state.charts_local = load_local(&state.tex);
                    show_message("导入成功");
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool> {
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
                        state.transit = Some((false, id, t, Rect::default(), false));
                        TRANSIT_ID.store(id, Ordering::SeqCst);
                    } else {
                        show_message("尚未加载完成");
                    }
                    return Ok(true);
                }
            }
        }
        if self.import_button.touch(touch) {
            request_file("chart");
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        SharedState::render_scroll(ui, state.content_size, &mut self.scroll, &mut state.charts_local);
        if let Some((false, id, _, rect, _)) = &mut state.transit {
            let width = state.content_size.0;
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
            let r = Rect::new(state.content_size.0 - pad - rad * 2., state.content_size.1 - pad - rad * 2., rad * 2., rad * 2.);
            let ct = r.center();
            ui.fill_circle(ct.x, ct.y, rad, ui.accent());
            self.import_button.set(ui, r);
            ui.text("+").pos(ct.x, ct.y).anchor(0.5, 0.5).size(1.4).no_baseline().draw();
        }
        Ok(())
    }
}
