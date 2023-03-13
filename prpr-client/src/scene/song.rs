prpr::tl_file!("song");

use super::fs_from_path;
use crate::{
    get_data,
    page::ChartItem,
    phizone::{Client, PZChart, PZSong, Ptr},
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{screen_aspect, semi_black, semi_white, RectExt, SafeTexture, ScaleType},
    fs,
    scene::{show_error, GameMode, LoadingScene, NextScene, Scene},
    task::Task,
    time::TimeManager,
    ui::{button_hit, DRectButton, RectButton, Scroll, Ui, UI_AUDIO},
};
use sasa::{AudioClip, Music, MusicParams};
use serde_json::json;
use std::ops::DerefMut;

const FADE_IN_TIME: f32 = 0.3;

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

pub struct SongScene {
    chart: ChartItem,

    first_in: bool,

    back_btn: RectButton,
    play_btn: DRectButton,

    icon_back: SafeTexture,
    icon_play: SafeTexture,

    next_scene: Option<NextScene>,

    preview: Option<Music>,
    preview_task: Option<Task<Result<AudioClip>>>,

    charts: Option<Vec<PZChart>>,
    charts_task: Option<Task<Result<Vec<PZChart>>>>,
    charts_scroll: Scroll,
}

impl SongScene {
    pub fn new(chart: ChartItem, icon_back: SafeTexture, icon_play: SafeTexture) -> Self {
        let song_ptr = chart.info.id.map(|it| Ptr::<PZSong>::from_id(it.1));
        let (charts, charts_task) = if let &Some((_, song_id)) = &chart.info.id {
            (None, Some(Task::new(async move { Client::query::<PZChart>().query("song", song_id.to_string()).send().await.map(|it| it.0) })))
        } else {
            (Some(Vec::new()), None)
        };
        Self {
            chart,

            first_in: true,

            back_btn: RectButton::new(),
            play_btn: DRectButton::new(),

            icon_back,
            icon_play,

            next_scene: None,

            preview: None,
            preview_task: Some(Task::new(async move {
                if let Some(song) = song_ptr {
                    let song = song.load().await?;
                    if let Some(preview) = &song.preview {
                        with_effects(preview.fetch().await?.to_vec(), None)
                    } else {
                        with_effects(song.music.fetch().await?.to_vec(), Some((song.preview_start.seconds, song.preview_end.seconds)))
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
            charts_task,
            charts_scroll: Scroll::new(),
        }
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
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        if self.back_btn.touch(touch) {
            button_hit();
            self.next_scene = Some(NextScene::PopWithResult(Box::new(())));
            return Ok(true);
        }
        if self.play_btn.touch(touch, t) {
            // LoadingScene::new(GameMode::Normal, self.chart.info.clone(), get_data().config.clone(), todo!(), None, None, None);
            return Ok(true);
        }
        if self.charts_scroll.touch(touch, t) {
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        self.chart.settle();
        if let Some(task) = &mut self.charts_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => show_error(err.context(tl!("load-charts-failed"))),
                    Ok(charts) => {
                        self.charts = Some(charts);
                    }
                }
                self.charts_task = None;
            }
        }
        self.charts_scroll.update(t);
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            ..Default::default()
        });
        let t = tm.now() as f32;
        ui.fill_rect(ui.screen_rect(), (*self.chart.illustration.1, ui.screen_rect()));
        ui.fill_rect(ui.screen_rect(), semi_black(0.55));

        let c = semi_white((t / FADE_IN_TIME).clamp(-1., 0.) + 1.);

        let r = ui.back_rect();
        self.back_btn.set(ui, r);
        ui.fill_rect(r, (*self.icon_back, r, ScaleType::Fit, c));

        let r = ui
            .text(&self.chart.info.name)
            .size(1.2)
            .pos(r.right() + 0.02, r.bottom() - 0.06)
            .color(c)
            .draw();
        ui.text(&self.chart.info.composer)
            .size(0.5)
            .pos(r.x + 0.02, r.bottom() + 0.03)
            .color(Color { a: c.a * 0.8, ..c })
            .draw();

        // charts slide
        let hh = 0.35;
        let item_h = 0.1;
        let w = 0.5;
        let r = Rect::new(-1., -hh, w, hh * 2.);
        self.charts_scroll.size((r.w, r.h));
        if let Some(charts) = &self.charts {
            ui.scope(|ui| {
                ui.dx(r.x);
                ui.dy(r.y);
                let mut oy = self.charts_scroll.y_scroller.offset();
                self.charts_scroll.render(ui, |ui| {
                    let mut h = hh;
                    ui.dy(hh);
                    for chart in charts {
                        ui.text(format!("{} Lv.{}", chart.level, chart.difficulty as u32))
                            .pos(0.1 - (oy * oy) / (hh * 1.5).powi(2) * 0.5, 0.)
                            .size(1.2 - (oy * oy / (0.23 * 0.23)).min(1.) * 0.6)
                            .anchor(0., 0.5)
                            .no_baseline()
                            .color(Color {
                                a: c.a * (1. - (oy * oy) / (hh * hh)).max(0.),
                                ..c
                            })
                            .draw();
                        ui.dy(item_h);
                        h += item_h;
                        oy -= item_h;
                    }
                    h += hh - item_h;
                    (r.w, h)
                });
            });
        }

        // bottom bar
        let h = 0.16;
        let r = Rect::new(-1., ui.top - h, 1.7, h);
        ui.fill_rect(r, (Color::from_hex(0xff283593), (r.x, r.y), Color::default(), (r.right(), r.y)));

        // play button
        let w = 0.26;
        let pad = 0.08;
        let r = Rect::new(1. - pad - w, ui.top - pad - w, w, w);
        let (r, _) = self
            .play_btn
            .render_shadow(ui, r, t, c.a, |r| (Color::from_hex(0xff303f9f), (r.x, r.y), Color::from_hex(0xff1976d2), (r.right(), r.bottom())));
        let r = r.feather(-0.04);
        ui.fill_rect(r, (*self.icon_play, r, ScaleType::Fit, c));

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
