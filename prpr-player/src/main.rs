use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{
    build_conf,
    core::init_assets,
    fs,
    scene::{show_error, GameMode, LoadingScene, NextScene, Scene},
    time::TimeManager,
    ui::{FontArc, TextPainter, Ui},
    Main,
};
use std::ops::DerefMut;

struct BaseScene(Option<NextScene>, bool);
impl Scene for BaseScene {
    fn on_result(&mut self, _tm: &mut TimeManager, result: Box<dyn std::any::Any>) -> Result<()> {
        show_error(result.downcast::<anyhow::Error>().unwrap().context("加载谱面失败"));
        self.1 = true;
        Ok(())
    }
    fn enter(&mut self, _tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.0.is_none() && !self.1 {
            self.0 = Some(NextScene::Exit);
        }
        Ok(())
    }
    fn update(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn render(&mut self, _tm: &mut TimeManager, _ui: &mut Ui) -> Result<()> {
        Ok(())
    }
    fn next_scene(&mut self, _tm: &mut TimeManager) -> prpr::scene::NextScene {
        self.0.take().unwrap_or_default()
    }
}

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

    let font = FontArc::try_from_vec(load_file("font.ttf").await?)?;
    let mut painter = TextPainter::new(font);

    let info = fs::load_info(fs.deref_mut()).await?;
    let config = config.unwrap_or_default();

    let mut fps_time = -1;

    let tm = TimeManager::default();
    let ctm = TimeManager::from_config(&config); // strange variable name...
    let mut main = Main::new(
        Box::new(BaseScene(
            Some(NextScene::Overlay(Box::new(LoadingScene::new(GameMode::Normal, info, config, fs, (None, None), None, None).await?))),
            false,
        )),
        ctm,
        None,
    )
    .await?;
    'app: loop {
        let frame_start = tm.real_time();
        main.update()?;
        main.render(&mut Ui::new(&mut painter))?;
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
