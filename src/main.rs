use anyhow::{bail, Context, Result};
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

    let mut fps_time = -1;
    let mut fps_last = 0;
    let gl = unsafe { get_internal_gl() }.quad_gl;
    loop {
        let frame_start = get_time();
        clear_background(Color::from_rgba(0x15, 0x65, 0xc0, 0xff));
        let time = (handle.position() as f32 - chart.offset).max(0.0);
        res.set_real_time(time);
        chart.update(&mut res);

        if res.update_size() {
            set_camera(&res.camera);
        }
        gl.viewport(res.camera.viewport);
        draw_rectangle(
            -1.0,
            -1.0,
            2.0,
            2.0,
            Color::from_rgba(0x21, 0x96, 0xf3, 0xff),
        );
        res.emitter.draw(vec2(0., 0.));
        res.emitter_square.draw(vec2(0., 0.));
        chart.render(&mut res);

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

        next_frame().await;
    }
}
