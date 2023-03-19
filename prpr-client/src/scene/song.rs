prpr::tl_file!("song");

use crate::{
    data::{BriefChartInfo, LocalChart},
    dir, get_data, get_data_mut,
    page::{ChartItem, Illustration},
    phizone::{recv_raw, Client, PZChart, PZFile, PZSong, PZUser, Ptr, UserManager},
    save_data,
};
use anyhow::{anyhow, Context, Result};
use cap_std::ambient_authority;
use futures_util::StreamExt;
use macroquad::prelude::*;
use prpr::{
    config::Config,
    ext::{poll_future, screen_aspect, semi_black, semi_white, LocalTask, RectExt, SafeTexture, ScaleType},
    fs,
    info::ChartInfo,
    scene::{show_error, show_message, BasicPlayer, GameMode, LoadingScene, NextScene, RecordUpdateState, Scene},
    task::Task,
    time::TimeManager,
    ui::{button_hit, list_switch, DRectButton, Dialog, RectButton, Scroll, Ui, UI_AUDIO},
};
use sasa::{AudioClip, Music, MusicParams};
use serde::Deserialize;
use serde_json::json;
use std::{
    borrow::Cow,
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, Mutex, Weak},
};
use zip::ZipArchive;

use super::fs_from_path;

const FADE_IN_TIME: f32 = 0.3;
const CHART_ITEM_H: f32 = 0.11;

fn with_effects(data: Vec<u8>, range: Option<(u32, u32)>) -> Result<AudioClip> {
    let (mut frames, sample_rate) = AudioClip::decode(data)?;
    if let Some((begin, end)) = range {
        frames.drain((end as usize * sample_rate as usize).min(frames.len())..);
        frames.drain(..(begin as usize * sample_rate as usize));
    }
    let len = (0.8 * sample_rate as f64) as usize;
    let len = len.min(frames.len() / 2);
    for (i, frame) in frames[..len].iter_mut().enumerate() {
        let s = i as f32 / len as f32;
        frame.0 *= s;
        frame.1 *= s;
    }
    let st = frames.len() - len;
    for (i, frame) in frames[st..].iter_mut().rev().enumerate() {
        let s = i as f32 / len as f32;
        frame.0 *= s;
        frame.1 *= s;
    }
    Ok(AudioClip::from_raw(frames, sample_rate))
}

struct Downloading {
    index: usize,
    status: Arc<Mutex<Cow<'static, str>>>,
    prog: Arc<Mutex<Option<f32>>>,
    task: Task<Result<LocalChart>>,
}

struct ChartEntry {
    file: Option<PZFile>,
    assets: Option<PZFile>,
    info: BriefChartInfo,
    local_index: Option<usize>,
}

pub struct SongScene {
    illu: Illustration,

    first_in: bool,

    back_btn: RectButton,
    play_btn: DRectButton,

    icon_back: SafeTexture,
    icon_play: SafeTexture,
    icon_download: SafeTexture,

    next_scene: Option<NextScene>,

    preview: Option<Music>,
    preview_task: Option<Task<Result<AudioClip>>>,

    charts: Vec<ChartEntry>,
    cur_chart: usize,
    charts_task: Option<Task<Result<Vec<ChartEntry>>>>,
    charts_scroll: Scroll,

    downloading: Option<Downloading>,
    cancel_download_btn: DRectButton,
    loading_last: f32,

    scene_task: LocalTask<Result<LoadingScene>>,
}

