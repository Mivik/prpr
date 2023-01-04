use std::ops::DerefMut;

use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{build_conf, core::init_assets, fs, scene::LoadingScene, time::TimeManager, ui::Ui, Main};

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    init_assets();

    #[cfg(target_arch = "wasm32")]
    let (mut fs, config) = {
        fn js_err(err: wasm_bindgen::JsValue) -> anyhow::Error {
            anyhow::Error::msg(format!("{err:?}"))
        }
        let params = web_sys::UrlSearchParams::new_with_str(&web_sys::window().unwrap().location().search().map_err(js_err)?).map_err(js_err)?;
        let name = params.get("chart").unwrap_or_else(|| "nc".to_string());
        (
            fs::fs_from_assets(format!("charts/{name}/"))?,
            Some(prpr::config::Config {
                autoplay: false,
                ..Default::default()
            }),
        )
    };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let (mut fs, config) = (fs::fs_from_assets("charts/moment/")?, None);
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android"), not(target_os = "ios")))]
    let (mut fs, config) = {
        let mut args = std::env::args();
        let program = args.next().unwrap();
        let Some(path) = args.next() else {
            anyhow::bail!("Usage: {program} <chart>");
        };
        let mut config = None;
        if let Some(config_path) = args.next() {
            config = Some(serde_yaml::from_str(&std::fs::read_to_string(config_path).context("Cannot read from config file")?)?);
        }
        (fs::fs_from_file(std::path::Path::new(&path))?, config)
    };

    let _guard = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .unwrap();
            let rt = Box::leak(Box::new(rt));
            rt.enter()
        }
        #[cfg(target_arch = "wasm32")]
        {
            ()
        }
    };

    let _ = prpr::ui::FONT.set(load_ttf_font("font.ttf").await?);

    let info = fs::load_info(fs.deref_mut()).await?;
    let config = config.unwrap_or_default();

    let mut fps_time = -1;

    let tm = TimeManager::default();
    let ctm = TimeManager::from_config(&config); // strange variable name...
    let mut main = Main::new(Box::new(LoadingScene::new(info, config, fs, None, None).await?), ctm, None)?;
    'app: loop {
        let frame_start = tm.real_time();
        main.update()?;
        main.render(&mut Ui::new())?;
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
    Ok(())
}
