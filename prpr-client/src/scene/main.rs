use crate::{
    billboard::BillBoard,
    cloud::{ChartItemData, Client},
    data::LocalChart,
    dir, get_data, get_data_mut, save_data,
    task::Task,
};
use anyhow::{Context, Result};
use image::DynamicImage;
use lyon::{
    math as lm,
    path::{builder::BorderRadii, Path, Winding},
};
use macroquad::{prelude::*, texture::RenderTarget};
use prpr::{
    core::Tweenable,
    ext::{poll_future, screen_aspect, SafeTexture, ScaleType},
    fs,
    scene::LoadingScene,
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Scroll, Ui},
};
use std::{collections::HashMap, future::Future, pin::Pin, sync::Mutex};
use tempfile::NamedTempFile;

const SIDE_PADDING: f32 = 0.02;
const ROW_NUM: u32 = 4;
const CARD_HEIGHT: f32 = 0.3;
const CARD_PADDING: f32 = 0.02;

const SWITCH_TIME: f32 = 0.4;

pub static CHOSEN_FILE: Mutex<Option<String>> = Mutex::new(None);

fn load_local(tex: &SafeTexture) -> Vec<ChartItem> {
    get_data()
        .unwrap()
        .charts
        .iter()
        .map(|it| ChartItem {
            id: it.id.clone(),
            name: it.name.clone(),
            intro: it.intro.clone(),
            tags: it.tags.clone(),
            path: it.path.clone(),
            illustration: tex.clone(),
            illustration_task: Task::new(async move {
                let fs = fs::fs_from_file(&std::path::Path::new(&format!("{}/{}", dir::charts()?, it.path)))?;
                let (info, mut fs) = fs::load_info(fs).await?;
                Ok(image::load_from_memory(&fs.load_file(&info.illustration).await?)?)
            }),
        })
        .collect()
}

pub struct ChartItem {
    pub id: Option<String>,
    pub name: String,
    pub intro: String,
    pub tags: Vec<String>,
    pub path: String,
    pub illustration: SafeTexture,
    pub illustration_task: Task<Result<DynamicImage>>,
}

pub struct MainScene {
    target: Option<RenderTarget>,
    next_scene: Option<NextScene>,
    future: Option<Pin<Box<dyn Future<Output = Result<LoadingScene>>>>>,
    scroll_local: Scroll,
    scroll_remote: Scroll,
    tex: SafeTexture,

    billboard: BillBoard,

    task_load: Task<Result<Vec<ChartItem>>>,
    task_started: bool,
    charts_local: Vec<ChartItem>,
    charts_remote: Vec<ChartItem>,

    choose_local: Option<u32>,
    choose_remote: Option<u32>,

    tab_scroll: Scroll,
    tab_index: usize,
    tab_buttons: [RectButton; 3],
    tab_start_time: f32,
    tab_from_index: usize,

    import_button: RectButton,
    import_task: Task<Result<LocalChart>>,

    downloading: HashMap<String, (String, Task<Result<LocalChart>>)>,
}

impl MainScene {
    pub fn new(tex: SafeTexture) -> Self {
        Self {
            target: None,
            next_scene: None,
            future: None,
            scroll_local: Scroll::new(),
            scroll_remote: Scroll::new(),
            tex: tex.clone(),

            billboard: BillBoard::new(),

            task_load: Task::pending(),
            task_started: false,
            charts_local: load_local(&tex),
            charts_remote: Vec::new(),

            choose_local: None,
            choose_remote: None,

            tab_scroll: Scroll::new(),
            tab_index: 0,
            tab_buttons: [RectButton::new(); 3],
            tab_start_time: f32::NEG_INFINITY,
            tab_from_index: 0,

            import_button: RectButton::new(),
            import_task: Task::pending(),

            downloading: HashMap::new(),
        }
    }

