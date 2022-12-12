use anyhow::{Result, bail};
use macroquad::prelude::*;
use once_cell::sync::Lazy;
use prpr::{build_conf, fs, Prpr};
use std::{sync::{mpsc, Mutex}, collections::HashMap};

#[cfg(not(target_os = "android"))]
compile_error!("Only supports android build");

static MESSAGES_TX: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static CHART_PATH: Mutex<Option<String>> = Mutex::new(None);
static OVERRIDES: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

async fn the_main() -> Result<()> {
    set_pc_assets_folder("assets");

    let path = CHART_PATH.lock().unwrap().clone().unwrap();

    let fs = if let Some(name) = path.strip_prefix(':') {
        fs::fs_from_assets(name)?
    } else {
        fs::fs_from_file(&path)?
    };

    let (mut config, fs) = fs::load_config(fs).await?;

    for (key, value) in OVERRIDES.lock().unwrap().iter() {
        // TODO simplify
        match key.as_str() {
            "adjustTime" => {
                config.adjust_time = value.parse()?;
            }
            "autoplay" => {
                config.autoplay = value.parse()?;
            }
            "multipleHint" => {
                config.multiple_hint = value.parse()?;
            }
            "speed" => {
                config.speed = value.parse()?;
            }

            "aggressive" => {
                config.aggressive = value.parse()?;
            }
            "particle" => {
                config.particle = value.parse()?;
            }

            "volumeMusic" => {
                config.volume_music = value.parse()?;
            }
            "volumeSfx" => {
                config.volume_sfx = value.parse()?;
            }
            _ => {
                bail!("Unknown config key: {key}");
            }
        }
    }

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
    unsafe { get_internal_gl() }.quad_context.order_quit();
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

unsafe fn string_from_java(env: *mut ndk_sys::JNIEnv, s: ndk_sys::jstring) -> String {
    let get_string_utf_chars = (**env).GetStringUTFChars.unwrap();
    let release_string_utf_chars = (**env).ReleaseStringUTFChars.unwrap();

    let ptr = (get_string_utf_chars)(env, s, ::std::ptr::null::<ndk_sys::jboolean>() as _);
    let res = std::ffi::CStr::from_ptr(ptr).to_str().unwrap().to_owned();
    (release_string_utf_chars)(env, s, ptr);

    res
}

#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnPause(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
) {
    let _ = MESSAGES_TX.lock().unwrap().as_mut().unwrap().send(());
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_setChartPath(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
    path: ndk_sys::jstring,
) {
    let env = crate::miniquad::native::attach_jni_env();
    *CHART_PATH.lock().unwrap() = Some(string_from_java(env, path));
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_clearOverrides(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
) {
    OVERRIDES.lock().unwrap().clear();
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_addOverride(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
    key: ndk_sys::jstring,
    value: ndk_sys::jstring,
) {
    let env = crate::miniquad::native::attach_jni_env();
    let key = string_from_java(env, key);
    let value = string_from_java(env, value);

    OVERRIDES.lock().unwrap().insert(key, value);
}
