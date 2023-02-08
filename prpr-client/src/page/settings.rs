prpr::tl_file!("settings");

use super::{Page, SharedState};
use crate::{dir, get_data, get_data_mut, save_data, sync_lang};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{
    core::{ParticleEmitter, ResourcePack, JUDGE_LINE_PERFECT_COLOR, NOTE_WIDTH_RATIO_BASE},
    ext::{create_audio_manger, poll_future, LocalTask, RectExt, SafeTexture},
    scene::{request_file, return_file, show_error, show_message, take_file},
    time::TimeManager,
    ui::{RectButton, Ui},
};
use sasa::{AudioClip, AudioManager, Music, MusicParams, PlaySfxParams, Sfx};
use std::borrow::Cow;

const RESET_WAIT: f32 = 0.8;

pub struct SettingsPage {
    focus: bool,

    audio: AudioManager,
    cali: Music,
    cali_hit: Sfx,
    cali_tm: TimeManager,
    cali_last: bool,
    click_texture: SafeTexture,
    emitter: ParticleEmitter,
    res_pack: ResourcePack, // prevent resource pack textures from being destroyed (ParticleEmitter holds a `weak` reference)

    chal_buttons: [RectButton; 6],

    load_res_task: LocalTask<Result<(ResourcePack, Option<String>)>>,
    reset_time: f32,
}

impl SettingsPage {
    pub async fn new() -> Result<Self> {
        let mut audio = create_audio_manger(&get_data().config)?;
        let cali = audio.create_music(
            AudioClip::new(load_file("cali.ogg").await?)?,
            MusicParams {
                loop_: true,
                amplifier: 0.7,
                ..Default::default()
            },
        )?;
        let cali_hit = audio.create_sfx(AudioClip::new(load_file("cali_hit.ogg").await?)?, Some(2))?;

        let mut cali_tm = TimeManager::new(1., true);
        cali_tm.force = 3e-2;
        let res_pack = ResourcePack::from_path(
            get_data()
                .config
                .res_pack_path
                .as_ref()
                .map(|it| format!("{}/{it}", dir::root().unwrap())),
        )
        .await?;
        let emitter = ParticleEmitter::new(&res_pack, get_data().config.note_scale, res_pack.info.hide_particles)?;
        Ok(Self {
            focus: false,

            audio,
            cali,
            cali_hit,
            cali_tm,
            cali_last: false,
            click_texture: res_pack.note_style.click.clone(),
            emitter,
            res_pack,

            chal_buttons: [RectButton::new(); 6],

            load_res_task: None,
            reset_time: f32::NEG_INFINITY,
        })
    }

    fn new_res_task(path: Option<String>) -> LocalTask<Result<(ResourcePack, Option<String>)>> {
        Some(Box::pin(async move {
            let res_pack = ResourcePack::from_path(path.as_ref()).await?;
            Ok((
                res_pack,
                if let Some(path) = path {
                    let dst = format!("{}/respack.zip", dir::root()?);
                    std::fs::copy(path, dst).with_context(|| tl!("respack-save-failed"))?;
                    Some("respack.zip".to_owned())
                } else {
                    None
                },
            ))
        }))
    }
}

impl Page for SettingsPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn update(&mut self, focus: bool, state: &mut SharedState) -> Result<()> {
        self.audio.recover_if_needed()?;
        let t = state.t;
        if !self.focus && focus {
            self.cali.seek_to(0.)?;
            self.cali.play()?;
            self.cali_tm.reset();
        }
        if self.focus && !focus {
            save_data()?;
            self.cali.pause()?;
        }
        self.focus = focus;

