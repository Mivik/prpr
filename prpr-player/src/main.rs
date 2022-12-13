use anyhow::{Context, Result};
use egui::{FontDefinitions, FontData, FontFamily};
use egui_miniquad::EguiMq;
use macroquad::{
    miniquad::EventHandler,
    prelude::{utils::{register_input_subscriber, repeat_all_miniquad_input}, *},
};
use prpr::{build_conf, fs, Prpr};

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    set_pc_assets_folder("assets");

    #[cfg(target_arch = "wasm32")]
    let (fs, config) = {
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
        let name = params.get("chart").unwrap_or_else(|| "nc".to_string());
        (fs::fs_from_assets(&name)?, None)
    };
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let (fs, config) = (fs::fs_from_assets("moment")?, None);
    #[cfg(all(
        not(target_arch = "wasm32"),
        not(target_os = "android"),
        not(target_os = "ios")
    ))]
    let (fs, config) = {
        let mut args = std::env::args();
        let program = args.next().unwrap();
        let Some(path) = args.next() else {
            anyhow::bail!("Usage: {program} <chart>");
        };
        let mut config = None;
        if let Some(config_path) = args.next() {
            config = Some(serde_yaml::from_str(
                &std::fs::read_to_string(config_path).context("Cannot read from config file")?,
            )?);
        }
        (fs::fs_from_file(&path)?, config)
    };

    let (info, fs) = fs::load_info(fs).await?;
    let config = config.unwrap_or_default();

    let mut fps_time = -1;

    let mut prpr = Prpr::new(info, config, fs, None).await?;

    let mut mq = EguiMq::new(prpr.gl.quad_context);

    let subscriber = register_input_subscriber();
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("Noto Sans CJK".to_owned(), FontData::from_owned(load_file("font.ttf").await?));
    fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "Noto Sans CJK".to_owned());
    fonts.families.get_mut(&FontFamily::Monospace).unwrap().insert(0, "Noto Sans CJK".to_owned());
    mq.egui_ctx().set_fonts(fonts);
    'app: loop {
        let frame_start = prpr.get_time();
        prpr.update(None)?;
        prpr.render(None)?;
        prpr.ui(true)?;
        prpr.process_keys()?;
        if prpr.should_exit {
            break 'app;
        }
        prpr.gl.flush();

        repeat_all_miniquad_input(&mut EventReceiver(&mut mq), subscriber);
        /*mq.run(prpr.gl.quad_context, |_, cx| {
            egui::CentralPanel::default().frame(egui::Frame {
                fill: egui::Color32::TRANSPARENT,
                ..Default::default()
            }).show(cx, |ui| {
                ui.label("测测你的！");
            });
        });*/
        // mq.draw(prpr.gl.quad_context);

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

struct EventReceiver<'a>(&'a mut EguiMq);
impl<'a> EventHandler for EventReceiver<'a> {
    fn draw(&mut self, _ctx: &mut miniquad::Context) {}
    fn update(&mut self, _ctx: &mut miniquad::Context) {}

    fn mouse_motion_event(&mut self, _: &mut miniquad::Context, x: f32, y: f32) {
        self.0.mouse_motion_event(x, y);
    }

    fn mouse_wheel_event(&mut self, _: &mut miniquad::Context, dx: f32, dy: f32) {
        self.0.mouse_wheel_event(dx, dy);
    }

    fn mouse_button_down_event(
        &mut self,
        ctx: &mut miniquad::Context,
        mb: miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.0.mouse_button_down_event(ctx, mb, x, y);
    }

    fn mouse_button_up_event(
        &mut self,
        ctx: &mut miniquad::Context,
        mb: miniquad::MouseButton,
        x: f32,
        y: f32,
    ) {
        self.0.mouse_button_up_event(ctx, mb, x, y);
    }

    fn char_event(
        &mut self,
        _ctx: &mut miniquad::Context,
        character: char,
        _keymods: miniquad::KeyMods,
        _repeat: bool,
    ) {
        self.0.char_event(character);
    }

    fn key_down_event(
        &mut self,
        ctx: &mut miniquad::Context,
        keycode: miniquad::KeyCode,
        keymods: miniquad::KeyMods,
        _repeat: bool,
    ) {
        self.0.key_down_event(ctx, keycode, keymods);
    }

    fn key_up_event(&mut self, _ctx: &mut miniquad::Context, keycode: miniquad::KeyCode, keymods: miniquad::KeyMods) {
        self.0.key_up_event(keycode, keymods);
    }
}
