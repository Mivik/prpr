use anyhow::Result;
use macroquad::prelude::*;
use prpr::{build_conf, config::Config, Prpr};

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    set_pc_assets_folder("assets");

    #[cfg(target_arch = "wasm32")]
    let name = {
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
        params.get("chart").unwrap_or_else(|| "nc".to_string())
    };
    #[cfg(target_os = "android")]
    let name = "strife".to_string();
    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
    let name = {
        let mut args = std::env::args();
        let program = args.next().unwrap();
        let Some(name) = args.next() else {
            anyhow::bail!("Usage: {program} <chart name>");
        };
        name
    };

    let mut config: Config = serde_yaml::from_str(&String::from_utf8(
        load_file(&format!("charts/{name}/info.yml")).await?,
    )?)?;
    config.id = name.clone();

    let mut fps_time = -1;

    let mut prpr = Prpr::new(config, None).await?;
    'app: loop {
        let frame_start = prpr.get_time();
        prpr.update(None);
        prpr.render(None)?;
        prpr.ui(true)?;
        prpr.process_keys()?;
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
