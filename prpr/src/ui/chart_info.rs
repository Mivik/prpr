use super::Ui;
use crate::{
    ext::RectExt,
    info::ChartInfo,
    scene::{request_input, return_input, show_message, take_input},
};
use anyhow::Result;
use std::collections::HashMap;

#[derive(Clone)]
pub struct ChartInfoEdit {
    pub info: ChartInfo,
    pub chart: Option<String>,
    pub music: Option<String>,
    pub illustration: Option<String>,
}

impl ChartInfoEdit {
    pub fn new(info: ChartInfo) -> Self {
        Self {
            info,
            chart: None,
            music: None,
            illustration: None,
        }
    }

    pub async fn to_patches(&self) -> Result<HashMap<String, Vec<u8>>> {
        let mut res = HashMap::new();
        res.insert("info.yml".to_owned(), serde_yaml::to_string(&self.info)?.into_bytes());
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(chart) = &self.chart {
                res.insert(self.info.chart.clone(), tokio::fs::read(chart).await?);
            }
            if let Some(music) = &self.music {
                res.insert(self.info.music.clone(), tokio::fs::read(music).await?);
            }
            if let Some(illustration) = &self.illustration {
                res.insert(self.info.illustration.clone(), tokio::fs::read(illustration).await?);
            }
        }
        Ok(res)
    }
}

pub fn render_chart_info(ui: &mut Ui, edit: &mut ChartInfoEdit, width: f32) -> (f32, f32) {
    let mut sy = 0.02;
    ui.scope(|ui| {
        let s = 0.01;
        ui.dx(0.01);
        ui.dy(sy);
        macro_rules! dy {
            ($dy:expr) => {{
                let dy = $dy;
                sy += dy;
                ui.dy(dy);
            }};
        }
        let r = ui.text("编辑谱面").size(0.7).draw();
        dy!(r.h + 0.04);
        let rt = ui.text("显示难度").size(0.4).measure().w;
        ui.dx(rt);
        let len = width - 0.2;
        let info = &mut edit.info;
        let r = ui.input("谱面名", &mut info.name, len);
        dy!(r.h + s);
        let r = ui.input("作者", &mut info.charter, len);
        dy!(r.h + s);
        let r = ui.input("曲师", &mut info.composer, len);
        dy!(r.h + s);
        let r = ui.input("画师", &mut info.illustrator, len);
        dy!(r.h + s + 0.02);

        let r = ui.input("显示难度", &mut info.level, len);
        dy!(r.h + s);

        ui.dx(-rt);
        let r = ui.slider("难度", 0.0..20.0, 0.1, &mut info.difficulty, Some(width - 0.2));
        dy!(r.h + s + 0.01);
        ui.dx(rt);

        let mut string = format!("{:.2}", info.preview_time);
        let r = ui.input("预览时间", &mut string, len);
        dy!(r.h + s);
        match string.parse::<f32>() {
            Err(_) => {
                show_message("输入非法");
            }
            Ok(value) => {
                info.preview_time = value;
            }
        }

        let mut string = format!("{:.3}", info.offset);
        let r = ui.input("偏移(s)", &mut string, len);
        dy!(r.h + s);
        match string.parse::<f32>() {
            Err(_) => {
                show_message("输入非法");
            }
            Ok(value) => {
                info.offset = value;
            }
        }

        let mut string = format!("{:.5}", info.aspect_ratio);
        let r = ui.input("宽高比", &mut string, len);
        dy!(r.h + s);
        match || -> Result<f32> {
            if let Some((w, h)) = string.split_once(':') {
                Ok(w.parse::<f32>()? / h.parse::<f32>()?)
            } else {
                Ok(string.parse()?)
            }
        }() {
            Err(_) => {
                show_message("输入非法");
            }
            Ok(value) => {
                info.aspect_ratio = value;
            }
        }
        dy!(ui.scope(|ui| {
            ui.text("注：").anchor(1., 0.).size(0.35).draw();
            ui.text("宽高比可以直接填小数，也可以是 w:h 的形式（英文半角冒号）")
                .size(0.35)
                .max_width(len)
                .multiline()
                .draw()
                .h
                + 0.03
        }));

        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::scene::{request_file, return_file, take_file};
            use macroquad::prelude::Rect;
            let mut choose_file = |id: &str, label: &str, value: &str| {
                let r = ui.text(label).size(0.4).anchor(1., 0.).draw();
                let r = Rect::new(0.02, r.y - 0.01, len, r.h + 0.02);
                if ui.button(id, r, value) {
                    request_file(id);
                }
                dy!(r.h + s);
            };
            choose_file("file_chart", "谱面文件", &info.chart);
            choose_file("file_music", "音乐文件", &info.music);
            choose_file("file_illustration", "插图文件", &info.illustration);
            if let Some((id, file)) = take_file() {
                match id.as_str() {
                    "file_chart" => {
                        edit.chart = Some(file);
                    }
                    "file_music" => {
                        edit.music = Some(file);
                    }
                    "file_illustration" => {
                        edit.illustration = Some(file);
                    }
                    _ => return_file(id, file),
                }
            }
        }

        let mut string = info.tip.clone().unwrap_or_default();
        let r = ui.input("Tip", &mut string, len);
        dy!(r.h + s);
        info.tip = if string.is_empty() { None } else { Some(string) };

        let r = ui.input("简介", &mut info.intro, len);
        dy!(r.h + s + 0.02);

        let r = ui.text("标签").anchor(1., 0.).size(0.4).draw();
        ui.dx(0.02);
        let max = width - 0.1;
        let mut cx = 0.;
        let mut line_height = r.h;
        let pad = 0.01;
        let mut remove = None;
        for (id, tag) in info.tags.iter().map(|it| it.as_str()).chain(std::iter::once("+")).enumerate() {
            let mut r = ui.text(tag).size(0.4).measure().feather(0.01);
            if cx + r.w > max {
                cx = 0.;
                dy!(line_height + s);
                line_height = 0.;
            }
            line_height = line_height.max(r.h);
            r.x = cx;
            if ui.button(&format!("tag#{id}"), r, tag) {
                if id == info.tags.len() {
                    request_input("new_tag", "");
                } else {
                    remove = Some(id);
                }
            }
            cx += r.w + pad;
        }
        if let Some(remove) = remove {
            info.tags.remove(remove);
        }
        dy!(line_height + s);
        if let Some((id, text)) = take_input() {
            if id == "new_tag" {
                if info.tags.iter().any(|it| it == &text) {
                    show_message("Tag 已存在");
                } else {
                    info.tags.push(text);
                }
            } else {
                return_input(id, text);
            }
        }
        ui.dx(-0.02);
    });
    (width, sy)
}