impl SongScene {
    pub fn new(chart: ChartItem, local_index: Option<usize>, icon_back: SafeTexture, icon_play: SafeTexture, icon_download: SafeTexture) -> Self {
        let song_ptr = chart.info.id.map(|it| Ptr::<PZSong>::from_id(it.1));
        let cur_chart = chart.info.id.map_or(0, |it| it.0 as _);
        let (charts, charts_task) = if let &Some((_, song_id)) = &chart.info.id {
            (
                Vec::new(),
                Some(Task::new(async move {
                    let charts = Client::query::<PZChart>().query("song", song_id.to_string()).send().await?;
                    let mut entries = Vec::new();
                    let local_charts = &get_data().charts;
                    for chart in charts.0 {
                        let song = chart.song.load().await?;
                        let info = chart.to_info(&song);
                        entries.push(ChartEntry {
                            file: chart.chart,
                            assets: chart.assets,
                            info,
                            local_index: local_charts.iter().position(|local| local.info.id.map_or(false, |id| id.0 == chart.id)),
                        })
                    }
                    Ok(entries)
                })),
            )
        } else {
            (
                vec![ChartEntry {
                    file: None,
                    assets: None,
                    info: chart.info,
                    local_index,
                }],
                None,
            )
        };
        let mut charts_scroll = Scroll::new();
        charts_scroll.y_scroller.step = CHART_ITEM_H;
        Self {
            illu: chart.illu,

            first_in: true,

            back_btn: RectButton::new(),
            play_btn: DRectButton::new(),

            icon_back,
            icon_play,
            icon_download,

            next_scene: None,

            preview: None,
            preview_task: Some(Task::new(async move {
                if let Some(song) = song_ptr {
                    let file = format!("{}/{}", dir::songs()?, song.id());
                    let path = Path::new(&file);
                    if path.exists() {
                        let song: PZSong = serde_yaml::from_str(&tokio::fs::read_to_string(format!("{file}.yml")).await?).unwrap();
                        with_effects(tokio::fs::read(path).await?, Some((song.preview_start.seconds, song.preview_end.seconds)))
                    } else {
                        let song = song.load().await?;
                        if let Some(preview) = &song.preview {
                            with_effects(preview.fetch().await?.to_vec(), None)
                        } else {
                            with_effects(song.music.fetch().await?.to_vec(), Some((song.preview_start.seconds, song.preview_end.seconds)))
                        }
                    }
                } else {
                    // let mut fs = fs_from_path(&path)?;
                    // let info = fs::load_info(fs.deref_mut()).await?;
                    // if let Some(preview) = info.preview {
                    // with_effects(fs.load_file(&preview).await?, None)
                    // } else {
                    // with_effects(
                    // fs.load_file(&info.music).await?,
                    // Some((info.preview_start as u32, info.preview_end.ceil() as u32)),
                    // )
                    // }
                    todo!()
                }
            })),

            charts,
            cur_chart,
            charts_task,
            charts_scroll,

            downloading: None,
            cancel_download_btn: DRectButton::new(),
            loading_last: 0.,

            scene_task: None,
        }
    }