    fn render_scroll(ui: &mut Ui, content_size: (f32, f32), scroll: &mut Scroll, charts: &mut Vec<ChartItem>) {
        scroll.size(content_size);
        scroll.render(ui, |ui| {
            let cw = content_size.0 / ROW_NUM as f32;
            let ch = CARD_HEIGHT;
            let p = CARD_PADDING;
            let path = {
                let mut path = Path::builder();
                path.add_rounded_rectangle(&lm::Box2D::new(lm::point(p, p), lm::point(cw - p, ch - p)), &BorderRadii::new(0.01), Winding::Positive);
                path.build()
            };
            ui.hgrids(content_size.0, ch, ROW_NUM, charts.len() as u32, |ui, id| {
                let chart = &mut charts[id as usize];
                if let Some(image) = chart.illustration_task.take() {
                    let image = image.unwrap();
                    chart.illustration = Texture2D::from_image(&Image {
                        width: image.width() as _,
                        height: image.height() as _,
                        bytes: image.into_rgba8().into_vec(),
                    })
                    .into();
                }
                ui.fill_path(&path, (*chart.illustration, Rect::new(0., 0., cw, ch), ScaleType::Scale));
                ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
                ui.text(&chart.name)
                    .pos(p + 0.01, ch - p - 0.02)
                    .max_width(cw - p * 2.)
                    .anchor(0., 1.)
                    .size(0.6)
                    .draw();
            })
        });
    }

    fn ui(&mut self, ui: &mut Ui, t: f32) {
        let px = SIDE_PADDING;
        ui.scope(|ui| {
            ui.dx(-1. + px);
            ui.dy(-ui.top + 0.03);
            let mut dx = 0.;
            let mut max_height: f32 = 0.;
            let mut from_range = (0., 0.);
            let mut current_range = (0., 0.);
            for (id, tab) in ["本地", "在线", "设置"].into_iter().enumerate() {
                let r = ui.text(tab).pos(dx, 0.).size(0.9).draw();
                self.tab_buttons[id].set(ui, Rect::new(r.x, r.y, r.w, r.h + 0.01));
                max_height = max_height.max(r.h);
                let range = (dx, dx + r.w);
                if self.tab_from_index == id {
                    from_range = range;
                }
                if self.tab_index == id {
                    current_range = range;
                }
                dx += r.w + 0.02;
            }
            let draw_range = if t >= self.tab_start_time + SWITCH_TIME {
                current_range
            } else {
                let p = (t - self.tab_start_time) / SWITCH_TIME;
                let p = 1. - (1. - p).powi(3);
                (f32::tween(&from_range.0, &current_range.0, p), f32::tween(&from_range.1, &current_range.1, p))
            };
            ui.fill_rect(Rect::new(draw_range.0, max_height + 0.02, draw_range.1 - draw_range.0, 0.01), WHITE);
            ui.dy(max_height + 0.04);
            let pos = ui.to_global((0., 0.)).1;
            let width = (1. - px) * 2.;
            let content_size = (width, ui.top - pos - 0.01);
            self.tab_scroll.size(content_size);
            self.tab_scroll.render(ui, |ui| {
                Self::render_scroll(ui, content_size, &mut self.scroll_local, &mut self.charts_local);
                {
                    let pad = 0.03;
                    let rad = 0.06;
                    let r = Rect::new(content_size.0 - pad - rad * 2., content_size.1 - pad - rad * 2., rad * 2., rad * 2.);
                    let ct = r.center();
                    ui.fill_circle(ct.x, ct.y, rad, ui.accent());
                    self.import_button.set(ui, r);
                    ui.text("+").pos(ct.x, ct.y).anchor(0.5, 0.5).size(1.4).draw();
                }
                ui.dx(content_size.0);
                Self::render_scroll(ui, content_size, &mut self.scroll_remote, &mut self.charts_remote);
                ui.dx(content_size.0);
                Self::render_settings(ui);
                (content_size.0 * 3., content_size.1)
            });
        });
    }

