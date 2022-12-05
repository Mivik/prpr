use anyhow::{bail, Context, Result};
use image::imageops::blur;
use kira::{
    sound::static_sound::{PlaybackState, StaticSoundData, StaticSoundSettings},
    tween::Tween,
};
use macroquad::prelude::*;
use prpr::{
    core::Resource,
    parse::{parse_pec, parse_phigros, parse_rpe},
};
use std::io::Cursor;

fn build_conf() -> Conf {
    Conf {
        window_title: "prpr".to_string(),
        window_width: 1080,
        window_height: 608,
        ..Default::default()
    }
}

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    set_pc_assets_folder("assets");
    #[cfg(target_arch = "wasm32")]
    let mut args = ["prpr", "nc"].map(str::to_owned).into_iter();
    #[cfg(not(target_arch = "wasm32"))]
    let mut args = std::env::args();

    let program = args.next().unwrap();
    let Some(name) = args.next() else {
        bail!("Usage: {program} [Chart Name] (rpe | pgr)");
    };

    let text = String::from_utf8(load_file(&format!("charts/{name}/chart.json")).await?)?;
    let format = args.next().unwrap_or_else(|| "rpe".to_string());
    let mut chart = match format.as_ref() {
        "rpe" => parse_rpe(&text).await?,
        "pgr" => parse_phigros(&text)?,
        "pec" => parse_pec(&text)?,
        _ => {
            bail!("Unknown chart format: {format}")
        }
    };

    let mut res = Resource::new().await.context("Failed to load resources")?;

    let raw_sound_data = load_file(&format!("charts/{name}/song.mp3")).await?;
    let sound_data =
        StaticSoundData::from_cursor(Cursor::new(raw_sound_data), StaticSoundSettings::default())?;
    let mut handle = res.audio_manager.play(sound_data.clone())?;
    handle.pause(Tween::default())?;

    async fn load_background(path: &str) -> Result<Texture2D> {
        let image =
            image::load_from_memory(&load_file(path).await?).context("Failed to decode image")?;
        let image = blur(&image, 15.);
        Ok(Texture2D::from_image(&Image {
            width: image.width() as u16,
            height: image.height() as u16,
            bytes: image.into_raw(),
        }))
    }

    let background = load_background(&format!("charts/{name}/background.png"))
        .await
        .unwrap_or_else(|err| {
            warn!("Failed to load background\n{:?}", err);
            Texture2D::from_rgba8(1, 1, &[0, 0, 0, 1])
        });

    let mut fps_time = -1;
    let mut fps_last = 0;
    let gl = unsafe { get_internal_gl() }.quad_gl;
    loop {
        let frame_start = get_time();
        push_camera_state();
        set_default_camera();
        {
            let sw = screen_width();
            let sh = screen_height();
            let bw = background.width();
            let bh = background.height();
            let s = (sw / bw).max(sh / bh);
            draw_texture_ex(
                background,
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

        let time = (handle.position() as f32 - chart.offset).max(0.0);
        res.set_real_time(time);
        chart.update(&mut res);

        if res.update_size() {
            set_camera(&res.camera);
        }
        gl.viewport(res.camera.viewport);
        draw_rectangle(-1., -1., 2., 2., Color::new(0., 0., 0., 0.6));
        chart.render(&mut res);
        let delta = get_frame_time();
        res.emitter.draw(vec2(0., 0.), delta);
        res.emitter_square.draw(vec2(0., 0.), delta);

        push_camera_state();
        set_default_camera();
        draw_text(&format!("{:.2}", res.time), 10., 25., 30., WHITE);

        let fps_now = get_time() as i32;
        if fps_now != fps_time {
            fps_time = fps_now;
            fps_last = (1. / (get_time() - frame_start)) as u32;
        }
        draw_text(
            &format!("FPS: {} (60)", fps_last),
            10.,
            screen_height() - 10.,
            30.,
            WHITE,
        );
        pop_camera_state();

        if is_key_pressed(KeyCode::Space) {
            if matches!(handle.state(), PlaybackState::Paused) {
                handle.resume(Tween::default())?;
            } else {
                handle.pause(Tween::default())?;
            }
        }
        if is_key_pressed(KeyCode::Left) {
            res.time -= 1.;
            handle.seek_by(-1.0)?;
        }
        if is_key_pressed(KeyCode::Right) {
            res.time += 1.;
            handle.seek_by(1.0)?;
        }
        if is_key_pressed(KeyCode::Q) {
            break;
        }

        next_frame().await;
    }
    Ok(())
}