    fn start_download(&mut self) -> Result<()> {
        let entry = self.charts.get(self.cur_chart).unwrap();
        let chart = entry.info.clone();
        let Some(url) = entry.file.as_ref().map(|it| it.url.clone()) else {
            show_error(anyhow!(tl!("no-chart-for-download")));
            return Ok(());
        };
        let assets = entry.assets.clone();
        let progress = Arc::new(Mutex::new(None));
        let prog_wk = Arc::downgrade(&progress);
        let status = Arc::new(Mutex::new(tl!("dl-status-fetch")));
        let status_shared = Arc::clone(&status);
        self.loading_last = 0.;
        self.downloading = Some(Downloading {
            index: self.cur_chart,
            prog: progress,
            status: status_shared,
            task: Task::new({
                let path = format!("{}/{}", dir::downloaded_charts()?, chart.id.unwrap().0);
                async move {
                    let path = std::path::Path::new(&path);
                    if path.exists() {
                        if !path.is_dir() {
                            tokio::fs::remove_file(path).await?;
                        }
                    } else {
                        tokio::fs::create_dir(path).await?;
                    }
                    let dir = cap_std::fs::Dir::open_ambient_dir(path, ambient_authority())?;

                    async fn download(mut file: impl Write, url: &str, prog_wk: &Weak<Mutex<Option<f32>>>) -> Result<()> {
                        let Some(prog) = prog_wk.upgrade() else { return Ok(()) };
                        *prog.lock().unwrap() = None;
                        // let mut file = dir.create(name)?;
                        let res = reqwest::get(url).await.with_context(|| tl!("request-failed"))?;
                        let size = res.content_length();
                        let mut stream = res.bytes_stream();
                        let mut count = 0;
                        while let Some(chunk) = stream.next().await {
                            let chunk = chunk?;
                            file.write_all(&chunk)?;
                            count += chunk.len() as u64;
                            if let Some(size) = size {
                                *prog.lock().unwrap() = Some(count.min(size) as f32 / size as f32);
                            }
                            if prog_wk.strong_count() == 1 {
                                // cancelled
                                break;
                            }
                        }
                        Ok(())
                    }

                    let song = Ptr::<PZSong>::from_id(chart.id.unwrap().1).load().await?.as_ref().clone();
                    let song_path = format!("{}/{}", dir::songs()?, song.id);
                    let path = Path::new(&song_path);
                    if !path.exists() {
                        *status.lock().unwrap() = tl!("dl-status-song");
                        download(File::create(path)?, &song.music.url, &prog_wk).await?;
                        *status.lock().unwrap() = tl!("dl-status-illustration");
                        download(File::create(format!("{song_path}.jpg"))?, &song.illustration.url, &prog_wk).await?;
                        tokio::fs::write(format!("{song_path}.yml"), serde_yaml::to_string(&song).unwrap()).await?;
                    }

                    *status.lock().unwrap() = tl!("dl-status-chart");
                    download(dir.create("chart")?, &url, &prog_wk).await?;
                    if let Some(assets) = &assets {
                        *status.lock().unwrap() = tl!("dl-status-assets");
                        download(dir.create("assets.zip")?, &assets.url, &prog_wk).await?;
                        if prog_wk.strong_count() != 0 {
                            dir.create_dir("assets")?;
                            let assets = dir.open_dir("assets")?;
                            let mut zip = ZipArchive::new(dir.open("assets.zip")?)?;
                            for i in 0..zip.len() {
                                let mut entry = zip.by_index(i)?;
                                if entry.is_dir() {
                                    assets.create_dir_all(entry.name())?;
                                } else {
                                    let mut file = assets.create(entry.name())?;
                                    std::io::copy(&mut entry, &mut file)?;
                                }
                            }
                            drop(zip);
                            dir.remove_file("assets.zip")?;
                        }
                    }
                    *status.lock().unwrap() = tl!("dl-status-saving");
                    if let Some(prog) = prog_wk.upgrade() {
                        *prog.lock().unwrap() = None;
                    }
                    dir.create("song")?.write_all(song.id.to_string().as_bytes())?;
                    // if let Some(preview) = &song.preview {
                    // download(&dir, "preview", &preview.url, &prog_wk).await?;
                    // }
                    let info = ChartInfo {
                        id: chart.id,
                        name: song.name,
                        difficulty: chart.difficulty,
                        level: chart.level,
                        charter: chart.charter,
                        composer: song.composer,
                        illustrator: song.illustrator,
                        chart: ":chart".to_owned(),
                        format: None,
                        music: ":music".to_owned(),
                        illustration: ":illustration".to_owned(),
                        preview_start: song.preview_start.seconds as f32,
                        preview_end: song.preview_end.seconds as f32,
                        intro: chart.intro,
                        offset: 0.,
                        ..Default::default()
                    };
                    if prog_wk.strong_count() != 0 {
                        dir.write("info", serde_yaml::to_string(&info).unwrap())?;
                    }

                    if prog_wk.strong_count() == 0 {
                        // cancelled
                        drop(dir);
                        tokio::fs::remove_dir_all(&path).await?;
                    }

                    let local_path = format!("download/{}", chart.id.unwrap().0);
                    Ok(LocalChart {
                        info: info.into(),
                        local_path,
                    })
                }
            }),
        });
        Ok(())
    }
}

