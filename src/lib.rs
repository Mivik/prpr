mod ext;

pub mod audio;
pub mod config;
pub mod core;
pub mod judge;
pub mod parse;
pub mod particle;

use crate::{
    audio::{Audio, PlayParams},
    config::{ChartFormat, Config},
    core::{
        draw_text_aligned, Matrix, Resource, Vector, JUDGE_LINE_GOOD_COLOR,
        JUDGE_LINE_PERFECT_COLOR,
    },
    judge::Judge,
    parse::{parse_pec, parse_phigros, parse_rpe},
};
use anyhow::{bail, Context, Result};
use concat_string::concat_string;
use macroquad::prelude::*;

const ADJUST_TIME_THRESHOLD: f64 = 0.05;

pub fn build_conf() -> Conf {
    Conf {
        window_title: "prpr".to_string(),
        window_width: 1080,
        window_height: 608,
        ..Default::default()
    }
}

pub async fn the_main() -> Result<()> {
    set_pc_assets_folder("assets");
    simulate_mouse_with_touch(false);
    #[cfg(target_arch = "wasm32")]
    let mut args = {
        fn js_err(err: wasm_bindgen::JsValue) -> anyhow::Error {
            anyhow::Error::msg(format!("{err:?}"))
        }
        let params = web_sys::UrlSearchParams::new_with_str(
            &web_sys::window()
                .unwrap()
                .location()
                .search()
                .map_err(js_err)?,
        )
        .map_err(js_err)?;
        [
            "prpr".to_string(),
            params.get("chart").unwrap_or_else(|| "nc".to_string()),
        ]
        .into_iter()
    };
    #[cfg(target_os = "android")]
    let mut args = ["prpr", "strife"].map(str::to_owned).into_iter();
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
    let mut args = std::env::args();

    let program = args.next().unwrap();
    let Some(name) = args.next() else {
        bail!("Usage: {program} <chart name>");
    };

    let prefix = concat_string!("charts/", name, "/");

    let mut config: Config = serde_yaml::from_str(&String::from_utf8(
        load_file(&concat_string!(prefix, "info.yml")).await?,
    )?)?;
    config.id = name.clone();

    let text = String::from_utf8(load_file(&concat_string!(prefix, config.chart)).await?)?;
    let mut chart = match config.format {
        ChartFormat::Rpe => parse_rpe(&text).await?,
        ChartFormat::Pgr => parse_phigros(&text)?,
        ChartFormat::Pec => parse_pec(&text)?,
    };

    let mut res = Resource::new(config)
        .await
        .context("Failed to load resources")?;

    let mut fps_time = -1;

    let mut judge = Judge::new(&chart);

    let gl = unsafe { get_internal_gl() }.quad_gl;

    let mut handle = res.audio.play(
        &res.music,
        PlayParams {
            volume: res.config.volume_music,
            playback_rate: res.config.speed,
            ..Default::default()
        },
    )?;
    // res.audio.pause(&mut handle)?;

    // we use performance.now() on web since audioContext.currentTime is not stable
    // and may cause serious latency problem
    #[cfg(target_arch = "wasm32")]
    let get_time = {
        let perf = web_sys::window().unwrap().performance().unwrap();
        let speed = res.config.speed;
        move || perf.now() / 1000. * speed
    };
    #[cfg(not(target_arch = "wasm32"))]
    let get_time = {
        let start = std::time::Instant::now();
        let speed = res.config.speed;
        move || start.elapsed().as_secs_f64() * speed
    };
    let mut start_time = get_time();
    let mut pause_time = None;

    let mut bad_notes = Vec::new();
    loop {
        let frame_start = get_time();
        push_camera_state();
        set_default_camera();
        {
            let sw = screen_width();
            let sh = screen_height();
            let bw = res.background.width();
            let bh = res.background.height();
            let s = (sw / bw).max(sh / bh);
            draw_texture_ex(
                res.background,
                (sw - bw * s) / 2.,
                (sh - bh * s) / 2.,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(bw * s, bh * s)),
                    ..Default::default()
                },
            );
        }
        draw_rectangle(
            0.,
            0.,
            screen_width(),
            screen_height(),
            Color::new(0., 0., 0., 0.3),
        );
        pop_camera_state();

        let time = pause_time.unwrap_or_else(&get_time) - start_time;
        let music_time = res.audio.position(&handle)?;
        if !cfg!(target_arch = "wasm32") && (music_time - time).abs() > ADJUST_TIME_THRESHOLD {
            warn!(
                "Times differ a lot: {} {}. Syncing time...",
                time, music_time
            );
            start_time -= music_time - time;
        }

        let time = (time as f32 - chart.offset).max(0.0);
        if time > res.track_length + 0.8 {
            break;
        }
        res.time = time;
        judge.update(&mut res, &mut chart, &mut bad_notes);
        res.judge_line_color = if judge.counts[2] + judge.counts[3] == 0 {
            if judge.counts[1] == 0 {
                JUDGE_LINE_PERFECT_COLOR
            } else {
                JUDGE_LINE_GOOD_COLOR
            }
        } else {
            WHITE
        };
        chart.update(&mut res);

        if res.update_size() {
            set_camera(&res.camera);
        }
        gl.viewport(res.camera.viewport);
        draw_rectangle(-1., -1., 2., 2., Color::new(0., 0., 0., 0.6));
        chart.render(&mut res);
        bad_notes.retain(|dummy| dummy.render(&mut res));
        let delta = get_frame_time();
        if res.config.particle {
            res.emitter.draw(vec2(0., 0.), delta);
            res.emitter_square.draw(vec2(0., 0.), delta);
        }

        // UI overlay
        res.with_model(
            Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)),
            |res| {
                res.apply_model(|| {
                    let eps = 2e-2 / res.config.aspect_ratio;
                    let top = -1. / res.config.aspect_ratio;
                    let margin = 0.03;
                    draw_text_aligned(
                        res,
                        &format!("{:07}", judge.score()),
                        1. - margin,
                        top + eps * 2.8,
                        (1., 0.),
                        0.8,
                        WHITE,
                    );
                    if judge.combo >= 2 {
                        let rect = draw_text_aligned(
                            res,
                            &judge.combo.to_string(),
                            0.,
                            top + eps * 2.,
                            (0.5, 0.),
                            1.,
                            WHITE,
                        );
                        draw_text_aligned(
                            res,
                            if res.config.autoplay {
                                "AUTOPLAY"
                            } else {
                                "COMBO"
                            },
                            0.,
                            rect.y + eps * 1.5,
                            (0.5, 0.),
                            0.4,
                            WHITE,
                        );
                    }
                    let bar_width = 0.01;
                    let rect = draw_text_aligned(
                        res,
                        &res.config.title,
                        -1. + margin + bar_width + 0.01,
                        -top - eps * 2.8,
                        (0., 1.),
                        0.6,
                        WHITE,
                    );
                    draw_rectangle(-1. + margin, rect.y - 0.034, bar_width, 0.034, WHITE);
                    draw_text_aligned(
                        res,
                        &res.config.level,
                        1. - margin,
                        -top - eps * 2.8,
                        (1., 1.),
                        0.6,
                        WHITE,
                    );
                    let hw = 0.003;
                    let height = eps * 1.2;
                    let dest = 2. * res.time / res.track_length;
                    draw_rectangle(-1., top, dest, height, Color::new(1., 1., 1., 0.6));
                    draw_rectangle(-1. + dest - hw, top, hw * 2., height, WHITE);
                })
            },
        );

        let fps_now = get_time() as i32;
        if fps_now != fps_time {
            fps_time = fps_now;
            info!("| {}", (1. / (get_time() - frame_start)) as u32);
        }

        if is_key_pressed(KeyCode::Space) {
            if res.audio.paused(&handle)? {
                res.audio.resume(&mut handle)?;
                start_time += get_time() - pause_time.take().unwrap();
            } else {
                res.audio.pause(&mut handle)?;
                pause_time = Some(get_time());
            }
        }
        if is_key_pressed(KeyCode::Left) {
            res.time -= 1.;
            let dst = res.audio.position(&handle)? - 1.;
            res.audio.seek_to(&mut handle, dst)?;
            start_time = get_time() - dst;
        }
        if is_key_pressed(KeyCode::Right) {
            res.time += 1.;
            let dst = res.audio.position(&handle)? + 1.;
            res.audio.seek_to(&mut handle, dst)?;
            start_time = get_time() - dst;
        }
        if is_key_pressed(KeyCode::Q) {
            break;
        }

        next_frame().await;
    }
    Ok(())
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn quad_main() {
    macroquad::Window::from_config(build_conf(), async {
        if let Err(err) = the_main().await {
            error!("Error: {:?}", err);
        }
    });
}