        if !self.cali.paused() {
            let pos = self.cali.position() as f64;
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
        if let Some(future) = &mut self.load_res_task {
            if let Some(result) = poll_future(future.as_mut()) {
                self.load_res_task = None;
                match result {
                    Err(err) => {
                        show_error(err.context(tl!("respack-load-failed")));
                    }
                    Ok((res_pack, dst)) => {
                        self.click_texture = res_pack.note_style.click.clone();
                        self.emitter = ParticleEmitter::new(&res_pack, get_data().config.note_scale, res_pack.info.hide_particles)?;
                        self.res_pack = res_pack;
                        get_data_mut().config.res_pack_path = dst;
                        save_data()?;
                        show_message(tl!("respack-loaded"));
                    }
                }
            }
        }
        if let Some((id, file)) = take_file() {
            if id == "res_pack" {
                self.load_res_task = Self::new_res_task(Some(file));
            } else {
                return_file(id, file);
            }
        }
        if t > self.reset_time + RESET_WAIT {
            self.reset_time = f32::NEG_INFINITY;
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, _state: &mut SharedState) -> Result<bool> {
        for (id, button) in self.chal_buttons.iter_mut().enumerate() {
            if button.touch(touch) {
                use prpr::config::ChallengeModeColor::*;
                get_data_mut().config.challenge_color = [White, Green, Blue, Red, Golden, Rainbow][id].clone();
                save_data()?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        let t = state.t;
        let config = &mut get_data_mut().config;
        let s = 0.01;
        ui.scope(|ui| {
            ui.dy(0.01);
            ui.dx(0.02);
            ui.scope(|ui| {
                let s = 0.005;
                let r = ui.checkbox(tl!("autoplay"), &mut config.autoplay);
                ui.dy(r.h + s);
                let r = ui.checkbox(tl!("double-tips"), &mut config.multiple_hint);
                ui.dy(r.h + s);
                let r = ui.checkbox(tl!("fixed-aspect-ratio"), &mut config.fix_aspect_ratio);
                ui.dy(r.h + s);
                let r = ui.checkbox(tl!("time-adjustment"), &mut config.adjust_time);
                ui.dy(r.h + s);
                let r = ui.checkbox(tl!("particles"), &mut config.particle);
                ui.dy(r.h + s);
                let r = ui.checkbox(tl!("aggressive-opt"), &mut config.aggressive);
                ui.dy(r.h + s);
                let mut low = config.sample_count == 1;
                let r = ui.checkbox(tl!("low-perf-mode"), &mut low);
                config.sample_count = if low { 1 } else { 2 };
                ui.dy(r.h + s);
                let r = ui.slider(tl!("player-rks"), 1.0..17.0, 0.01, &mut config.player_rks, Some(0.45));
                ui.dy(r.h + s);
            });
            ui.dx(0.62);

            ui.scope(|ui| {
                let r = ui.slider(tl!("offset"), -0.5..0.5, 0.005, &mut config.offset, None);
                ui.dy(r.h + s);
                let r = ui.slider(tl!("speed"), 0.5..2.0, 0.005, &mut config.speed, None);
                ui.dy(r.h + s);
                let r = ui.slider(tl!("note-size"), 0.8..1.2, 0.005, &mut config.note_scale, None);
                self.emitter.set_scale(config.note_scale);
                ui.dy(r.h + s);
                let r = ui.slider(tl!("music-vol"), 0.0..2.0, 0.05, &mut config.volume_music, None);
                ui.dy(r.h + s);
                let r = ui.slider(tl!("sfx-vol"), 0.0..2.0, 0.05, &mut config.volume_sfx, None);
                ui.dy(r.h + s);
                let r = ui.text(tl!("chal-color")).size(0.4).draw();
                let chosen = config.challenge_color.clone() as usize;
                ui.dy(r.h + s * 2.);
                let dy = ui.scope(|ui| {
                    let mut max: f32 = 0.;
                    for (id, (name, button)) in tl!("chal-colors").split(',').zip(self.chal_buttons.iter_mut()).enumerate() {
                        let r = ui.text(name).size(0.4).measure().feather(0.01);
                        button.set(ui, r);
                        ui.fill_rect(r, if chosen == id { ui.accent() } else { WHITE });
                        let color = if chosen == id { WHITE } else { ui.accent() };
                        ui.text(name).size(0.4).color(color).draw();
                        ui.dx(r.w + s);
                        max = max.max(r.h);
                    }
                    max
                });
                ui.dy(dy + s);

                let mut rks = config.challenge_rank as f32;
                let r = ui.slider(tl!("chal-level"), 0.0..48.0, 1., &mut rks, Some(0.45));
                config.challenge_rank = rks.round() as u32;
                ui.dy(r.h + s);
            });

            ui.scope(|ui| {
                ui.dx(0.65);
                let r = ui.checkbox(tl!("double-click-pause"), &mut config.double_click_to_pause);
                ui.dy(r.h + s);
                let r = ui.text(tl!("respack")).size(0.4).anchor(1., 0.).draw();
                let mut r = Rect::new(0.02, r.y - 0.01, 0.3, r.h + 0.02);
                if ui.button("choose_res_pack", r, &self.res_pack.info.name) {
                    request_file("res_pack");
                }
                r.x += 0.3 + 0.02;
                r.w = 0.1;
                if ui.button("reset_res_pack", r, tl!("reset")) {
                    self.load_res_task = Self::new_res_task(None);
                }
                ui.dy(r.h + s * 2.);
                r.x -= 0.3 + 0.02;
                r.w = 0.4;
                let label = tl!("audio-buffer");
                let default = tl!("default");
                let mut input = config.audio_buffer_size.map(|it| it.to_string()).unwrap_or_else(|| default.to_string());
                ui.input(label, &mut input, 0.3);
                if input.trim().is_empty() || input == default {
                    config.audio_buffer_size = None;
                } else {
                    match input.parse::<u32>() {
                        Err(_) => {
                            show_message(tl!("invalid-input"));
                        }
                        Ok(value) => {
                            config.audio_buffer_size = Some(value);
                        }
                    }
                }
                ui.dy(r.h + s * 2.);
                r.w = r.w * 1.3 / 2. - 0.01;
                // TODO refine this
                let text = tl!("switch-language");
                if ui.button("switch_lang", r, text.as_ref()) {
                    if text == "中文" {
                        get_data_mut().language = Some("zh-CN".to_owned());
                    } else {
                        get_data_mut().language = Some("en-US".to_owned());
                    }
                    let _ = save_data();
                    sync_lang();
                }
                r.x += r.w + 0.01;
                if ui.button(
                    "reset_all",
                    r,
                    if self.reset_time.is_finite() {
                        tl!("confirm-reset")
                    } else {
                        tl!("reset-all")
                    },
                ) {
                    if self.reset_time.is_finite() {
                        self.reset_time = f32::NEG_INFINITY;
                        *config = prpr::config::Config::default();
                        if let Err(err) = save_data() {
                            show_error(err.context("保存失败"));
                        } else {
                            self.load_res_task = Self::new_res_task(None);
                            show_message("设定恢复成功");
                        }
                    } else {
                        self.reset_time = t;
                    }
                }
            });

            let ct = (0.9, ui.top * 1.5);
            let len = 0.25;
            ui.fill_rect(Rect::new(ct.0 - len, ct.1 - 0.005, len * 2., 0.01), WHITE);
            let mut cali_t = self.cali_tm.now() as f32 - config.offset;
            if cali_t < 0. {
                cali_t += 2.;
            }
            if cali_t >= 2. {
                cali_t -= 2.;
            }
            if cali_t <= 1. {
                let w = NOTE_WIDTH_RATIO_BASE * config.note_scale * 2.;
                let h = w * self.click_texture.height() / self.click_texture.width();
                let r = Rect::new(ct.0 - w / 2., ct.1 + (cali_t - 1.) * 0.4, w, h);
                ui.fill_rect(r, (*self.click_texture, r));
                self.cali_last = true;
            } else {
                if self.cali_last {
                    let g = ui.to_global(ct);
                    self.emitter.emit_at(vec2(g.0, g.1), 0., JUDGE_LINE_PERFECT_COLOR);
                    if self.focus {
                        let _ = self.cali_hit.play(PlaySfxParams::default());
                    }
                }
                self.cali_last = false;
            }
        });
        self.emitter.draw(get_frame_time());
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        save_data()?;
        if self.focus {
            self.cali_tm.pause();
            self.cali.pause()?;
        }
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        if self.focus {
            self.cali_tm.resume();
            self.cali.play()?;
        }
        Ok(())
    }
}