impl Scene for SongScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.first_in {
            self.first_in = false;
            tm.seek_to(-FADE_IN_TIME as _);
        }
        if let Some(music) = &mut self.preview {
            music.seek_to(0.)?;
            music.play()?;
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        if self.scene_task.is_some() {
            return Ok(false);
        }
        if self.downloading.is_some() {
            if self.cancel_download_btn.touch(touch, t) {
                self.downloading = None;
                return Ok(true);
            }
            return Ok(false);
        }
        if self.back_btn.touch(touch) {
            button_hit();
            self.next_scene = Some(NextScene::PopWithResult(Box::new(())));
            return Ok(true);
        }
        if self.play_btn.touch(touch, t) {
            if let Some(entry) = self.charts.get(self.cur_chart) {
                if let Some(index) = entry.local_index {
                    let mut fs = fs_from_path(&get_data().charts[index].local_path)?;
                    #[cfg(feature = "closed")]
                    let rated = {
                        let config = &get_data().config;
                        !config.offline_mode && entry.info.id.is_some() && !config.autoplay && config.speed >= 1.0 - 1e-3
                    };
                    #[cfg(not(feature = "closed"))]
                    let rated = false;
                    if !rated && entry.info.id.is_some() && !get_data().config.offline_mode {
                        show_message(tl!("warn-unrated")).warn();
                    }
                    // LoadingScene::new(GameMode::Normal, entry.info.clone(), get_data().config.clone(), todo!(), None, None, None);
                    self.scene_task = Some(Box::pin(async move {
                        #[derive(Deserialize)]
                        struct Resp {
                            play_token: Option<String>,
                        }
                        let info = fs::load_info(fs.as_mut()).await?;
                        let mut play_token: Option<String> = None;
                        if rated {
                            let resp: Resp = recv_raw(Client::get("/player/play/").await.query(&json!({
                                "chart": info.id.unwrap().0,
                                "config": 1,
                            })))
                            .await?
                            .json()
                            .await?;
                            play_token = Some(resp.play_token.ok_or_else(|| anyhow!("didn't receive play token"))?);
                        }
                        LoadingScene::new(
                            GameMode::Normal,
                            info,
                            Config {
                                player_name: get_data()
                                    .me
                                    .as_ref()
                                    .map(|it| it.name.clone())
                                    .unwrap_or_else(|| tl!("guest").to_string()),
                                res_pack_path: get_data()
                                    .config
                                    .res_pack_path
                                    .as_ref()
                                    .map(|it| format!("{}/{it}", dir::root().unwrap())),
                                ..get_data().config.clone()
                            },
                            fs,
                            get_data().me.as_ref().map(|it| BasicPlayer {
                                avatar: UserManager::get_avatar(it.id),
                                id: it.id,
                                rks: it.rks,
                            }),
                            None,
                            Some(Arc::new(move |mut data| {
                                let Some(play_token) = play_token.clone() else { unreachable!() };
                                data["play_token"] = play_token.into();
                                data["app"] = 3.into();
                                Task::new(async move {
                                    #[derive(Deserialize)]
                                    struct Resp {
                                        exp_delta: f64,
                                        former_rks: f64,
                                        player: PZUser,
                                        new_best: Option<bool>,
                                        improvement: Option<u32>,
                                    }
                                    let resp: Resp = recv_raw(Client::post("/records/", &data).await).await?.json().await?;
                                    Ok(RecordUpdateState {
                                        best: resp.new_best.unwrap_or_default(),
                                        improvement: resp.improvement.unwrap_or_default(),
                                        gain_exp: resp.exp_delta as f32,
                                        new_rks: resp.player.rks,
                                    })
                                })
                            })),
                        )
                        .await
                    }));
                } else {
                    self.start_download()?;
                }
            }
            return Ok(true);
        }
        if self.charts_scroll.touch(touch, t) {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        self.illu.settle(t);
        if let Some(task) = &mut self.charts_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => show_error(err.context(tl!("load-charts-failed"))),
                    Ok(charts) => {
                        self.cur_chart = charts.iter().position(|chart| chart.info.id.unwrap().0 == self.cur_chart as u64).unwrap();
                        self.charts = charts;
                        self.charts_scroll.y_scroller.offset = CHART_ITEM_H * self.cur_chart as f32;
                    }
                }
                self.charts_task = None;
            }
        }
        if let Some(task) = &mut self.preview_task {
            if let Some(result) = task.take() {
                match result {
                    Err(err) => {
                        show_error(err.context(tl!("load-preview-failed")));
                    }
                    Ok(clip) => {
                        let mut music = UI_AUDIO.with(|it| {
                            it.borrow_mut().create_music(
                                clip,
                                MusicParams {
                                    amplifier: 0.7,
                                    loop_: true,
                                    ..Default::default()
                                },
                            )
                        })?;
                        music.play()?;
                        self.preview = Some(music);
                    }
                }
                self.preview_task = None;
            }
        }
        if !self.charts.is_empty() {
            let index = ((self.charts_scroll.y_scroller.offset / CHART_ITEM_H).round() as usize).clamp(0, self.charts.len() - 1);
            if index != self.cur_chart {
                self.cur_chart = index;
                list_switch();
            }
        }
        self.charts_scroll.update(t);
        if let Some(dl) = &mut self.downloading {
            if let Some(res) = dl.task.take() {
                match res {
                    Err(err) => {
                        let path = format!("{}/{}", dir::downloaded_charts()?, self.charts[dl.index].info.id.unwrap().0);
                        let path = Path::new(&path);
                        if path.exists() {
                            std::fs::remove_dir_all(path)?;
                        }
                        show_error(err.context(tl!("dl-failed")));
                    }
                    Ok(chart) => {
                        // self.charts.as_mut().unwrap()[dl.index].info =
                        get_data_mut().charts.push(chart);
                        save_data()?;
                        self.charts[dl.index].local_index = Some(get_data().charts.len() - 1);
                        show_message(tl!("dl-success")).ok();
                    }
                }
                self.downloading = None;
            }
        }
        if let Some(task) = &mut self.scene_task {
            if let Some(result) = poll_future(task.as_mut()) {
                match result {
                    Err(err) => {
                        let error = format!("{err:?}");
                        Dialog::plain(tl!("failed-to-play"), error)
                            .buttons(vec![tl!("play-cancel").to_string(), tl!("play-switch-to-offline").to_string()])
                            .listener(move |pos| {
                                if pos == 1 {
                                    get_data_mut().config.offline_mode = true;
                                    let _ = save_data();
                                    show_message(tl!("switched-to-offline")).ok();
                                }
                            })
                            .show();
                    }
                    Ok(scene) => self.next_scene = Some(NextScene::Overlay(Box::new(scene))),
                }
                self.scene_task = None;
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            ..Default::default()
        });
        let t = tm.now() as f32;
        ui.fill_rect(ui.screen_rect(), (*self.illu.texture.1, ui.screen_rect()));
        ui.fill_rect(ui.screen_rect(), semi_black(0.55));

        let c = semi_white((t / FADE_IN_TIME).clamp(-1., 0.) + 1.);

        let r = ui.back_rect();
        self.back_btn.set(ui, r);
        ui.fill_rect(r, (*self.icon_back, r, ScaleType::Fit, c));

        if let Some(info) = self.charts.get(self.cur_chart).map(|it| &it.info) {
            let r = ui.text(&info.name).size(1.2).pos(r.right() + 0.02, r.bottom() - 0.06).color(c).draw();
            ui.text(&info.composer)
                .size(0.5)
                .pos(r.x + 0.02, r.bottom() + 0.03)
                .color(Color { a: c.a * 0.8, ..c })
                .draw();
        }

        // charts slide
        let hh = 0.35;
        let w = 0.5;
        let r = Rect::new(-1., -hh, w, hh * 2.);
        self.charts_scroll.size((r.w, r.h));
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            let mut oy = self.charts_scroll.y_scroller.offset;
            self.charts_scroll.render(ui, |ui| {
                let mut h = hh;
                ui.dy(hh);
                for chart in &self.charts {
                    ui.text(format!("{} Lv.{}", chart.info.level, chart.info.difficulty as u32))
                        .pos(0.1 - (oy * oy) / (hh * 1.5).powi(2) * 0.5, 0.)
                        .size(1.2 - (oy * oy / (0.21 * 0.21)).min(1.5) * 0.6)
                        .anchor(0., 0.5)
                        .no_baseline()
                        .color(Color {
                            a: c.a * (1. - (oy * oy) / (hh * hh)).max(0.),
                            ..c
                        })
                        .draw();
                    ui.dy(CHART_ITEM_H);
                    h += CHART_ITEM_H;
                    oy -= CHART_ITEM_H;
                }
                h += hh - CHART_ITEM_H;
                (r.w, h)
            });
        });

        // bottom bar
        let h = 0.16;
        let r = Rect::new(-1., ui.top - h, 1.7, h);
        ui.fill_rect(r, (Color::from_hex(0xff283593), (r.x, r.y), Color::default(), (r.right(), r.y)));

        // play button
        let w = 0.26;
        let pad = 0.08;
        let r = Rect::new(1. - pad - w, ui.top - pad - w, w, w);
        let (r, _) = self.play_btn.render_shadow(ui, r, t, c.a, |_| semi_white(0.3 * c.a));
        let r = r.feather(-0.04);
        ui.fill_rect(
            r,
            (
                if self.charts.get(self.cur_chart).map_or(true, |it| it.local_index.is_some()) {
                    *self.icon_play
                } else {
                    *self.icon_download
                },
                r,
                ScaleType::Fit,
                c,
            ),
        );

        if let Some(dl) = &self.downloading {
            ui.fill_rect(ui.screen_rect(), semi_black(0.6));
            ui.loading(0., -0.06, t, WHITE, (*dl.prog.lock().unwrap(), &mut self.loading_last));
            ui.text(dl.status.lock().unwrap().clone()).pos(0., 0.02).anchor(0.5, 0.).size(0.6).draw();
            let size = 0.7;
            let r = ui.text(tl!("dl-cancel")).pos(0., 0.12).anchor(0.5, 0.).size(size).measure().feather(0.02);
            self.cancel_download_btn.render_text(ui, r, t, 1., tl!("dl-cancel"), 0.6, true);
        }

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        if let Some(scene) = self.next_scene.take() {
            if let Some(music) = &mut self.preview {
                let _ = music.pause();
            }
            scene
        } else {
            NextScene::default()
        }
    }
}
