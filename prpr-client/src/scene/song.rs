use super::main::{UPDATE_INFO, UPDATE_TEXTURE};
use crate::{
    cloud::{Client, LCChartItem, Pointer, UserManager},
    data::{BriefChartInfo, LocalChart},
    dir, get_data, get_data_mut,
    page::{illustration_task, ChartItem, SHOULD_UPDATE},
    save_data,
    task::Task,
};
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use image::DynamicImage;
use macroquad::prelude::*;
use pollster::FutureExt;
use prpr::{
    config::Config,
    core::Tweenable,
    ext::{poll_future, screen_aspect, JoinToString, LocalTask, RectExt, SafeTexture, ScaleType, BLACK_TEXTURE},
    fs::{self, update_zip, FileSystem, ZipFileSystem},
    info::ChartInfo,
    scene::{show_error, show_message, GameMode, LoadingScene, NextScene, Scene},
    time::TimeManager,
    ui::{render_chart_info, ChartInfoEdit, Dialog, RectButton, Scroll, Ui},
};
use std::{
    ops::DerefMut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::io::AsyncWriteExt;

const FADEIN_TIME: f32 = 0.3;
const EDIT_TRANSIT: f32 = 0.32;
const IMAGE_LIMIT: usize = 2 * 1024 * 1024;
const CHART_LIMIT: usize = 10 * 1024 * 1024;
const EDIT_CHART_INFO_WIDTH: f32 = 0.7;

static CONFIRM_UPLOAD: AtomicBool = AtomicBool::new(false);
static UPLOAD_STATUS: Mutex<Option<String>> = Mutex::new(None);

fn fs_from_path(path: &str) -> Result<Box<dyn FileSystem>> {
    if let Some(name) = path.strip_prefix(':') {
        fs::fs_from_assets(format!("charts/{name}/"))
    } else {
        fs::fs_from_file(std::path::Path::new(&format!("{}/{}", dir::charts()?, path)))
    }
}

pub struct TrashBin {
    icon_delete: SafeTexture,
    icon_question: SafeTexture,
    button: RectButton,
    pub clicked: bool,
    height: f32,
    offset: f32,
    time: f32,
}

impl TrashBin {
    pub const TRANSIT_TIME: f32 = 0.2;
    pub const WAIT_TIME: f32 = 1.;

    pub fn new(icon_delete: SafeTexture, icon_question: SafeTexture) -> Self {
        Self {
            icon_delete,
            icon_question,
            button: RectButton::new(),
            clicked: false,
            height: 0.,
            offset: 0.,
            time: f32::INFINITY,
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.button.touch(touch) {
            if (0.0..Self::WAIT_TIME).contains(&(t - self.time - Self::TRANSIT_TIME)) {
                // delete
                self.clicked = true;
            } else if self.time.is_infinite() {
                self.time = t;
            }
            true
        } else {
            false
        }
    }

    pub fn update(&mut self, t: f32) {
        if self.time.is_infinite() {
            self.offset = 0.;
        } else {
            let p = ((t - self.time - Self::WAIT_TIME - Self::TRANSIT_TIME) / Self::TRANSIT_TIME).min(1.);
            if p >= 0. {
                self.offset = (1. - p.powi(3)) * self.height;
                if p >= 1. {
                    self.time = f32::INFINITY;
                }
            } else {
                let p = 1. - (1. - ((t - self.time) / Self::TRANSIT_TIME).min(1.)).powi(3);
                self.offset = p * self.height;
            }
        }
    }

    pub fn render(&mut self, ui: &mut Ui, mut rect: Rect, color: Color) {
        self.button.set(ui, rect);
        self.height = rect.h;
        ui.scissor(Some(rect));
        rect.y -= self.offset;
        ui.fill_rect(rect, (*self.icon_delete, rect, ScaleType::Fit, color));
        rect.y += rect.h;
        ui.fill_rect(rect, (*self.icon_question, rect, ScaleType::Fit, color));
        ui.scissor(None);
    }
}

pub struct SongScene {
    chart: ChartItem,
    illustration: SafeTexture,
    icon_tool: SafeTexture,
    icon_edit: SafeTexture,
    icon_back: SafeTexture,
    icon_download: SafeTexture,
    icon_play: SafeTexture,
    bin: TrashBin,

    tool_button: RectButton,
    edit_button: RectButton,
    back_button: RectButton,
    center_button: RectButton,

    scroll: Scroll,
    edit_scroll: Scroll,

    info_task: Option<Task<ChartInfo>>,
    illustration_task: Option<Task<Result<DynamicImage>>>,
    chart_info: Option<ChartInfo>,
    scene_task: LocalTask<Result<LoadingScene>>,

    target: Option<RenderTarget>,
    first_in: bool,

    next_scene: Option<NextScene>,
    save_task: Option<Task<Result<()>>>,
    upload_task: Option<Task<Result<()>>>,
    info_edit: Option<ChartInfoEdit>,
    side_show_edit: bool,
    side_enter_time: f32,

    downloading: Option<(String, Arc<Mutex<f32>>, Task<Result<LocalChart>>)>,
    remote: bool,
}

fn create_info_task(path: String, brief: BriefChartInfo) -> Task<ChartInfo> {
    Task::new(async move {
        let info: Result<ChartInfo> = async {
            let mut fs = fs_from_path(&path)?;
            fs::load_info(fs.deref_mut()).await
        }
        .await;
        match info {
            Err(err) => {
                show_error(err.context("加载谱面信息失败"));
                brief.into_full()
            }
            Ok(ok) => ChartInfo {
                intro: brief.intro,
                tags: brief.tags,
                ..ok
            },
        }
    })
}

impl SongScene {
    pub fn new(
        chart: ChartItem,
        illustration: SafeTexture,
        icon_tool: SafeTexture,
        icon_edit: SafeTexture,
        icon_back: SafeTexture,
        icon_download: SafeTexture,
        icon_play: SafeTexture,
        bin: TrashBin,
        remote: bool,
    ) -> Self {
        if let Some(user) = chart.info.uploader.as_ref() {
            UserManager::request(&user.id);
        }
        let path = chart.path.clone();
        let brief = chart.info.clone();
        if let Some(user) = brief.uploader.as_ref() {
            UserManager::request(&user.id);
        }
        Self {
            chart,
            illustration,
            icon_tool,
            icon_edit,
            icon_back,
            icon_download,
            icon_play,
            bin,

            tool_button: RectButton::new(),
            edit_button: RectButton::new(),
            back_button: RectButton::new(),
            center_button: RectButton::new(),

            scroll: Scroll::new(),
            edit_scroll: Scroll::new(),

            info_task: if remote { None } else { Some(create_info_task(path, brief)) },
            illustration_task: None,
            chart_info: None,
            scene_task: None,

            target: None,
            first_in: true,

            next_scene: None,
            save_task: None,
            upload_task: None,
            info_edit: None,
            side_show_edit: true,
            side_enter_time: f32::INFINITY,

            downloading: None,
            remote,
        }
    }

    fn scroll_progress(&self) -> f32 {
        (self.scroll.y_scroller.offset() / (1. / screen_aspect() * 0.7)).clamp(0., 1.)
    }

    fn ui(&mut self, ui: &mut Ui, t: f32) {
        let sp = self.scroll_progress();
        let r = ui.screen_rect();
        ui.fill_rect(r, (*self.illustration, r));
        ui.fill_rect(r, Color::new(0., 0., 0., f32::tween(&0.55, &0.8, sp)));
        let p = ((t + FADEIN_TIME) / FADEIN_TIME).min(1.);
        let color = Color::new(1., 1., 1., p * (1. - sp));
        let r = Rect::new(-1. + 0.02, -ui.top + 0.02, 0.07, 0.07);
        ui.fill_rect(r, (*self.icon_back, r, ScaleType::Fit, color));
        self.back_button.set(ui, r);

        let s = 0.1;
        let r = Rect::new(-s, -s, s * 2., s * 2.);
        ui.fill_rect(r, (if self.remote { *self.icon_download } else { *self.icon_play }, r, ScaleType::Fit, color));
        if self.remote {
            let p = self.downloading.as_ref().map_or(0., |(_, p, _)| *p.lock().unwrap());
            let r = r.feather(0.04);
            ui.fill_rect(Rect::new(r.x, r.y + r.h * (1. - p), r.w, r.h * p), color);
        }
        self.center_button.set(ui, r);

        ui.scope(|ui| {
            ui.dx(1. - 0.03);
            ui.dy(-ui.top + 0.03);
            let s = 0.08;
            let mut r = Rect::new(-s, 0., s, s);
            let color = if self.remote { Color { a: color.a * 0.4, ..color } } else { color };
            self.bin.render(ui, r, color);
            r.x -= s + 0.02;
            ui.fill_rect(r, (*self.icon_edit, r, ScaleType::Fit, color));
            self.edit_button.set(ui, r);
            r.x -= s + 0.02;
            ui.fill_rect(r, (*self.icon_tool, r, ScaleType::Fit, color));
            self.tool_button.set(ui, r);
        });

        let color = Color::new(1., 1., 1., p);
        ui.scope(|ui| {
            ui.dx(-1.);
            ui.dy(-ui.top);
            self.scroll.size((2., ui.top * 2.));
            self.scroll.render(ui, |ui| {
                ui.dx(0.06);
                let top = ui.top * 2.;
                let mut sy = 0.;
                let r = ui
                    .text(&self.chart.info.name)
                    .pos(0., top - 0.06)
                    .anchor(0., 1.)
                    .size(1.4)
                    .color(color)
                    .draw();
                ui.text(&self.chart.info.level)
                    .pos(r.right() + 0.01, r.bottom())
                    .anchor(0., 1.)
                    .size(0.7)
                    .color(color)
                    .draw();
                ui.text(&self.chart.info.composer)
                    .pos(0., r.y - 0.02)
                    .anchor(0., 1.)
                    .size(0.4)
                    .color(Color::new(1., 1., 1., 0.77 * p))
                    .draw();
                ui.dy(top + 0.03);
                sy += top + 0.03;
                if let Some(user) = self.chart.info.uploader.as_ref() {
                    let r = Rect::new(0., 0., 0.1, 0.1);
                    if let Some(avatar) = UserManager::get_avatar(&user.id) {
                        let ct = r.center();
                        ui.fill_circle(ct.x, ct.y, r.w / 2., (*avatar, r));
                    }
                    if let Some(name) = UserManager::get_name(&user.id) {
                        ui.text(name).pos(r.right() + 0.01, r.center().y).anchor(0., 0.5).size(0.6).draw();
                    }
                    ui.dy(r.h + 0.02);
                    sy += r.h + 0.02;
                }
                let r = ui
                    .text(format!(
                        "{}\n\n{}\n\n难度：{} ({:.1})\n曲师：{}\n插图：{}",
                        self.chart.info.intro,
                        self.chart.info.tags.iter().map(|it| format!("#{it}")).join(" "),
                        self.chart.info.level,
                        self.chart.info.difficulty,
                        self.chart.info.composer,
                        self.chart.info.illustrator
                    ))
                    .multiline()
                    .max_width(2. - 0.06 * 2.)
                    .size(0.5)
                    .color(Color::new(1., 1., 1., 0.77))
                    .draw();
                ui.dy(r.h + 0.02);
                sy += r.h + 0.02;
                (2., sy + 0.06)
            });
        });
        if self.side_enter_time.is_finite() {
            let p = ((t - self.side_enter_time.abs()) / EDIT_TRANSIT).min(1.);
            let p = 1. - (1. - p).powi(3);
            let p = if self.side_enter_time < 0. { 1. - p } else { p };
            ui.fill_rect(ui.screen_rect(), Color::new(0., 0., 0., p * 0.6));
            let lf = f32::tween(&1.04, &(1. - EDIT_CHART_INFO_WIDTH), p);
            ui.scope(|ui| {
                ui.dx(lf);
                ui.dy(-ui.top);
                let r = Rect::new(-0.04, 0., 0.04, ui.top * 2.);
                ui.fill_rect(r, (Color::default(), (r.x, r.y), BLACK, (r.right(), r.y)));
                let r = Rect::new(0., 0., 1. - lf, ui.top * 2.);
                ui.fill_rect(r, BLACK);

                if self.side_show_edit {
                    self.side_chart_info(ui, t);
                } else {
                    self.side_tools(ui);
                }
            });
        }
    }

    fn side_tools(&mut self, ui: &mut Ui) {
        let pad = 0.03;
        let width = EDIT_CHART_INFO_WIDTH - pad;
        ui.dy(0.02);
        let r = ui.text("功能").size(0.7).draw();
        ui.dy(r.h + 0.03);
        let r = Rect::new(0., 0., width, 0.07);
        if ui.button("tweak_offset", r, "调整延迟") {
            self.play_chart(GameMode::TweakOffset).unwrap();
        }
        ui.dy(r.h + 0.01);
        if ui.button("exercise", r, "分段练习") {
            self.play_chart(GameMode::Exercise).unwrap();
        }
    }

    fn side_chart_info(&mut self, ui: &mut Ui, t: f32) {
        let h = 0.11;
        let pad = 0.03;
        let width = EDIT_CHART_INFO_WIDTH - pad;

        let vpad = 0.02;
        let hpad = 0.01;
        let dx = width / 3.;
        let mut r = Rect::new(hpad, ui.top * 2. - h + vpad, dx - hpad * 2., h - vpad * 2.);
        if ui.button("cancel", r, "取消") {
            self.side_enter_time = -t;
        }
        r.x += dx;
        if ui.button(
            "upload",
            r,
            if self.upload_task.is_some() {
                UPLOAD_STATUS.lock().unwrap().clone().unwrap()
            } else {
                "上传".to_owned()
            },
        ) && self.upload_task.is_none()
            && self.save_task.is_none()
        {
            if get_data().me.is_none() {
                show_message("请先登录！");
            } else if self.chart.path.starts_with(':') {
                show_message("不能上传内置谱面");
            } else if self.chart.info.uploader.is_some() {
                show_message("不能上传下载的谱面");
            } else if !CONFIRM_UPLOAD.load(Ordering::SeqCst) {
                Dialog::plain("上传须知", include_str!("upload_info.txt"))
                    .buttons(vec!["再想想".to_owned(), "确认上传".to_owned()])
                    .listener(|pos| {
                        if pos == 1 {
                            CONFIRM_UPLOAD.store(true, Ordering::SeqCst);
                        }
                    })
                    .show();
            }
        }
        r.x += dx;
        if ui.button("save", r, if self.save_task.is_some() { "保存中…" } else { "保存" }) && self.upload_task.is_none() && self.save_task.is_none()
        {
            if self.chart.path.starts_with(':') {
                show_message("不能更改内置谱面");
            } else {
                self.save_edit();
            }
        }

        self.edit_scroll.size((width, ui.top * 2. - h));
        self.edit_scroll.render(ui, |ui| {
            let (w, mut h) = render_chart_info(ui, self.info_edit.as_mut().unwrap(), width);
            ui.dx(0.02);
            ui.dy(h);
            let r = Rect::new(0., 0., EDIT_CHART_INFO_WIDTH - 0.2, 0.06);
            if ui.button("fix", r, "自动修复谱面") {
                if let Err(err) =
                    fs::fix_info(fs_from_path(&self.chart.path).unwrap().deref_mut(), &mut self.info_edit.as_mut().unwrap().info).block_on()
                {
                    show_error(err.context("修复失败"));
                } else {
                    show_message("修复成功");
                }
            }
            h += r.h + 0.1;
            (w, h)
        });
    }

    fn save_edit(&mut self) {
        if let Some(edit) = &self.info_edit {
            self.chart_info = Some(edit.info.clone());
            let path = self.chart.path.clone();
            let edit = edit.clone();
            self.save_task = Some(Task::new(async move {
                let mut fs = fs_from_path(&path)?;
                let patches = edit.to_patches().await.context("加载文件失败")?;
                if let Some(zip) = fs.as_any().downcast_mut::<ZipFileSystem>() {
                    let bytes = update_zip(&mut zip.0.lock().unwrap(), patches).context("写入配置文件失败")?;
                    std::fs::write(format!("{}/{}", dir::charts()?, path), bytes).context("保存文件失败")?;
                } else {
                    unreachable!();
                }
                Ok(())
            }));
        }
        self.update_chart_info(self.chart_info.clone().unwrap().into());
    }

    fn update_chart_info(&mut self, mut info: BriefChartInfo) {
        assert!(!self.remote);
        info.uploader = self.chart.info.uploader.clone();
        self.chart.info = info.clone();
        get_data_mut().charts[get_data().find_chart(&self.chart).unwrap()].info = info;
        let _ = save_data();
        UPDATE_INFO.store(true, Ordering::SeqCst);
    }

    fn start_download(&mut self) -> Result<()> {
        let id = self.chart.info.id.as_ref().unwrap();
        dir::downloaded_charts()?;
        let path = format!("download/{id}");
        if get_data().charts.iter().any(|it| it.path == path) {
            show_message("已经下载过"); // TODO redirect instead of showing this
            return Ok(());
        }
        show_message("正在下载");
        let url = self.chart.path.clone();
        let chart = LocalChart {
            info: self.chart.info.clone(),
            path,
        };
        let progress = Arc::new(Mutex::new(0.));
        let prog_cl = Arc::clone(&progress);
        self.downloading = Some((
            chart.info.name.clone(),
            progress,
            Task::new({
                let path = format!("{}/{}", dir::downloaded_charts()?, id);
                async move {
                    let mut file = tokio::fs::File::create(path).await?;
                    let res = reqwest::get(url).await.context("请求失败")?;
                    let size = res.content_length();
                    let mut stream = res.bytes_stream();
                    let mut count = 0;
                    while let Some(chunk) = stream.next().await {
                        let chunk = chunk.context("下载失败")?;
                        file.write_all(&chunk).await?;
                        count += chunk.len() as u64;
                        if let Some(size) = size {
                            *prog_cl.lock().unwrap() = count.min(size) as f32 / size as f32;
                        }
                    }
                    Ok(chart)
                }
            }),
        ));
        Ok(())
    }

    fn play_chart(&mut self, mode: GameMode) -> Result<()> {
        if self.scene_task.is_some() {
            return Ok(());
        }
        let fs = fs_from_path(&self.chart.path)?;
        let info = self.chart_info.clone().unwrap();
        self.scene_task = Some(Box::pin(async move {
            LoadingScene::new(
                mode,
                info,
                Config {
                    player_name: get_data().me.as_ref().map(|it| it.name.clone()).unwrap_or_else(|| "游客".to_string()),
                    skin_path: get_data().config.skin_path.as_ref().map(|it| format!("{}/{it}", dir::root().unwrap())),
                    ..get_data().config.clone()
                },
                fs,
                get_data().me.as_ref().and_then(|it| UserManager::get_avatar(&it.id)),
                None,
            )
            .await
        }));
        Ok(())
    }
}

impl Scene for SongScene {
    fn on_result(&mut self, _tm: &mut TimeManager, result: Box<dyn std::any::Any>) -> Result<()> {
        let result = match result.downcast::<anyhow::Error>() {
            Ok(error) => {
                show_error(error.context("加载谱面失败"));
                return Ok(());
            }
            Err(res) => res,
        };
        let _result = match result.downcast::<Option<f32>>() {
            Ok(offset) => {
                if let Some(offset) = *offset {
                    self.chart_info.as_mut().unwrap().offset = offset;
                    self.info_edit = Some(ChartInfoEdit::new(self.chart_info.clone().unwrap()));
                    self.save_edit();
                }
                return Ok(());
            }
            Err(res) => res,
        };
        Ok(())
    }

    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.target = target;
        if self.first_in {
            self.first_in = false;
            tm.seek_to(-FADEIN_TIME as _);
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if tm.now() < 0. {
            return Ok(false);
        }
        if self.scene_task.is_some() {
            return Ok(true);
        }
        let loaded = self.chart_info.is_some();
        if self.scroll_progress() < 0.4 {
            if self.side_enter_time.is_infinite() {
                if loaded && !self.remote {
                    if self.bin.touch(touch, tm.now() as _) {
                        return Ok(true);
                    }
                    if self.tool_button.touch(touch) {
                        self.side_show_edit = false;
                        self.side_enter_time = tm.now() as _;
                        return Ok(true);
                    }
                    if self.edit_button.touch(touch) {
                        self.info_edit = Some(ChartInfoEdit::new(self.chart_info.clone().unwrap()));
                        self.side_show_edit = true;
                        self.side_enter_time = tm.now() as _;
                        return Ok(true);
                    }
                }
                if (loaded || self.remote) && self.center_button.touch(touch) {
                    if self.remote {
                        if self.downloading.take().is_some() {
                            show_message("已取消");
                        } else {
                            self.start_download()?;
                        }
                    } else {
                        self.play_chart(GameMode::Normal)?;
                    }
                    return Ok(true);
                }
                if self.back_button.touch(touch) && (!self.remote || self.downloading.is_none()) {
                    self.next_scene = Some(NextScene::Pop);
                    return Ok(true);
                }
            } else if self.side_enter_time > 0. && tm.now() as f32 > self.side_enter_time + EDIT_TRANSIT {
                if touch.position.x < 1. - EDIT_CHART_INFO_WIDTH
                    && touch.phase == TouchPhase::Started
                    && self.save_task.is_none()
                    && self.illustration_task.is_none()
                {
                    self.side_enter_time = -tm.now() as _;
                    return Ok(true);
                }
                if self.edit_scroll.touch(touch, tm.now() as _) {
                    return Ok(true);
                }
            }
        }
        if self.side_enter_time.is_infinite() && self.scroll.touch(touch, tm.now() as _) {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if self.bin.clicked {
            self.next_scene = Some(NextScene::Pop);
            super::main::SHOULD_DELETE.store(true, Ordering::SeqCst);
        }
        if let Some(future) = &mut self.scene_task {
            if let Some(scene) = poll_future(future.as_mut()) {
                self.scene_task = None;
                self.next_scene = Some(NextScene::Overlay(Box::new(scene?)));
            }
        }
        let t = tm.now() as f32;
        self.bin.update(t);
        self.scroll.update(t);
        self.edit_scroll.update(t);
        if self.side_enter_time < 0. && -tm.now() as f32 + EDIT_TRANSIT < self.side_enter_time {
            self.side_enter_time = f32::INFINITY;
        }
        if let Some(task) = &mut self.info_task {
            if let Some(info) = task.take() {
                self.update_chart_info(info.clone().into());
                self.chart_info = Some(info);
                self.info_task = None;
            }
        }
        if let Some(task) = &mut self.save_task {
            if let Some(result) = task.take() {
                if let Err(err) = result {
                    show_error(err.context("保存失败"));
                } else {
                    if self.info_edit.as_ref().unwrap().illustration.is_some() {
                        self.illustration_task = Some(illustration_task(self.chart.path.clone()));
                    }
                    show_message("保存成功");
                }
                self.save_task = None;
            }
        }
        if let Some(task) = &mut self.upload_task {
            if let Some(result) = task.take() {
                if let Err(err) = result {
                    show_error(err.context("上传失败"));
                } else {
                    show_message("上传成功，请等待审核！");
                }
                self.upload_task = None;
            }
        }
        if let Some(task) = &mut self.illustration_task {
            if let Some(result) = task.take() {
                match result {
                    Err(err) => {
                        show_error(err.context("加载插图失败"));
                        self.illustration = BLACK_TEXTURE.clone();
                    }
                    Ok(image) => {
                        self.illustration = image.into();
                    }
                }
                *UPDATE_TEXTURE.lock().unwrap() = Some(self.illustration.clone());
                self.illustration_task = None;
            }
        }
        if CONFIRM_UPLOAD.fetch_and(false, Ordering::SeqCst) {
            *UPLOAD_STATUS.lock().unwrap() = Some("上传中…".to_owned());
            CONFIRM_UPLOAD.store(false, Ordering::SeqCst);
            let info = self.info_edit.as_ref().unwrap().info.clone();
            let path = self.chart.path.clone();
            let user_id = get_data().me.as_ref().unwrap().id.clone();
            self.upload_task = Some(Task::new(async move {
                let chart_bytes = tokio::fs::read(format!("{}/{}", dir::charts()?, path)).await.context("读取文件失败")?;
                if chart_bytes.len() > CHART_LIMIT {
                    bail!("谱面文件过大");
                }
                let mut fs = fs_from_path(&path)?;
                let image = fs.load_file(&info.illustration).await.context("读取插图失败")?;
                if image.len() > IMAGE_LIMIT {
                    bail!("插图文件过大");
                }
                *UPLOAD_STATUS.lock().unwrap() = Some("上传谱面中…".to_owned());
                let file = Client::upload_file("chart.zip", &chart_bytes).await.context("上传谱面失败")?;
                *UPLOAD_STATUS.lock().unwrap() = Some("上传插图中…".to_owned());
                let illustration = Client::upload_file("illustration.jpg", &image).await.context("上传插图失败")?;
                *UPLOAD_STATUS.lock().unwrap() = Some("保存中…".to_owned());
                let item = LCChartItem {
                    id: None,
                    info: BriefChartInfo {
                        uploader: Some(Pointer::from(user_id).with_class_name("_User")),
                        ..info.into()
                    },
                    file,
                    illustration,
                };
                Client::create(item).await?;
                Ok(())
            }));
        }
        let mut downloaded = Vec::new();
        if let Some((name, _, task)) = &mut self.downloading {
            if task.ok() {
                downloaded.push((name.clone(), task.take().unwrap()));
                self.downloading = None;
            }
        }
        for (name, res) in downloaded {
            match res {
                Err(err) => {
                    show_error(err.context(format!("{name} 下载失败")));
                }
                Ok(chart) => {
                    self.chart.info = chart.info.clone();
                    self.chart.path = chart.path.clone();
                    self.info_task = Some(create_info_task(chart.path.clone(), chart.info.clone()));
                    get_data_mut().charts.push(chart);
                    save_data()?;
                    SHOULD_UPDATE.store(true, Ordering::SeqCst);
                    self.remote = false;
                    show_message(format!("{name} 下载完成"));
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: self.target,
            ..Default::default()
        });
        self.ui(ui, tm.now() as _);
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