    fn render_settings(ui: &mut Ui) {
        let config = &mut get_data_mut().unwrap().config;
        let s = 0.01;
        ui.scope(|ui| {
            ui.dx(0.02);
            ui.scope(|ui| {
                let r = ui.checkbox("自动游玩", &mut config.autoplay);
                ui.dy(r.h + s);
                let r = ui.checkbox("双押提示", &mut config.multiple_hint);
                ui.dy(r.h + s);
                let r = ui.checkbox("固定宽高比", &mut config.fix_aspect_ratio);
                ui.dy(r.h + s);
                let r = ui.checkbox("自动对齐时间", &mut config.adjust_time);
                ui.dy(r.h + s);
                let r = ui.checkbox("粒子效果", &mut config.particle);
                ui.dy(r.h + s);
                let r = ui.checkbox("激进优化", &mut config.aggressive);
                ui.dy(r.h + s);
            });
            ui.dx(0.4);

            ui.scope(|ui| {
                let r = ui.slider("偏移", -0.5..0.5, 0.005, &mut config.offset);
                ui.dy(r.h + s);
                let r = ui.slider("速度", 0.8..1.2, 0.005, &mut config.speed);
                ui.dy(r.h + s);
                let r = ui.slider("音符大小", 0.8..1.2, 0.005, &mut config.note_scale);
                ui.dy(r.h + s);
                let r = ui.slider("音乐音量", 0.0..2.0, 0.1, &mut config.volume_music);
                ui.dy(r.h + s);
                let r = ui.slider("音效音量", 0.0..2.0, 0.1, &mut config.volume_sfx);
                ui.dy(r.h + s);
            });
        });
    }

    fn get_touched(pos: (f32, f32)) -> Option<u32> {
        let row = (pos.1 / CARD_HEIGHT) as i32;
        if row < 0 {
            return None;
        }
        let width = (2. - SIDE_PADDING * 2.) / ROW_NUM as f32;
        let column = (pos.0 / width) as i32;
        if column < 0 || column >= ROW_NUM as i32 {
            return None;
        }
        let x = pos.0 - width * column as f32;
        if x < CARD_PADDING || x + CARD_PADDING >= width {
            return None;
        }
        let y = pos.1 - CARD_HEIGHT * row as f32;
        if y < CARD_PADDING || y + CARD_PADDING >= CARD_HEIGHT {
            return None;
        }
        let id = row as u32 * ROW_NUM + column as u32;
        Some(id)
    }

    fn trigger_grid(phase: TouchPhase, choose: &mut Option<u32>, id: Option<u32>) -> bool {
        match phase {
            TouchPhase::Started => {
                *choose = id;
                false
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                if *choose != id {
                    *choose = None;
                }
                false
            }
            TouchPhase::Cancelled => {
                *choose = None;
                false
            }
            TouchPhase::Ended => choose.take() == id && id.is_some(),
        }
    }
}

