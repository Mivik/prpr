use super::{song::TrashBin, SongScene};
use crate::{
    billboard::BillBoard,
    cloud::{ChartItemData, Client},
    data::{BriefChartInfo, LocalChart},
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
    audio::{Audio, AudioClip, AudioHandle, DefaultAudio, PlayParams},
    core::{ParticleEmitter, Tweenable, JUDGE_LINE_PERFECT_COLOR, NOTE_WIDTH_RATIO_BASE},
    ext::{screen_aspect, SafeTexture, ScaleType},
    fs,
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Scroll, Ui},
};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Mutex},
};
use tempfile::NamedTempFile;

const SIDE_PADDING: f32 = 0.02;
const ROW_NUM: u32 = 4;
const CARD_HEIGHT: f32 = 0.3;
const CARD_PADDING: f32 = 0.02;

const SWITCH_TIME: f32 = 0.4;
const TRANSIT_TIME: f32 = 0.4;

pub static CHOSEN_FILE: Mutex<Option<String>> = Mutex::new(None);
pub static SHOULD_DELETE: AtomicBool = AtomicBool::new(false);

fn load_local(tex: &SafeTexture) -> Vec<ChartItem> {
    get_data()
        .unwrap()
        .charts
        .iter()
        .map(|it| ChartItem {
            info: it.info.clone(),
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
    pub info: BriefChartInfo,
    pub path: String,
    pub illustration: SafeTexture,
    pub illustration_task: Task<Result<DynamicImage>>,
}

pub struct MainScene {
    target: Option<RenderTarget>,
    next_scene: Option<NextScene>,
    scroll_local: Scroll,
    scroll_remote: Scroll,
    tex: SafeTexture,
    click_texture: SafeTexture,
    icon_back: SafeTexture,
    icon_play: SafeTexture,
    icon_delete: SafeTexture,
    icon_question: SafeTexture,

    audio: DefaultAudio,
    cali_clip: AudioClip,
    cali_hit_clip: AudioClip,
    cali_handle: Option<AudioHandle>,
    cali_tm: TimeManager,
    cali_last: bool,
    emitter: ParticleEmitter,

    billboard: BillBoard,

    task_load: Task<Result<Vec<ChartItem>>>,
    remote_first_time: bool,
    loading_remote: bool,
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
    transit: Option<(u32, f32, Rect, bool)>,
}

impl MainScene {
    pub async fn new() -> Result<Self> {
        let tex: SafeTexture = Texture2D::from_image(&load_image("player.jpg").await?).into();
        let audio = DefaultAudio::new()?;
        let cali_clip = audio.create_clip(load_file("cali.ogg").await?)?.0;
        let cali_hit_clip = audio.create_clip(load_file("cali_hit.ogg").await?)?.0;

        let mut cali_tm = TimeManager::new(1., true);
        cali_tm.force = 3e-2;
        macro_rules! load_tex {
            ($path:literal) => {
                SafeTexture::from(Texture2D::from_image(&load_image($path).await?))
            };
        }
        Ok(Self {
            target: None,
            next_scene: None,
            scroll_local: Scroll::new(),
            scroll_remote: Scroll::new(),
            tex: tex.clone(),
            click_texture: load_tex!("click.png"),
            icon_back: load_tex!("back.png"),
            icon_play: load_tex!("resume.png"),
            icon_delete: load_tex!("delete.png"),
            icon_question: load_tex!("question.png"),

            audio,
            cali_clip,
            cali_hit_clip,
            cali_handle: None,
            cali_tm,
            cali_last: false,
            emitter: ParticleEmitter::new().await?,

            billboard: BillBoard::new(),

            task_load: Task::pending(),
            remote_first_time: true,
            loading_remote: false,
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
            transit: None,
        })
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
                ui.fill_path(&path, (*chart.illustration, Rect::new(0., 0., cw, ch)));
                ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
                ui.text(&chart.info.name)
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
                if let Some((id, _, rect, _)) = &mut self.transit {
                    *rect = ui.rect_to_global(Rect::new(
                        (*id % ROW_NUM) as f32 * width / ROW_NUM as f32,
                        (*id / ROW_NUM) as f32 * CARD_HEIGHT - self.scroll_local.y_scroller.offset(),
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
                    ui.text("+").pos(ct.x, ct.y).anchor(0.5, 0.5).size(1.4).draw();
                }
                ui.dx(content_size.0);
                Self::render_scroll(ui, content_size, &mut self.scroll_remote, &mut self.charts_remote);
                ui.dx(content_size.0);
                if Self::render_settings(ui, &self.click_texture, self.cali_tm.now() as _, &mut self.cali_last, &mut self.emitter)
                    && self.tab_index == 2
                {
                    let _ = self.audio.play(&self.cali_hit_clip, PlayParams::default());
                }
                (content_size.0 * 3., content_size.1)
            });
        });
    }

    fn render_settings(ui: &mut Ui, click: &SafeTexture, cali_t: f32, cali_last: &mut bool, emitter: &mut ParticleEmitter) -> bool {
        let config = &mut get_data_mut().unwrap().config;
        let s = 0.01;
        let mut result = false;
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
                let r = ui.slider("偏移(s)", -0.5..0.5, 0.005, &mut config.offset);
                ui.dy(r.h + s);
                let r = ui.slider("速度", 0.8..1.2, 0.005, &mut config.speed);
                ui.dy(r.h + s);
                let r = ui.slider("音符大小", 0.8..1.2, 0.005, &mut config.note_scale);
                ui.dy(r.h + s);
                let r = ui.slider("音乐音量", 0.0..2.0, 0.05, &mut config.volume_music);
                ui.dy(r.h + s);
                let r = ui.slider("音效音量", 0.0..2.0, 0.05, &mut config.volume_sfx);
                ui.dy(r.h + s);
            });

            let ct = (0.8, ui.top * 1.3);
            let len = 0.25;
            ui.fill_rect(Rect::new(ct.0 - len, ct.1 - 0.005, len * 2., 0.01), WHITE);
            let mut cali_t = cali_t - config.offset;
            if cali_t < 0. {
                cali_t += 2.;
            }
            if cali_t >= 2. {
                cali_t -= 2.;
            }
            if cali_t <= 1. {
                let w = NOTE_WIDTH_RATIO_BASE * config.note_scale * 2.;
                let h = w * click.height() / click.width();
                let r = Rect::new(ct.0 - w / 2., ct.1 + (cali_t - 1.) * 0.4, w, h);
                ui.fill_rect(r, (**click, r));
                *cali_last = true;
            } else {
                if *cali_last {
                    let g = ui.to_global(ct);
                    emitter.emit_at(vec2(g.0, g.1), JUDGE_LINE_PERFECT_COLOR);
                    result = true;
                }
                *cali_last = false;
            }
        });
        emitter.draw(get_frame_time());
        result
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

    fn refresh_remote(&mut self, tm: &TimeManager) {
        if self.loading_remote {
            return;
        }
        self.charts_remote.clear();
        self.billboard.add("正在加载", tm.now() as _);
        self.loading_remote = true;
        self.task_load = Task::new({
            let tex = self.tex.clone();
            async move {
                let cli = Client::new("https://uxjq2roe.lc-cn-n1-shared.com/1.1", "uxjq2ROe26ucGlFXIbWYOhEW-gzGzoHsz", "LW6yy6lkSFfXDqZo0442oFjT");
                let charts: Vec<ChartItemData> = cli.query().await?;
                Ok(charts
                    .into_iter()
                    .map(|it| {
                        let url = it.illustration;
                        ChartItem {
                            info: BriefChartInfo {
                                id: Some(it.id),
                                ..it.info.clone()
                            },
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
}

impl Scene for MainScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.tab_start_time = f32::NEG_INFINITY;
        self.target = target;
        if let Some((_, st, _, true)) = &mut self.transit {
            *st = tm.now() as _;
        } else {
            self.billboard.clear();
            self.billboard.add("欢迎回来", tm.now() as _);
        }
        Ok(())
    }

    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        save_data()?;
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: Touch) -> Result<()> {
        if tm.now() as f32 <= self.tab_start_time + SWITCH_TIME || self.transit.is_some() {
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
                if tab_id == 1 && self.remote_first_time {
                    self.remote_first_time = false;
                    self.refresh_remote(tm);
                }
                if tab_id == 2 {
                    self.cali_handle = Some(self.audio.play(
                        &self.cali_clip,
                        PlayParams {
                            loop_: true,
                            volume: 0.7,
                            ..Default::default()
                        },
                    )?);
                    self.cali_tm.reset();
                }
                if self.tab_from_index == 2 {
                    if let Some(handle) = &mut self.cali_handle {
                        self.audio.pause(handle)?;
                    }
                    self.cali_handle = None;
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
        if self.tab_index == 0 && !self.scroll_local.touch(touch.clone(), t) {
            if let Some(pos) = self.scroll_local.position(&touch) {
                let id = Self::get_touched(pos);
                let trigger = Self::trigger_grid(touch.phase, &mut self.choose_local, id);
                if trigger {
                    let id = id.unwrap();
                    if id < self.charts_local.len() as u32 {
                        self.transit = Some((id, tm.now() as _, Rect::default(), false));
                    }
                    return Ok(());
                }
            }
        } else {
            self.choose_local = None;
        }
        if self.tab_index == 1 && !self.scroll_remote.touch(touch.clone(), t) {
            if let Some(pos) = self.scroll_remote.position(&touch) {
                let id = Self::get_touched(pos);
                let trigger = Self::trigger_grid(touch.phase, &mut self.choose_remote, id);
                if trigger {
                    let id = id.unwrap();
                    if id < self.charts_remote.len() as u32 {
                        let chart_id = self.charts_remote[id as usize].info.id.as_ref().unwrap();
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
                            info: chart.info.clone(),
                            path,
                        };
                        self.downloading.insert(
                            chart_id.clone(),
                            (
                                chart.info.name.clone(),
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
        if self.scroll_remote.y_scroller.pulled {
            self.refresh_remote(tm);
        }
        self.billboard.update(t);
        self.scroll_local.update(t);
        self.scroll_remote.update(t);
        let p = ((tm.now() as f32 - self.tab_start_time) / SWITCH_TIME).min(1.);
        if let Some(handle) = &self.cali_handle {
            let pos = self.audio.position(handle)?;
            let now = self.cali_tm.now();
            if now > 2. {
                self.cali_tm.seek_to(now - 2.);
                self.cali_tm.dont_wait();
            }
            let now = self.cali_tm.now();
            if now - pos >= -1. {
                self.cali_tm.update(pos);
            }
        }
        if p < 1. {
            let p = 1. - (1. - p).powi(3);
            self.tab_scroll
                .set_offset(f32::tween(&(self.tab_from_index as f32), &(self.tab_index as f32), p) * (1. - SIDE_PADDING) * 2., 0.);
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
            self.loading_remote = false;
            match charts {
                Ok(charts) => {
                    self.billboard.add("加载完成", tm.now() as _);
                    self.charts_remote = charts;
                }
                Err(err) => {
                    self.remote_first_time = true;
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
                    info: BriefChartInfo { id: None, ..info.into() },
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
        let mut ui = Ui::new();
        ui.scope(|ui| self.ui(ui, tm.now() as f32));
        ui.scope(|ui| self.billboard.render(ui));
        if let Some((id, st, rect, back)) = &mut self.transit {
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
            let chart = &self.charts_local[*id as usize];
            ui.fill_path(&path, (*chart.illustration, rect, ScaleType::Scale));
            ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
            if *back && p <= 0. {
                if SHOULD_DELETE.fetch_and(false, std::sync::atomic::Ordering::SeqCst) {
                    let err: Result<_> = (|| {
                        let Some((id, ..)) = self.transit else {unreachable!()};
                        let id = id as usize;
                        let path = format!("{}/{}", dir::charts()?, self.charts_local[id].path);
                        let path = std::path::Path::new(&path);
                        if path.is_file() {
                            std::fs::remove_file(path)?;
                        } else {
                            std::fs::remove_dir_all(path)?;
                        }
                        get_data_mut().unwrap().charts.remove(id);
                        save_data()?;
                        self.charts_local.remove(id);
                        Ok(())
                    })();
                    if let Err(err) = err {
                        self.billboard.add(format!("删除失败：{err:?}"), tm.now() as _);
                    } else {
                        self.billboard.add("删除成功", tm.now() as _);
                    }
                }
                self.transit = None;
            } else if !*back && p >= 1. {
                self.next_scene = Some(NextScene::Overlay(Box::new(SongScene::new(
                    ChartItem {
                        info: chart.info.clone(),
                        path: chart.path.clone(),
                        illustration: chart.illustration.clone(),
                        illustration_task: Task::pending(),
                    },
                    chart.illustration.clone(),
                    self.icon_back.clone(),
                    self.icon_play.clone(),
                    TrashBin::new(self.icon_delete.clone(), self.icon_question.clone()),
                ))));
                *back = true;
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
