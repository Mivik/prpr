use anyhow::{Context, Result};
use core::slice;
use macroquad::prelude::*;
use prpr::{
    build_conf,
    config::Config,
    core::DPI_VALUE,
    fs::{self, ZipFileSystem},
    ext::poll_future,
    scene::LoadingScene,
    time::TimeManager,
    Main,
};
use std::{
    ffi::CString,
    sync::{mpsc, Mutex},
};

#[cfg(not(target_os = "android"))]
compile_error!("Only supports android build");

static MESSAGES_TX: Mutex<Option<mpsc::Sender<bool>>> = Mutex::new(None);
static CHART_PATH: Mutex<Option<String>> = Mutex::new(None);
static CONFIG: Mutex<Option<Config>> = Mutex::new(None);

async fn the_main() -> Result<()> {
    set_pc_assets_folder("assets");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    let _ = prpr::ui::FONT.set(load_ttf_font("font.ttf").await?);

    let path = CHART_PATH.lock().unwrap().clone().unwrap();

    let fs = if let Some(name) = path.strip_prefix(':') {
        fs::fs_from_assets(name)?
    } else {
        fs::fs_from_file(&std::path::Path::new(&path))?
    };

    let (info, fs) = fs::load_info(fs).await?;

    let config = CONFIG.lock().unwrap().take().unwrap_or_default();

    let rx = {
        let (tx, rx) = mpsc::channel();
        *MESSAGES_TX.lock().unwrap() = Some(tx);
        rx
    };

    let mut fps_time = -1;

    let tm = TimeManager::default();
    let ctm = TimeManager::from_config(&config); // strange variable name...
    let mut main = Main::new(Box::new(LoadingScene::new(info, config, fs, None).await?), ctm, None)?;
    'app: loop {
        let frame_start = tm.real_time();
        main.update()?;
        main.render()?;
        if let Ok(paused) = rx.try_recv() {
            if paused {
                main.pause()?;
            } else {
                main.resume()?;
            }
        }
        if main.should_exit() {
            break 'app;
        }

        let t = tm.real_time();
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
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnPause(_: *mut std::ffi::c_void, _: *const std::ffi::c_void) {
    let _ = MESSAGES_TX.lock().unwrap().as_mut().unwrap().send(true);
}

#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnResume(_: *mut std::ffi::c_void, _: *const std::ffi::c_void) {
    if let Some(tx) = MESSAGES_TX.lock().unwrap().as_mut() {
        let _ = tx.send(false);
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_setChartPath(_: *mut std::ffi::c_void, _: *const std::ffi::c_void, path: ndk_sys::jstring) {
    let env = crate::miniquad::native::attach_jni_env();
    *CHART_PATH.lock().unwrap() = Some(string_from_java(env, path));
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_setConfig(_: *mut std::ffi::c_void, _: *const std::ffi::c_void, json: ndk_sys::jstring) {
    let env = crate::miniquad::native::attach_jni_env();
    let json = string_from_java(env, json);
    *CONFIG.lock().unwrap() = Some(serde_json::from_str(&json).unwrap());
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_setDpi(_: *mut std::ffi::c_void, _: *const std::ffi::c_void, dpi: ndk_sys::jint) {
    DPI_VALUE.store(dpi as _, std::sync::atomic::Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_getChartName(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
    bytes: ndk_sys::jobject,
) -> ndk_sys::jstring {
    let env = crate::miniquad::native::attach_jni_env();
    let get_array_length = (**env).GetArrayLength.unwrap();
    let get_byte_array_elements = (**env).GetByteArrayElements.unwrap();
    let new_string_utf = (**env).NewStringUTF.unwrap();
    let release_byte_array_elements = (**env).ReleaseByteArrayElements.unwrap();

    let len = (get_array_length)(env, bytes);
    let array = (get_byte_array_elements)(env, bytes, ::std::ptr::null::<ndk_sys::jboolean>() as _);
    let vec = slice::from_raw_parts_mut::<u8>(std::mem::transmute(array), len as _).to_vec();
    (release_byte_array_elements)(env, bytes, array, ndk_sys::JNI_ABORT as _);

    fn get_name(vec: Vec<u8>) -> Result<String> {
        let mut future = Box::pin(tokio::spawn(fs::load_info(Box::new(ZipFileSystem::new(vec).context("Failed to load the zip")?))));
        loop {
            if let Some(info) = poll_future(future.as_mut()) {
                break Ok(info??.0.name);
            }
            std::thread::yield_now();
        }
    }
    let res = match get_name(vec) {
        Err(err) => {
            format!("E{:?}", err)
        }
        Ok(name) => format!("O{name}"),
    };
    let cstr = CString::new(res).unwrap();
    (new_string_utf)(env, cstr.as_ptr())
}