impl Scene for MainScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.tab_start_time = f32::NEG_INFINITY;
        tm.reset();
        self.target = target;
        self.billboard.clear();
        self.billboard.add("欢迎回来", tm.now() as _);
        Ok(())
    }

    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        save_data()?;
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: Touch) -> Result<()> {
        if tm.now() as f32 <= self.tab_start_time + SWITCH_TIME {
            return Ok(());
        }
        if let Some(tab_id) = self.tab_buttons.iter_mut().position(|it| it.touch(&touch)) {
            if tab_id != self.tab_index {
                self.tab_from_index = self.tab_index;
                self.tab_index = tab_id;
                self.tab_start_time = tm.now() as f32;
                if self.tab_from_index == 2 {
                    save_data()?;
                }
                if tab_id == 1 && !self.task_started {
                    self.task_started = true;
                    self.task_load = Task::new({
                        let tex = self.tex.clone();
                        async move {
                            let cli = Client::new(
                                "https://uxjq2roe.lc-cn-n1-shared.com/1.1",
                                "uxjq2ROe26ucGlFXIbWYOhEW-gzGzoHsz",
                                "LW6yy6lkSFfXDqZo0442oFjT",
                            );
                            let charts: Vec<ChartItemData> = cli.query().await?;
                            Ok(charts
                                .into_iter()
                                .map(|it| {
                                    let url = it.illustration;
                                    ChartItem {
                                        id: Some(it.id),
                                        name: it.name,
                                        intro: it.intro,
                                        tags: it.tags,
                                        path: it.file.url,
                                        illustration: tex.clone(),
                                        illustration_task: Task::new(async move {
                                            let bytes = reqwest::get(url.url).await?.bytes().await?;
                                            let image = image::load_from_memory(&bytes)?;
                                            Ok(image)
                                        }),
                                    }
                                })
                                .collect::<Vec<_>>())
                        }
                    });
                }
                return Ok(());
            }
        }
        if self.import_button.touch(&touch) {
            *CHOSEN_FILE.lock().unwrap() = None;
            #[cfg(not(target_os = "android"))]
            {
                use nfd::Response;
                let result = nfd::open_file_dialog(None, None)?;
                match result {
                    Response::Okay(file_path) => {
                        *CHOSEN_FILE.lock().unwrap() = Some(file_path);
                    }
                    Response::OkayMultiple(_) => unreachable!(),
                    Response::Cancel => {}
                }
            }
            #[cfg(target_os = "android")]
            unsafe {
                let env = crate::miniquad::native::attach_jni_env();
                let ctx = ndk_context::android_context().context();
                let class = (**env).GetObjectClass.unwrap()(env, ctx);
                let method = (**env).GetMethodID.unwrap()(env, class, b"chooseFile\0".as_ptr() as _, b"()V\0".as_ptr() as _);
                (**env).CallVoidMethod.unwrap()(env, ctx, method);
            }
        }
        let t = tm.now() as _;
        if !self.scroll_local.touch(touch.clone(), t) {
            if let Some(pos) = self.scroll_local.position(&touch) {
                let id = Self::get_touched(pos);
                let trigger = Self::trigger_grid(touch.phase, &mut self.choose_local, id);
                if trigger {
                    let id = id.unwrap();
                    if id < self.charts_local.len() as u32 {
                        let chart = &self.charts_local[id as usize];
                        let fs = if let Some(name) = chart.path.strip_prefix(':') {
                            fs::fs_from_assets(name)?
                        } else {
                            fs::fs_from_file(&std::path::Path::new(&format!("{}/{}", dir::charts()?, chart.path)))?
                        };
                        self.future = Some(Box::pin(async move {
                            let (info, fs) = fs::load_info(fs).await?;
                            LoadingScene::new(info, get_data().unwrap().config.clone(), fs, None).await
                        }));
                    }
                    return Ok(());
                }
            }
        } else {
            self.choose_local = None;
        }
        if !self.scroll_remote.touch(touch.clone(), t) {
            if let Some(pos) = self.scroll_remote.position(&touch) {
                let id = Self::get_touched(pos);
                let trigger = Self::trigger_grid(touch.phase, &mut self.choose_remote, id);
                if trigger {
                    let id = id.unwrap();
                    if id < self.charts_remote.len() as u32 {
                        let chart_id = self.charts_remote[id as usize].id.as_ref().unwrap();
                        dir::downloaded_charts()?;
                        let path = format!("download/{}", chart_id);
                        if get_data().unwrap().charts.iter().any(|it| it.path == path) {
                            self.billboard.add("已经下载", tm.now() as _);
                            return Ok(());
                        }
                        if self.downloading.contains_key(chart_id) {
                            self.billboard.add("已经在下载队列中", tm.now() as _);
                            return Ok(());
                        }
                        self.billboard.add("正在下载", tm.now() as _);
                        let chart = &self.charts_remote[id as usize];
                        let url = chart.path.clone();
                        let chart = LocalChart {
                            id: chart.id.clone(),
                            name: chart.name.clone(),
                            intro: chart.intro.clone(),
                            tags: chart.tags.clone(),
                            path,
                        };
                        self.downloading.insert(
                            chart_id.clone(),
                            (
                                chart.name.clone(),
                                Task::new({
                                    let path = format!("{}/{}", dir::downloaded_charts()?, chart_id);
                                    async move {
                                        tokio::fs::write(path, reqwest::get(url).await?.bytes().await?).await?;
                                        Ok(chart)
                                    }
                                }),
                            ),
                        );
                        return Ok(());
                    }
                }
            }
        } else {
            self.choose_remote = None;
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as _;
        self.billboard.update(t);
        self.scroll_local.update(t);
        self.scroll_remote.update(t);
        let p = ((tm.now() as f32 - self.tab_start_time) / SWITCH_TIME).min(1.);
        if p < 1. {
            let p = 1. - (1. - p).powi(3);
            self.tab_scroll
                .set_offset(f32::tween(&(self.tab_from_index as f32), &(self.tab_index as f32), p) * (1. - SIDE_PADDING) * 2., 0.);
        }
        if let Some(future) = &mut self.future {
            if let Some(scene) = poll_future(future.as_mut()) {
                self.future = None;
                self.next_scene = Some(NextScene::Overlay(Box::new(scene?)));
            }
        }
        let remove = self
            .downloading
            .iter_mut()
            .map(|(key, (_, task))| (key, task.inspect(|it| it.is_some())))
            .filter(|it| it.1)
            .map(|it| it.0.clone())
            .collect::<Vec<_>>();
        for id in remove {
            let mut task = self.downloading.remove(&id).unwrap();
            let res = task.1.take().unwrap();
            match res {
                Err(err) => {
                    warn!("Failed to download: {:?}", err);
                    self.billboard.add(format!("{} 下载失败", task.0), tm.now() as f32);
                }
                Ok(chart) => {
                    get_data_mut().unwrap().charts.push(chart);
                    save_data()?;
                    self.charts_local = load_local(&self.tex);
                    self.billboard.add(format!("{} 下载完成", task.0), tm.now() as f32);
                }
            }
        }
        if let Some(charts) = self.task_load.take() {
            match charts {
                Ok(charts) => {
                    self.charts_remote = charts;
                }
                Err(err) => {
                    self.billboard.add(format!("加载失败：{err:?}"), tm.now() as _);
                }
            }
        }
        if let Some(file) = CHOSEN_FILE.lock().unwrap().take() {
            async fn import(from: String) -> Result<LocalChart> {
                let file = NamedTempFile::new_in(dir::custom_charts()?)?.keep()?.1;
                std::fs::copy(from, &file).context("Failed to save")?;
                let fs = fs::fs_from_file(&std::path::Path::new(&file))?;
                let (info, _) = fs::load_info(fs).await?;
                Ok(LocalChart {
                    id: None,
                    name: info.name,
                    intro: info.intro,
                    tags: info.tags,
                    path: format!("custom/{}", file.file_name().unwrap().to_str().unwrap()),
                })
            }
            self.import_task = Task::new(import(file));
        }
        if let Some(result) = self.import_task.take() {
            match result {
                Err(err) => {
                    self.billboard.add(format!("导入失败：{err:?}"), tm.now() as _);
                }
                Ok(chart) => {
                    get_data_mut().unwrap().charts.push(chart);
                    save_data()?;
                    self.charts_local = load_local(&self.tex);
                    self.billboard.add(format!("导入成功"), tm.now() as _);
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: self.target,
            ..Default::default()
        });
        clear_background(GRAY);
        self.ui(&mut Ui::new(), tm.now() as f32);
        self.billboard.render(&mut Ui::new());
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
