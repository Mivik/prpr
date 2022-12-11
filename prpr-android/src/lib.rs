use anyhow::Result;
use macroquad::prelude::*;
use prpr::{build_conf, fs, Prpr};
use std::sync::{mpsc, Mutex};

#[cfg(not(target_os = "android"))]
compile_error!("Only supports android build");

static MESSAGES_TX: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);

async fn the_main() -> Result<()> {
    set_pc_assets_folder("assets");

    let name = "moment".to_string();

    let (config, fs) = fs::load_config(fs::fs_from_assets(&name)?).await?;

    let rx = {
        let (tx, rx) = mpsc::channel();
        *MESSAGES_TX.lock().unwrap() = Some(tx);
        rx
    };

    let mut fps_time = -1;

    let mut prpr = Prpr::new(config, fs, None).await?;
    'app: loop {
        let frame_start = prpr.get_time();
        prpr.update(None)?;
        prpr.render(None)?;
        prpr.ui(true)?;
        prpr.process_keys()?;
        if rx.try_recv().is_ok() {
            prpr.pause()?;
        }
        if prpr.should_exit {
            break 'app;
        }

        let t = prpr.get_time();
        let fps_now = t as i32;
        if fps_now != fps_time {
            fps_time = fps_now;
            info!("| {}", (1. / (t - frame_start)) as u32);
        }

        next_frame().await;
    }
    Ok(())
}

#[no_mangle]
pub extern "C" fn quad_main() {
    macroquad::Window::from_config(build_conf(), async {
        if let Err(err) = the_main().await {
            error!("Error: {:?}", err);
        }
    });
}

#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnPause(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
) {
    MESSAGES_TX
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .send(())
        .unwrap();
}
