use anyhow::{bail, Context, Result};
use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{PlaybackState, StaticSoundData, StaticSoundSettings},
    tween::Tween,
    Volume,
};
use macroquad::prelude::*;
use prpr::{
    core::{Resource, ASPECT_RATIO},
    parse::{parse_phigros, parse_rpe},
};
use std::{io::Cursor, time::Instant};

fn build_conf() -> Conf {
    Conf {
        window_title: "prpr".to_string(),
        window_width: 1080,
        window_height: 608,
        ..Default::default()
    }
}

fn viewport() -> (i32, i32, i32, i32) {
    let (w, h) = (screen_width(), screen_height());
    let (rw, rh) = {
        let ew = h * ASPECT_RATIO;
        if ew > w {
            let eh = w / ASPECT_RATIO;
            (w, eh)
        } else {
            (ew, h)
        }
    };
    (
        ((w - rw) / 2.).round() as i32,
        ((h - rh) / 2.).round() as i32,
        rw as i32,
        rh as i32,
    )
}

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    set_pc_assets_folder("assets");
    let mut args = std::env::args();
    let program = args.next().unwrap();
    let Some(name) = args.next() else {
        bail!("Usage: {program} [Chart Name] (rpe | pgr)");
    };

    let text = String::from_utf8(load_file(&format!("charts/{name}/chart.json")).await?)?;
    let format = args.next().unwrap_or("rpe".to_string());
    let mut chart = match format.as_ref() {
        "rpe" => parse_rpe(&text).await?,
        "pgr" => parse_phigros(&text)?,
        _ => {
            bail!("Unknown chart format: {format}")
        }
    };

    let mut res = Resource::new().await.context("Failed to load resources")?;

    let mut manager = AudioManager::<CpalBackend>::new(AudioManagerSettings::default())?;
    let raw_sound_data = load_file(&format!("charts/{name}/song.mp3")).await?;
    let sound_data = StaticSoundData::from_cursor(
        Cursor::new(raw_sound_data),
        StaticSoundSettings::default().volume(Volume::Decibels(-20.0)),
    )?;
    let mut handle = manager.play(sound_data.clone())?;
    handle.pause(Tween::default())?;

    res.camera = Camera2D {
        target: vec2(0., 0.),
        zoom: vec2(1., ASPECT_RATIO),
        ..Default::default()
    };
    res.camera_matrix = res.camera.matrix();
    let mut last_time = 0.0;

    let mut fps_time = -1;
    let mut fps_last = 0;
    let gl = unsafe { get_internal_gl() }.quad_gl;
    loop {
        let frame_start = Instant::now();
        clear_background(Color::from_rgba(0x15, 0x65, 0xc0, 0xff));
        let now_time = handle.position() as f32;
        last_time = (last_time * 2. + now_time) / 3.;
        res.time = (last_time - chart.offset).max(0.0);
        chart.set_time(res.time);

        let vp = viewport();
        if Some(vp) != res.camera.viewport {
            res.camera.viewport = Some(vp);
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
        chart.render(&res);

        push_camera_state();
        set_default_camera();
        draw_text(&format!("{:.2}", res.time), 10., 25., 30., WHITE);

        let fps_now = get_time() as i32;
        if fps_now != fps_time {
            fps_time = fps_now;
            fps_last = (1. / frame_start.elapsed().as_secs_f64()) as u32;
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
            handle.seek_by(-1.0)?;
        }
        if is_key_pressed(KeyCode::Right) {
            handle.seek_by(1.0)?;
        }

        next_frame().await;
    }
}
